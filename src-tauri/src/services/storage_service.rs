//! Phase 08 storage accounting + regenerable-cache cleanup.
//!
//! Roots are app-owned (app-data `library`, `cache`, `covers`) — the renderer
//! never names a path. Library artifacts are NEVER cleanup candidates: only
//! staging dirs of tasks that reached a terminal state (or vanished from the
//! DB entirely) and the unused covers cache are removed, each checked against
//! the live active-task id set so an in-flight download is never cut off.

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::domain::{CacheCleanupDto, StorageStatusDto};
use crate::downloader::storage;
use crate::error::AppError;

/// Upper bounds keeping the walk linear in what the engine itself wrote.
const MAX_STAGING_ENTRIES: usize = 10_000;

/// Prepares app-managed sub-directories used by later phases (cover cache in
/// Phase 05+, stored pages in Phase 06). Creating directories is not a
/// permission to read/write arbitrary paths: every future file operation must
/// still resolve through this root and be validated by the owning service.
pub fn prepare(app_data_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(app_data_dir.join("covers"))?;
    std::fs::create_dir_all(app_data_dir.join("cache"))?;
    Ok(())
}

pub async fn status(pool: &SqlitePool, app_data_dir: &Path) -> Result<StorageStatusDto, AppError> {
    let library_bytes: Option<i64> = sqlx::query_scalar(
        "SELECT SUM(local_size_bytes) FROM chapters WHERE local_path IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    let active_tasks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM download_tasks WHERE status NOT IN ('completed', 'cancelled')",
    )
    .fetch_one(pool)
    .await?;
    let library_root = storage::library_root(app_data_dir);
    let (cache_bytes, staging_dirs) = staging_footprint(&storage::staging_root(app_data_dir));
    let covers_bytes = dir_size(&app_data_dir.join("covers"));
    let root = crate::services::storage_root::current(pool, app_data_dir).await?;
    let link_ok = root.link_ok;
    let volume = storage::volume_space(&library_root);
    Ok(StorageStatusDto {
        library_bytes: library_bytes.unwrap_or(0),
        cache_bytes: (cache_bytes + covers_bytes) as i64,
        staging_dirs: staging_dirs as i64,
        active_tasks,
        // "Available" here means the app-managed roots can be read at all;
        // a broken custom link is never reported as an available-but-empty
        // library.
        root_available: link_ok
            && (std::fs::read_dir(&library_root).is_ok() || library_root.exists()),
        approximate: true,
        custom_root: root.is_custom,
        root_link_ok: link_ok,
        volume_total_bytes: volume.map(|(total, _)| total),
        volume_free_bytes: volume.map(|(_, free)| free),
    })
}

/// Remove staging dirs of terminal/unknown tasks + covers cache. Returns
/// exactly what was deleted; active task staging is always kept.
pub async fn clear_regenerable_cache(
    pool: &SqlitePool,
    app_data_dir: &Path,
) -> Result<CacheCleanupDto, AppError> {
    let active: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM download_tasks WHERE status NOT IN ('completed', 'cancelled') LIMIT 5000",
    )
    .fetch_all(pool)
    .await?;
    let active_ids: std::collections::HashSet<String> =
        active.into_iter().map(|(id,)| id).collect();

    let staging_root = storage::staging_root(app_data_dir);
    let mut report = CacheCleanupDto {
        removed_dirs: 0,
        freed_bytes: 0,
        kept_active_dirs: 0,
    };
    if let Some(entries) = bounded_read_dir(&staging_root) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if !storage::valid_task_id_shape(&name) {
                continue;
            }
            if active_ids.contains(&name) {
                report.kept_active_dirs += 1;
                continue;
            }
            let size = dir_size(&entry.path());
            match std::fs::remove_dir_all(entry.path()) {
                Ok(()) => {
                    report.removed_dirs += 1;
                    report.freed_bytes += size as i64;
                }
                // Locked/partially-removed staging stays for the next pass —
                // cleanup is idempotent by design. Kind only: no paths in logs.
                Err(error) => crate::app_log!("staging cleanup skipped {name}: {:?}", error.kind()),
            }
        }
    }
    // Covers cache (Phase 08 does not write covers; dir exists from v1 prep).
    let covers = app_data_dir.join("covers");
    let covers_size = dir_size(&covers);
    if covers_size > 0 && clear_dir_contents(&covers) {
        report.freed_bytes += covers_size as i64;
    }
    Ok(report)
}

fn bounded_read_dir(path: &Path) -> Option<std::fs::ReadDir> {
    std::fs::read_dir(path).ok()
}

