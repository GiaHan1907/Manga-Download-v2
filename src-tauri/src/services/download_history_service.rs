//! Feature plan F2 — download-history purge. Deletes history ROWS only and
//! never touches a file: a purged completed chapter keeps its artifact on
//! disk, and the Library's own proof (`chapters.local_path`) survives, so
//! the chapter stays readable and counted as downloaded.

use sqlx::SqlitePool;

use crate::db::repositories::download_repo;
use crate::domain::ClearHistoryDto;
#[cfg(test)]
use crate::downloader::storage;
use crate::error::AppError;

/// Remove ONE settled task row, for a per-card "gỡ khỏi danh sách". Same
/// rule as the bulk purge: the row goes, the file and the chapter's own
/// library record never do.
pub async fn remove_one(pool: &SqlitePool, task_id: &str) -> Result<bool, AppError> {
    crate::domain::validate_id(task_id, "taskId")?;
    let row = download_repo::get_task(pool, task_id)
        .await?
        .ok_or_else(|| AppError::not_found("tác vụ tải"))?;
    if !download_repo::TERMINAL_STATES.contains(&row.status.as_str()) {
        return Err(AppError::validation(
            "Chỉ gỡ được tác vụ đã kết thúc (hoàn thành, lỗi hoặc đã hủy).",
        ));
    }
    download_repo::delete_terminal_task(pool, task_id).await
}

pub async fn clear(
    pool: &SqlitePool,
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
    let mut deleted = 0i64;
    let mut skipped_other = 0i64;
    for row in candidates {
        if download_repo::delete_terminal_task(pool, &row.id).await? {
            deleted += 1;
        } else {
            // Row moved since selection (e.g. retried mid-purge) — never force.
            skipped_other += 1;
        }
    }
    Ok(ClearHistoryDto {
        deleted,
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
    async fn t11_purge_deletes_every_terminal_row_but_never_files() {
        let fx = fixture().await;
        // completed + artifact ON DISK → the ROW is deleted, the FILE stays.
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

        let outcome = clear(&fx.pool, None, None).await.unwrap();
        assert_eq!(outcome.deleted, 4);
        assert_eq!(outcome.skipped_other, 0);
        let left = statuses(&fx.pool).await;
        let ids: Vec<&str> = left.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["tk-queued"]);
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
    async fn t11_purging_a_completed_row_leaves_the_library_proof_untouched() {
        let fx = fixture().await;
        task_in_status_with(
            &fx.pool,
            "tk-done",
            &fx.chapters[0],
            "completed",
            Some("title-ma1/ch-proof.cbz"),
        )
        .await;
        let dir = storage::library_root(fx.data.path()).join("title-ma1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ch-proof.cbz"), b"real").unwrap();
        sqlx::query("UPDATE chapters SET local_path = 'title-ma1/ch-proof.cbz' WHERE id = ?1")
            .bind(&fx.chapters[0])
            .execute(&fx.pool)
            .await
            .unwrap();

        let outcome = clear(&fx.pool, Some(vec!["completed".to_string()]), None)
            .await
            .unwrap();
        assert_eq!(outcome.deleted, 1);
        assert!(download_repo::get_task(&fx.pool, "tk-done")
            .await
            .unwrap()
            .is_none());
        let stored: Option<String> =
            sqlx::query_scalar("SELECT local_path FROM chapters WHERE id = ?1")
                .bind(&fx.chapters[0])
                .fetch_one(&fx.pool)
                .await
                .unwrap();
        assert_eq!(
            stored.as_deref(),
            Some("title-ma1/ch-proof.cbz"),
            "the chapter's own library record must survive its history row"
        );
        assert!(dir.join("ch-proof.cbz").exists());
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

        let outcome = clear(&fx.pool, Some(vec!["failed".to_string()]), Some(30))
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
            clear(&fx.pool, Some(vec!["downloading".to_string()]), None).await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            clear(&fx.pool, Some(vec![]), None).await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            clear(&fx.pool, None, Some(-1)).await,
            Err(AppError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn t11_remove_one_deletes_a_settled_row_with_its_pages() {
        let fx = fixture().await;
        task_in_status(&fx.pool, "tk-f", &fx.chapters[0], "failed").await;
        sqlx::query("INSERT INTO download_pages (task_id, page_index, status, created_at, updated_at) VALUES (?1, 0, 'pending', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')")
            .bind("tk-f").execute(&fx.pool).await.unwrap();

        assert!(remove_one(&fx.pool, "tk-f").await.unwrap());
        assert!(download_repo::get_task(&fx.pool, "tk-f")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn t11_remove_one_refuses_running_tasks_and_deletes_artifact_backed_rows() {
        let fx = fixture().await;
        task_in_status(&fx.pool, "tk-run", &fx.chapters[0], "downloading").await;
        assert!(matches!(
            remove_one(&fx.pool, "tk-run").await,
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
        assert!(
            remove_one(&fx.pool, "tk-kept").await.unwrap(),
            "a settled row is removable even while its file stays on disk"
        );
        assert!(download_repo::get_task(&fx.pool, "tk-kept")
            .await
            .unwrap()
            .is_none());
        assert!(
            dir.join("ch-keep.cbz").exists(),
            "removing history must never delete the file"
        );

        assert!(matches!(
            remove_one(&fx.pool, "tk-ghost").await,
            Err(AppError::NotFound(_))
        ));
    }
}
