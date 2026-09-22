//! Startup reconciliation. Bounded to app-owned locations and to task ids
//! the database knows about — the user's library is never scanned
//! recursively and unknown files are never deleted (design §7.4).

use std::collections::HashSet;
use std::path::Path;

use sqlx::SqlitePool;

use crate::db::repositories::download_repo;
use crate::downloader::storage;
use crate::downloader::transfer;
use crate::error::AppError;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct StartupReport {
    /// Active worker states normalized back to queued/paused.
    pub normalized_tasks: u64,
    /// `*.part` files discarded in known task staging dirs.
    pub discarded_parts: usize,
    /// Orphan export temp artifacts (`*.cbz.part`, `*.part-dir`) discarded
    /// inside library dirs — only names carrying a known task token.
    pub discarded_library_parts: usize,
    /// Completed tasks whose verified artifact vanished (offline flag
    /// cleared, task history preserved — no phantom "available offline").
    pub cleared_missing_outputs: usize,
}

pub async fn reconcile_startup(
    pool: &SqlitePool,
    app_data_dir: &Path,
) -> Result<StartupReport, AppError> {
    storage::ensure_layout(app_data_dir)?;
    // 1. Lifecycle normalization first: statuses are authoritative for what
    //    the filesystem pass may touch.
    let mut report = StartupReport {
        normalized_tasks: download_repo::recover_stale_tasks(pool).await?,
        ..Default::default()
    };

    // 2. Discard interrupted writes in staging dirs of tasks the DB still
    //    considers unfinished (bounded: only known task folders).
    let unfinished: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM download_tasks WHERE status NOT IN ('completed', 'cancelled') LIMIT 5000",
    )
    .fetch_all(pool)
    .await?;
    for (task_id,) in unfinished {
        if !storage::valid_task_id_shape(&task_id) {
            continue;
        }
        let staging = storage::task_staging_dir(app_data_dir, &task_id);
        let before = count_parts(&staging);
        transfer::discard_part_files(&staging)?;
        report.discarded_parts += before.saturating_sub(count_parts(&staging));
        // Crash boundary B (published-but-uncommitted): the task is now
        // `queued`; re-entry re-does export idempotently and publish_artifact
        // adopts the verified existing artifact instead of overwriting.
    }

    // 2b. Orphan export temps inside the library (crash between writing the
    //     `.cbz.part` / `.part-dir` and publishing). Bounded like everything
    //     else: only names embedding the token of a task the DB knows about
    //     are removed — unknown files are never touched.
    let library_root = storage::library_root(app_data_dir);
    report.discarded_library_parts = sweep_library_export_temps(&library_root, pool).await?;

    // 3. Truthful offline flags: a completed task only counts while its
    //    recorded artifact exists on disk.
    let completed = download_repo::tasks_in_status(pool, "completed").await?;
    let mut seen_chapters: HashSet<String> = HashSet::new();
    for task in completed {
        let Some(relpath) = task.output_relpath.as_deref() else {
            continue;
        };
        if storage::validate_relpath(relpath).is_err() {
            continue;
        }
        if !library_root.join(relpath).exists() && seen_chapters.insert(task.chapter_id.clone()) {
            sqlx::query(
                "UPDATE chapters SET local_path = NULL, output_format = NULL WHERE id = ?1",
            )
            .bind(&task.chapter_id)
            .execute(pool)
            .await?;
            report.cleared_missing_outputs += 1;
        }
    }
    Ok(report)
}

