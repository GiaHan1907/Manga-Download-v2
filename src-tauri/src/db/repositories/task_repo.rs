use sqlx::SqlitePool;

use crate::db::repositories::download_repo::{self, TaskRow};
use crate::domain::DownloadTaskDto;
use crate::error::AppError;

/// Read-only task metadata for the Downloads surface (Phase 04 contract,
/// widened by the Phase 06 engine columns).
pub async fn list_tasks(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<DownloadTaskDto>, i64), AppError> {
    let valid_status = match status {
        None => None,
        Some(value) if download_repo::STATUS_STATES.contains(&value) => Some(value),
        Some(_) => return Err(AppError::validation("Trạng thái tác vụ không hợp lệ.")),
    };

    let total = match valid_status {
        Some(value) => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM download_tasks WHERE status = ?1")
                .bind(value)
                .fetch_one(pool)
                .await?
        }
        None => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM download_tasks")
                .fetch_one(pool)
                .await?
        }
    };

    let rows: Vec<TaskRow> = match valid_status {
        Some(value) => {
            sqlx::query_as(
                "SELECT t.id, t.chapter_id, t.status, t.target_format, t.priority, t.completed_pages, \
                        t.total_pages, t.attempts, t.next_retry_at, t.error_code, t.error_message, t.revision, \
                        t.finished_at, t.output_relpath, c.title AS chapter_title, m.title AS manga_title, \
                        c.manga_id AS manga_id, c.number AS chapter_number, \
                        t.created_at, t.updated_at \
                 FROM download_tasks t \
                 JOIN chapters c ON c.id = t.chapter_id \
                 JOIN mangas m ON m.id = c.manga_id \
                 WHERE t.status = ?1 \
                 ORDER BY t.priority DESC, \
                          (SELECT MIN(t2.created_at) FROM download_tasks t2 \
                           JOIN chapters c2 ON c2.id = t2.chapter_id \
                           WHERE c2.manga_id = c.manga_id) ASC, \
                          c.manga_id ASC, \
                          c.number IS NULL ASC, c.number ASC, t.created_at ASC, t.id ASC \
                 LIMIT ?2 OFFSET ?3",
            )
            .bind(value)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as(
                "SELECT t.id, t.chapter_id, t.status, t.target_format, t.priority, t.completed_pages, \
                        t.total_pages, t.attempts, t.next_retry_at, t.error_code, t.error_message, t.revision, \
                        t.finished_at, t.output_relpath, c.title AS chapter_title, m.title AS manga_title, \
                        c.manga_id AS manga_id, c.number AS chapter_number, \
                        t.created_at, t.updated_at \
                 FROM download_tasks t \
                 JOIN chapters c ON c.id = t.chapter_id \
                 JOIN mangas m ON m.id = c.manga_id \
                 ORDER BY t.priority DESC, \
                          (SELECT MIN(t2.created_at) FROM download_tasks t2 \
                           JOIN chapters c2 ON c2.id = t2.chapter_id \
                           WHERE c2.manga_id = c.manga_id) ASC, \
                          c.manga_id ASC, \
                          c.number IS NULL ASC, c.number ASC, t.created_at ASC, t.id ASC \
                 LIMIT ?1 OFFSET ?2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
    };

    Ok((rows.into_iter().map(to_dto).collect(), total))
}

pub fn to_dto(row: TaskRow) -> DownloadTaskDto {
    DownloadTaskDto {
        id: row.id,
        chapter_id: row.chapter_id,
        status: row.status,
        target_format: row.target_format,
        priority: row.priority,
        completed_pages: row.completed_pages,
        total_pages: row.total_pages,
        attempts: row.attempts,
        next_retry_at: row.next_retry_at,
        error_code: row.error_code,
        error_message: row.error_message,
        revision: row.revision,
        finished_at: row.finished_at,
        output_relpath: row.output_relpath,
        chapter_title: row.chapter_title,
        manga_title: row.manga_title,
        manga_id: row.manga_id,
        chapter_number: row.chapter_number,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

impl From<TaskRow> for DownloadTaskDto {
    fn from(row: TaskRow) -> Self {
        to_dto(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};

    async fn seed_chapter(pool: &SqlitePool) -> String {
        ensure_source(pool, "src-a", "Source A").await.unwrap();
        let (manga_id, _) = upsert_manga(pool, "src-a", "m1", "T", None, None, "ongoing")
            .await
            .unwrap();
        let (chapter_id, _) = upsert_chapter(pool, &manga_id, "c-1", "C1", None, None)
            .await
            .unwrap();
        chapter_id
    }

    /// The Downloads list is READ in chapter order, not submission order:
    /// `queue_batch` stamps one created_at on the whole batch, so ordering by
    /// created_at/id showed a bulk queue in effectively random order (the same
    /// defect the claim query had). A chapter with no number sorts last.
    #[tokio::test]
    async fn list_tasks_orders_by_chapter_number() {
        let (_dir, pool) = crate::db::test_pool().await;
        ensure_source(&pool, "src-a", "Source A").await.unwrap();
        let (manga_id, _) = upsert_manga(&pool, "src-a", "m-order", "T", None, None, "ongoing")
            .await
            .unwrap();
        let mut chapters = Vec::new();
        for number in [3.0, 1.0, 2.0] {
            let (chapter_id, _) = upsert_chapter(
                &pool,
                &manga_id,
                &format!("c-{number}"),
                "C",
                Some(number),
                None,
            )
            .await
            .unwrap();
            chapters.push(chapter_id);
        }
        let (unknown, _) = upsert_chapter(&pool, &manga_id, "c-unknown", "C", None, None)
            .await
            .unwrap();
        chapters.push(unknown);

        let ts = crate::domain::now_iso();
        for chapter_id in chapters {
            sqlx::query(
                "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, \
                 completed_pages, total_pages, attempts, revision, created_at, updated_at) \
                 VALUES (?1, ?2, 'queued', 'cbz', 0, 0, 10, 0, 1, ?3, ?3)",
            )
            .bind(crate::domain::new_id())
            .bind(chapter_id)
            .bind(&ts)
            .execute(&pool)
            .await
            .unwrap();
        }

        let (items, total) = list_tasks(&pool, None, 20, 0).await.unwrap();
        assert_eq!(total, 4);
        let numbers: Vec<Option<f64>> = items.iter().map(|task| task.chapter_number).collect();
        assert_eq!(
            numbers,
            vec![Some(1.0), Some(2.0), Some(3.0), None],
            "chapter order, unnumbered last"
        );
    }

    #[tokio::test]
    async fn t04_09_tasks_list_empty_and_status_filter_validated() {
        let (_dir, pool) = crate::db::test_pool().await;
        let (items, total) = list_tasks(&pool, None, 20, 0).await.unwrap();
        assert!(items.is_empty());
        assert_eq!(total, 0);

        assert!(list_tasks(&pool, Some("queued"), 20, 0).await.is_ok());
        assert!(list_tasks(&pool, Some("downloading"), 20, 0).await.is_ok());
        assert!(matches!(
            list_tasks(&pool, Some("not-a-status"), 20, 0)
                .await
                .unwrap_err(),
            AppError::Validation(_)
        ));
    }

    #[tokio::test]
    async fn fk_restrict_prevents_deleting_chapter_with_task_row() {
        let (_dir, pool) = crate::db::test_pool().await;
        let chapter_id = seed_chapter(&pool).await;

        let outcome =
            download_repo::queue_batch(&pool, std::slice::from_ref(&chapter_id), "cbz", 0)
                .await
                .unwrap();
        assert_eq!(outcome.created.len(), 1);

        let manga_id: (String,) = sqlx::query_as("SELECT manga_id FROM chapters WHERE id = ?1")
            .bind(&chapter_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let error = sqlx::query("DELETE FROM mangas WHERE id = ?1")
            .bind(&manga_id.0)
            .execute(&pool)
            .await;
        assert!(
            error.is_err(),
            "RESTRICT must block deleting manga referenced downstream"
        );
    }
}
