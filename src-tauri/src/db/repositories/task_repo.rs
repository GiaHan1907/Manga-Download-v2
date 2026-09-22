use sqlx::{Row, SqlitePool};

use crate::domain::DownloadTaskDto;
use crate::error::AppError;

/// Read-only in Phase 04: exposes persisted task metadata (usually empty).
/// Creating/transitioning real tasks belongs to the Phase 06 scheduler — do not
/// add write commands here before then.
pub async fn list_tasks(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DownloadTaskDto>, i64), AppError> {
    let valid_status = match status {
        None => None,
        Some(value @ ("queued" | "paused" | "failed" | "completed" | "cancelled")) => Some(value),
        Some(_) => return Err(AppError::validation("Trạng thái tác vụ không hợp lệ.")),
    };

    let total = match valid_status {
        Some(value) => {
            sqlx::query("SELECT COUNT(*) AS count FROM download_tasks WHERE status = ?1")
                .bind(value)
                .fetch_one(pool)
                .await?
                .try_get::<i64, _>("count")?
        }
        None => sqlx::query("SELECT COUNT(*) AS count FROM download_tasks")
            .fetch_one(pool)
            .await?
            .try_get::<i64, _>("count")?,
    };

    let rows = match valid_status {
        Some(value) => {
            sqlx::query(
                "SELECT id, chapter_id, status, target_format, priority, completed_pages, total_pages, error_code, created_at, updated_at \
                 FROM download_tasks WHERE status = ?1 ORDER BY created_at ASC, id ASC LIMIT ?2 OFFSET ?3",
            )
            .bind(value)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, chapter_id, status, target_format, priority, completed_pages, total_pages, error_code, created_at, updated_at \
                 FROM download_tasks ORDER BY created_at ASC, id ASC LIMIT ?1 OFFSET ?2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
    };

    let items = rows
        .iter()
        .map(|row| {
            Ok(DownloadTaskDto {
                id: row.try_get("id")?,
                chapter_id: row.try_get("chapter_id")?,
                status: row.try_get("status")?,
                target_format: row.try_get("target_format")?,
                priority: row.try_get("priority")?,
                completed_pages: row.try_get("completed_pages")?,
                total_pages: row.try_get("total_pages")?,
                error_code: row.try_get("error_code")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok((items, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};

    #[tokio::test]
    async fn t04_09_tasks_list_empty_and_status_filter_validated() {
        let (_dir, pool) = crate::db::test_pool().await;
        let (items, total) = list_tasks(&pool, None, 20, 0).await.unwrap();
        assert!(items.is_empty());
        assert_eq!(total, 0);

        assert!(list_tasks(&pool, Some("queued"), 20, 0).await.is_ok());
        assert!(matches!(
            list_tasks(&pool, Some("downloading"), 20, 0)
                .await
                .unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn fk_restrict_prevents_deleting_chapter_with_task_row() {
        let (_dir, pool) = crate::db::test_pool().await;
        ensure_source(&pool, "src-a", "Source A").await.unwrap();
        let (manga_id, _) = upsert_manga(&pool, "src-a", "m1", "T", None, None, "ongoing")
            .await
            .unwrap();
        let (chapter_id, _) = upsert_chapter(&pool, &manga_id, "c-1", "C1", None, None)
            .await
            .unwrap();

        // Raw insert only for constraint testing; Phase 04 has no task-writing API.
        let ts = crate::domain::now_iso();
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, completed_pages, total_pages, created_at, updated_at) \
             VALUES ('task-x', ?1, 'queued', 'cbz', 0, 0, 0, ?2, ?2)",
        )
        .bind(&chapter_id)
        .bind(&ts)
        .execute(&pool)
        .await
        .unwrap();

        let error = sqlx::query("DELETE FROM mangas WHERE id = ?1")
            .bind(&manga_id)
            .execute(&pool)
            .await;
        assert!(
            error.is_err(),
            "RESTRICT must block deleting manga referenced downstream"
        );
    }
}