/// Recursive byte sum over regular files. Symlinks are never followed
/// (reparse-point escape defense, same rule as image_folder listing).
fn dir_size(path: &Path) -> u64 {
    let Some(entries) = bounded_read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    let mut stack: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    stack.truncate(MAX_STAGING_ENTRIES);
    while let Some(current) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&current) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if let Ok(children) = std::fs::read_dir(&current) {
                for child in children.filter_map(Result::ok).take(MAX_STAGING_ENTRIES) {
                    stack.push(child.path());
                }
            }
        } else {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

/// Remove files inside (never the directory itself). True on full success:
/// ANY unreadable entry or failed removal reports failure, so the caller
/// never claims a directory was cleared when contents may remain.
fn clear_dir_contents(path: &Path) -> bool {
    let Some(entries) = bounded_read_dir(path) else {
        return false;
    };
    let mut count = 0usize;
    let mut ok_count = 0usize;
    for entry in entries {
        count += 1;
        let Ok(entry) = entry else {
            continue;
        };
        let target = entry.path();
        let removed = if target.is_dir() {
            std::fs::remove_dir_all(&target).is_ok()
        } else {
            std::fs::remove_file(&target).is_ok()
        };
        if removed {
            ok_count += 1;
        }
    }
    count > 0 && ok_count == count
}

/// staging footprint: bytes + number of direct task dirs.
fn staging_footprint(root: &Path) -> (u64, usize) {
    let Some(entries) = bounded_read_dir(root) else {
        return (0, 0);
    };
    let mut bytes = 0u64;
    let mut dirs = 0usize;
    for entry in entries.filter_map(Result::ok) {
        let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if meta.is_dir() && !meta.file_type().is_symlink() {
            dirs += 1;
            bytes = bytes.saturating_add(dir_size(&entry.path()));
        } else if meta.is_file() {
            bytes = bytes.saturating_add(meta.len());
        }
    }
    (bytes, dirs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::reader::testkit;

    #[test]
    fn creates_subdirectories_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        prepare(dir.path()).unwrap();
        prepare(dir.path()).unwrap();
        assert!(dir.path().join("covers").is_dir());
        assert!(dir.path().join("cache").is_dir());
    }

    async fn task_in(pool: &SqlitePool, task_id: &str, chapter_id: &str, status: &str) {
        let ts = crate::domain::now_iso();
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, \
             completed_pages, total_pages, attempts, revision, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'cbz', 0, 0, 0, 0, 1, ?4, ?4)",
        )
        .bind(task_id)
        .bind(chapter_id)
        .bind(status)
        .bind(&ts)
        .execute(pool)
        .await
        .unwrap();
    }

    fn write_staging(data: &Path, task: &str, bytes: usize) -> PathBuf {
        let dir = storage::task_staging_dir(data, task);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("page.png"), vec![7u8; bytes]).unwrap();
        dir
    }

    #[tokio::test]
    async fn t08_clear_cache_keeps_active_and_removes_terminal_orphans() {
        let (_db, pool) = test_pool().await;
        let data = tempfile::tempdir().unwrap();
        storage::ensure_layout(data.path()).unwrap();
        // Real chapter rows (FK); task ids are their own.
        let (_manga, chapters) = testkit::seed_manga(&pool, "stg", 4).await;
        task_in(&pool, "t-active", &chapters[0], "queued").await;
        task_in(&pool, "t-downloading", &chapters[1], "downloading").await;
        task_in(&pool, "t-done", &chapters[2], "completed").await;
        task_in(&pool, "t-cancelled", &chapters[3], "cancelled").await;

        let active = write_staging(data.path(), "t-active", 10);
        let downloading = write_staging(data.path(), "t-downloading", 10);
        let done = write_staging(data.path(), "t-done", 10);
        let cancelled = write_staging(data.path(), "t-cancelled", 10);
        let orphan = write_staging(data.path(), "not-in-db", 10);

        let report = clear_regenerable_cache(&pool, data.path()).await.unwrap();
        assert_eq!(report.kept_active_dirs, 2);
        assert_eq!(report.removed_dirs, 3, "done + cancelled + orphan");
        assert!(active.exists() && downloading.exists(), "active kept");
        assert!(!done.exists() && !cancelled.exists() && !orphan.exists());
        assert_eq!(report.freed_bytes, 30);
        // Idempotent second pass frees nothing further.
        let again = clear_regenerable_cache(&pool, data.path()).await.unwrap();
        assert_eq!(again.removed_dirs, 0);
    }

    #[tokio::test]
    async fn t08_status_reports_db_sizes_and_root_flag() {
        let (_db, pool) = test_pool().await;
        let data = tempfile::tempdir().unwrap();
        storage::ensure_layout(data.path()).unwrap();
        let (_manga_id, chapters) = testkit::seed_manga(&pool, "st", 2).await;
        testkit::build_cbz(
            &data.path().join("stg"),
            &storage::library_root(data.path()),
            "st/c1.cbz",
            2,
        );
        testkit::mark_downloaded(&pool, &chapters[0], "st/c1.cbz", "cbz").await;
        sqlx::query("UPDATE chapters SET local_size_bytes = 1234 WHERE id = ?1")
            .bind(&chapters[0])
            .execute(&pool)
            .await
            .unwrap();
        write_staging(data.path(), "t-some", 5);
        task_in(&pool, "t-some", &chapters[1], "downloading").await;

        let s = status(&pool, data.path()).await.unwrap();
        assert_eq!(s.library_bytes, 1234);
        assert_eq!(s.active_tasks, 1);
        assert_eq!(s.staging_dirs, 1);
        assert!(s.cache_bytes >= 5);
        assert!(s.root_available);
        assert!(s.approximate, "sizes are documented as approximate");
    }
}
