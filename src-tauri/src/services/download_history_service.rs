//! Feature plan F2 — download-history purge. Non-destructive to anything the
//! library still references: a completed task whose verified artifact is
//! present on disk keeps its row (that row is the proof behind the card's
//! "đã xác minh" line and the `has_missing` signal — deleting it would turn
//! a healthy chapter into an unprovable one). Never touches files.

use std::path::Path;

use sqlx::SqlitePool;

use crate::db::repositories::download_repo;
use crate::domain::ClearHistoryDto;
use crate::downloader::storage;
use crate::error::AppError;

/// Remove ONE settled task row, for a per-card "gỡ khỏi danh sách". Same
/// non-destructive rule as the bulk purge: a completed task whose verified
/// artifact is still on disk keeps its row — that row is the proof behind the
/// card's "đã xác minh" line, so removing it would turn a healthy chapter into
/// an unprovable one. Never touches files.
pub async fn remove_one(
    pool: &SqlitePool,
    app_data_dir: &Path,
    task_id: &str,
) -> Result<bool, AppError> {
    crate::domain::validate_id(task_id, "taskId")?;
    let row = download_repo::get_task(pool, task_id)
        .await?
        .ok_or_else(|| AppError::not_found("tác vụ tải"))?;
    if !download_repo::TERMINAL_STATES.contains(&row.status.as_str()) {
        return Err(AppError::validation(
            "Chỉ gỡ được tác vụ đã kết thúc (hoàn thành, lỗi hoặc đã hủy).",
        ));
    }
    let artifact_present = row.status == "completed"
        && row.output_relpath.as_deref().is_some_and(|relpath| {
            storage::validate_relpath(relpath).is_ok()
                && storage::library_root(app_data_dir).join(relpath).exists()
        });
    if artifact_present {
        return Err(AppError::validation(
            "Chương này còn file đã tải — xóa trong Thư viện để gỡ cả bản ghi.",
        ));
    }
    download_repo::delete_terminal_task(pool, task_id).await
}