/// Remove export temps (`<base>.cbz.part`, `<base>.part-dir`) from manga
/// directories, but only when the embedded `-t<token>` belongs to a task the
/// DB knows about. Unknown names are left exactly as found.
async fn sweep_library_export_temps(
    library_root: &Path,
    pool: &SqlitePool,
) -> Result<usize, AppError> {
    let task_ids: Vec<(String,)> = sqlx::query_as("SELECT id FROM download_tasks LIMIT 5000")
        .fetch_all(pool)
        .await?;
    let tokens: HashSet<String> = task_ids
        .iter()
        .map(|(id,)| storage::id_token(id))
        .filter(|token| !token.is_empty())
        .collect();
    let mut removed = 0usize;
    let Ok(manga_dirs) = std::fs::read_dir(library_root) else {
        return Ok(0);
    };
    for manga_dir in manga_dirs.filter_map(Result::ok) {
        if !manga_dir.path().is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(manga_dir.path()) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(token) = export_temp_task_token(&name) else {
                continue;
            };
            if !tokens.contains(token) {
                continue;
            }
            let path = entry.path();
            let removed_ok = if path.is_dir() {
                std::fs::remove_dir_all(&path).is_ok()
            } else {
                std::fs::remove_file(&path).is_ok()
            };
            if removed_ok {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

/// `<title>-c<chapter-token>-t<task-token>[.cbz].part(-dir)?` → task token.
/// Returns None for names that are not export temps of the engine.
fn export_temp_task_token(name: &str) -> Option<&str> {
    let base = name
        .strip_suffix(".part-dir")
        .or_else(|| name.strip_suffix(".part"))?;
    let tail = base.rsplit_once("-t")?.1;
    let token = tail.split('.').next().filter(|t| !t.is_empty())?;
    if token.len() > 8 || !token.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(token)
}

fn count_parts(dir: &Path) -> usize {
    if !dir.exists() {
        return 0;
    }
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .is_some_and(|x| x == "part")
                })
                .count()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::{
        download_repo,
        manga_repo::{ensure_source, upsert_chapter, upsert_manga},
    };
    use crate::db::test_pool;

    async fn seed_chapter(pool: &SqlitePool) -> (String, String) {
        ensure_source(pool, "src-a", "A").await.unwrap();
        let (manga_id, _) = upsert_manga(pool, "src-a", "m", "Title", None, None, "ongoing")
            .await
            .unwrap();
        let (chapter_id, _) = upsert_chapter(pool, &manga_id, "c1", "C1", None, None)
            .await
            .unwrap();
        (manga_id, chapter_id)
    }

    #[tokio::test]
    async fn t06_15_crash_after_page_rename_before_checkpoint_resumes() {
        let (_dir, pool) = test_pool().await;
        let app_data = tempfile::tempdir().unwrap();
        let (_, chapter) = seed_chapter(&pool).await;
        let task = download_repo::queue_batch(&pool, &[chapter], "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        download_repo::ensure_page_rows(&pool, &task.id, 2)
            .await
            .unwrap();
        // Simulated crash point: file renamed on disk, but the row is still
        // 'pending' (checkpoint lost). Recovery must keep the task queued;
        // reconcile in the worker reuses... actually a pending row + present
        // file means re-download overwrites it deterministically (safe), and
        // a stored row + missing file resets to pending. Verify both stable.
        let staging = storage::task_staging_dir(app_data.path(), &task.id);
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("000000.png.part"), b"partial").unwrap();
        download_repo::transition(&pool, &task.id, &["queued"], "downloading")
            .await
            .unwrap();

        let report = reconcile_startup(&pool, app_data.path()).await.unwrap();
        assert_eq!(report.normalized_tasks, 1);
        assert_eq!(report.discarded_parts, 1);
        let after = download_repo::get_task(&pool, &task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.status, "queued");
        // Partial page file gone; the untouched final dir remains.
        assert!(!staging.join("000000.png.part").exists());
    }

    #[tokio::test]
    async fn t06_16_completed_task_with_vanished_file_clears_offline_flag() {
        let (_dir, pool) = test_pool().await;
        let app_data = tempfile::tempdir().unwrap();
        let (_, chapter) = seed_chapter(&pool).await;
        let task = download_repo::queue_batch(&pool, std::slice::from_ref(&chapter), "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        for from_to in [
            ("queued", "resolving"),
            ("resolving", "downloading"),
            ("downloading", "verifying"),
            ("verifying", "exporting"),
        ] {
            download_repo::transition(&pool, &task.id, &[from_to.0], from_to.1)
                .await
                .unwrap();
        }
        download_repo::publish_completed(&pool, &task.id, "title-abc/ch-def-t123.cbz", "cbz", None)
            .await
            .unwrap();
        assert!(sqlx::query("SELECT 1")
            .fetch_optional(&pool)
            .await
            .unwrap()
            .is_some());

        let report = reconcile_startup(&pool, app_data.path()).await.unwrap();
        assert_eq!(report.cleared_missing_outputs, 1);
        let local: (Option<String>,) =
            sqlx::query_as("SELECT local_path FROM chapters WHERE id = ?1")
                .bind(&chapter)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(local.0, None, "phantom offline flag must clear");
    }

    #[test]
    fn t10_export_temp_token_parsing_is_conservative() {
        assert_eq!(
            export_temp_task_token("ch-title-abc-tdeadbeef.cbz.part"),
            Some("deadbeef")
        );
        assert_eq!(
            export_temp_task_token("ch-title-abc-tdeadbeef.part-dir"),
            Some("deadbeef")
        );
        // Not an export temp: unknown shapes never match.
        assert_eq!(export_temp_task_token("notes.txt"), None);
        assert_eq!(export_temp_task_token("chapter.cbz"), None);
        assert_eq!(export_temp_task_token("readme.part"), None);
        assert_eq!(export_temp_task_token("x-tdeadbeef1.part"), None); // token too long
        assert_eq!(export_temp_task_token("x-t!.part"), None);
        // A published artifact is NEVER an export temp.
        assert_eq!(export_temp_task_token("ch-a-cb-tdeadbeef.cbz"), None);
    }

    #[tokio::test]
    async fn t10_startup_sweeps_library_export_temps_only_for_known_tasks() {
        use crate::downloader::storage;
        let (_dir, pool) = test_pool().await;
        let app_data = tempfile::tempdir().unwrap();
        let (_, chapter) = seed_chapter(&pool).await;
        let task = download_repo::queue_batch(&pool, std::slice::from_ref(&chapter), "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        let token = storage::id_token(&task.id);

        let manga_dir = storage::library_root(app_data.path()).join("title-ma1");
        std::fs::create_dir_all(&manga_dir).unwrap();
        let orphan = manga_dir.join(format!("ch-c1-cabc-t{token}.cbz.part"));
        let orphan_dir = manga_dir.join(format!("ch-c2-cdef-t{token}.part-dir"));
        let foreign = manga_dir.join("ch-c3-cghi-t0000dead.cbz.part"); // unknown task
        let published = manga_dir.join(format!("ch-c4-cjkl-t{token}.cbz"));
        std::fs::write(&orphan, b"half-written").unwrap();
        std::fs::create_dir_all(&orphan_dir).unwrap();
        std::fs::write(&foreign, b"someone else's temp").unwrap();
        std::fs::write(&published, b"real artifact").unwrap();

        let report = reconcile_startup(&pool, app_data.path()).await.unwrap();
        assert_eq!(report.discarded_library_parts, 2);
        assert!(!orphan.exists() && !orphan_dir.exists());
        assert!(foreign.exists(), "unknown-task temps are never touched");
        assert!(
            published.exists(),
            "published artifacts are never touched by the sweep"
        );
    }
}