pub async fn clear(
    pool: &SqlitePool,
    app_data_dir: &Path,
    statuses: Option<Vec<String>>,
    older_than_days: Option<i64>,
) -> Result<ClearHistoryDto, AppError> {
    let requested: Vec<&str> = match statuses.as_deref() {
        None => download_repo::TERMINAL_STATES.to_vec(),
        Some([]) => {
            return Err(AppError::validation(
                "Phải chọn ít nhất một trạng thái để xóa.",
            ))
        }
        Some(list) => {
            for status in list {
                if !download_repo::TERMINAL_STATES.contains(&status.as_str()) {
                    return Err(AppError::validation(
                        "Chỉ được xóa lịch sử ở trạng thái hoàn tất/lỗi/đã hủy.",
                    ));
                }
            }
            list.iter().map(String::as_str).collect()
        }
    };
    let cutoff = match older_than_days {
        None => None,
        Some(days) if (0..=36_500).contains(&days) => Some(
            (chrono::Utc::now() - chrono::Duration::days(days))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        ),
        Some(_) => {
            return Err(AppError::validation(
                "olderThanDays phải trong khoảng 0..=36500.",
            ))
        }
    };

    let candidates = download_repo::history_candidates(pool, &requested, cutoff.as_deref()).await?;
    let library_root = storage::library_root(app_data_dir);
    let mut deleted = 0i64;
    let mut skipped_artifact = 0i64;
    let mut skipped_other = 0i64;
    for row in candidates {
        let artifact_present = row.status == "completed"
            && row.output_relpath.as_deref().is_some_and(|relpath| {
                storage::validate_relpath(relpath).is_ok() && library_root.join(relpath).exists()
            });
        if artifact_present {
            skipped_artifact += 1;
            continue;
        }
        if download_repo::delete_terminal_task(pool, &row.id).await? {
            deleted += 1;
        } else {
            // Row moved since selection (e.g. retried mid-purge) — never force.
            skipped_other += 1;
        }
    }
    Ok(ClearHistoryDto {
        deleted,
        skipped_artifact,
        skipped_other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};
    use crate::db::test_pool;

    async fn seed(pool: &SqlitePool) -> Vec<String> {
        ensure_source(pool, "src-a", "A").await.unwrap();
        let (manga_id, _) = upsert_manga(pool, "src-a", "m", "Title", None, None, "ongoing")
            .await
            .unwrap();
        let mut chapters = Vec::new();
        for index in 0..5 {
            let (chapter_id, _) = upsert_chapter(
                pool,
                &manga_id,
                &format!("c{index}"),
                &format!("C{index}"),
                None,
                None,
            )
            .await
            .unwrap();
            chapters.push(chapter_id);
        }
        chapters
    }

    async fn task_in_status(pool: &SqlitePool, id: &str, chapter_id: &str, status: &str) {
        task_in_status_with(pool, id, chapter_id, status, None).await;
    }

    async fn task_in_status_with(
        pool: &SqlitePool,
        id: &str,
        chapter_id: &str,
        status: &str,
        relpath: Option<&str>,
    ) {
        let ts = crate::domain::now_iso();
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, \
             completed_pages, total_pages, attempts, revision, output_relpath, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'cbz', 0, 1, 2, 0, 1, ?4, ?5, ?5)",
        )
        .bind(id)
        .bind(chapter_id)
        .bind(status)
        .bind(relpath)
        .bind(&ts)
        .execute(pool)
        .await
        .unwrap();
    }

    struct Fixture {
        pool: SqlitePool,
        _db: tempfile::TempDir,
        data: tempfile::TempDir,
        chapters: Vec<String>,
    }

    async fn fixture() -> Fixture {
        let (_db, pool) = test_pool().await;
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(storage::library_root(data.path())).unwrap();
        let chapters = seed(&pool).await;
        Fixture {
            pool,
            _db,
            data,
            chapters,
        }
    }

    async fn statuses(pool: &SqlitePool) -> Vec<(String, String)> {
        sqlx::query_as("SELECT id, status FROM download_tasks ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn t11_purge_keeps_completed_with_artifact_deletes_rest_cascade_pages() {
        let fx = fixture().await;
        // completed + artifact ON DISK → kept; completed + gone → deleted.
        task_in_status_with(
            &fx.pool,
            "tk-kept",
            &fx.chapters[0],
            "completed",
            Some("title-ma1/ch-x-t11.cbz"),
        )
        .await;
        let artifact_dir = storage::library_root(fx.data.path()).join("title-ma1");
        std::fs::create_dir_all(&artifact_dir).unwrap();
        std::fs::write(artifact_dir.join("ch-x-t11.cbz"), b"real").unwrap();
        // Same-status twin whose artifact file is genuinely absent → deleted.
        task_in_status_with(
            &fx.pool,
            "tk-gone",
            &fx.chapters[1],
            "completed",
            Some("title-ma1/ch-y-t22.cbz"),
        )
        .await;
        task_in_status(&fx.pool, "tk-failed", &fx.chapters[2], "failed").await;
        task_in_status(&fx.pool, "tk-cancelled", &fx.chapters[3], "cancelled").await;
        task_in_status(&fx.pool, "tk-queued", &fx.chapters[4], "queued").await;
        // Checkpoint rows must cascade with the deleted tasks.
        for id in ["tk-gone", "tk-failed"] {
            sqlx::query("INSERT INTO download_pages (task_id, page_index, status, created_at, updated_at) VALUES (?1, 0, 'pending', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')")
                .bind(id).execute(&fx.pool).await.unwrap();
        }

        let outcome = clear(&fx.pool, fx.data.path(), None, None).await.unwrap();
        assert_eq!(outcome.deleted, 3);
        assert_eq!(outcome.skipped_artifact, 1);
        assert_eq!(outcome.skipped_other, 0);
        let left = statuses(&fx.pool).await;
        let ids: Vec<&str> = left.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["tk-kept", "tk-queued"]);
        let pages: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM download_pages")
            .fetch_one(&fx.pool)
            .await
            .unwrap();
        assert_eq!(pages.0, 0, "cascade removed checkpoint rows");
        assert!(
            storage::library_root(fx.data.path())
                .join("title-ma1/ch-x-t11.cbz")
                .exists(),
            "purge must never delete artifacts"
        );
    }

    #[tokio::test]
    async fn t11_cutoff_and_status_filters_are_honored() {
        let fx = fixture().await;
        task_in_status(&fx.pool, "tk-old", &fx.chapters[0], "failed").await;
        sqlx::query(
            "UPDATE download_tasks SET updated_at = '2020-01-01T00:00:00.000Z' WHERE id = 'tk-old'",
        )
        .execute(&fx.pool)
        .await
        .unwrap();
        task_in_status(&fx.pool, "tk-new", &fx.chapters[1], "failed").await;
        task_in_status(&fx.pool, "tk-cancelled", &fx.chapters[2], "cancelled").await;

        let outcome = clear(
            &fx.pool,
            fx.data.path(),
            Some(vec!["failed".to_string()]),
            Some(30),
        )
        .await
        .unwrap();
        assert_eq!(outcome.deleted, 1);
        let left = statuses(&fx.pool).await;
        let ids: Vec<&str> = left.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["tk-cancelled", "tk-new"],
            "only the old failed row matches; cancelled is outside the requested statuses"
        );
    }

    #[tokio::test]
    async fn t11_rejects_non_terminal_or_empty_status_sets() {
        let fx = fixture().await;
        assert!(matches!(
            clear(
                &fx.pool,
                fx.data.path(),
                Some(vec!["downloading".to_string()]),
                None
            )
            .await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            clear(&fx.pool, fx.data.path(), Some(vec![]), None).await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            clear(&fx.pool, fx.data.path(), None, Some(-1)).await,
            Err(AppError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn t11_remove_one_deletes_a_settled_row_with_its_pages() {
        let fx = fixture().await;
        task_in_status(&fx.pool, "tk-f", &fx.chapters[0], "failed").await;
        sqlx::query("INSERT INTO download_pages (task_id, page_index, status, created_at, updated_at) VALUES (?1, 0, 'pending', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')")
            .bind("tk-f").execute(&fx.pool).await.unwrap();

        assert!(remove_one(&fx.pool, fx.data.path(), "tk-f").await.unwrap());
        assert!(download_repo::get_task(&fx.pool, "tk-f")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn t11_remove_one_refuses_running_tasks_and_artifact_backed_rows() {
        let fx = fixture().await;
        task_in_status(&fx.pool, "tk-run", &fx.chapters[0], "downloading").await;
        assert!(matches!(
            remove_one(&fx.pool, fx.data.path(), "tk-run").await,
            Err(AppError::Validation(_))
        ));

        task_in_status_with(
            &fx.pool,
            "tk-kept",
            &fx.chapters[1],
            "completed",
            Some("title-ma1/ch-keep.cbz"),
        )
        .await;
        let dir = storage::library_root(fx.data.path()).join("title-ma1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ch-keep.cbz"), b"real").unwrap();
        assert!(matches!(
            remove_one(&fx.pool, fx.data.path(), "tk-kept").await,
            Err(AppError::Validation(_))
        ));
        assert!(
            download_repo::get_task(&fx.pool, "tk-kept")
                .await
                .unwrap()
                .is_some(),
            "the proof row must survive a refused removal"
        );

        assert!(matches!(
            remove_one(&fx.pool, fx.data.path(), "tk-ghost").await,
            Err(AppError::NotFound(_))
        ));
    }
}
