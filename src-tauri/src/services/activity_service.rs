//! "What happened while I was not looking": new chapters the watcher found,
//! chapters that finished downloading, and downloads that gave up. Read-only
//! merge of rows the app already writes — no new storage, no notifications.

use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;

/// One line in the activity feed. `kind` is the closed set
/// `alert | downloaded | failed`; `code` is present for failures only.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItemDto {
    pub kind: String,
    pub manga_id: String,
    pub manga_title: String,
    pub chapter_title: String,
    /// ISO-8601 instant used for ordering AND display (the UI renders it
    /// relative, so both sides agree on the instant).
    pub at: String,
    pub code: Option<String>,
}

pub const ACTIVITY_MAX_LIMIT: i64 = 50;
pub const ACTIVITY_DEFAULT_LIMIT: i64 = 20;

pub async fn recent(pool: &SqlitePool, limit: i64) -> Result<Vec<ActivityItemDto>, AppError> {
    if !(1..=ACTIVITY_MAX_LIMIT).contains(&limit) {
        return Err(AppError::validation(format!(
            "Số mục nhật ký phải trong khoảng 1..={ACTIVITY_MAX_LIMIT}."
        )));
    }

    let alerts = sqlx::query(
        "SELECT ca.manga_id, m.title AS manga_title, c.title AS chapter_title, ca.detected_at \
         FROM chapter_alerts ca \
         JOIN chapters c ON c.id = ca.chapter_id \
         JOIN mangas m ON m.id = ca.manga_id \
         ORDER BY ca.detected_at DESC, ca.chapter_id DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let tasks = sqlx::query(
        "SELECT t.status, c.manga_id, m.title AS manga_title, c.title AS chapter_title, \
                COALESCE(t.finished_at, t.updated_at) AS at, t.error_code \
         FROM download_tasks t \
         JOIN chapters c ON c.id = t.chapter_id \
         JOIN mangas m ON m.id = c.manga_id \
         WHERE t.status IN ('completed', 'failed') \
         ORDER BY COALESCE(t.finished_at, t.updated_at) DESC, t.id DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut items: Vec<ActivityItemDto> = Vec::with_capacity(alerts.len() + tasks.len());
    for row in alerts {
        items.push(ActivityItemDto {
            kind: "alert".to_string(),
            manga_id: row.try_get("manga_id")?,
            manga_title: row.try_get("manga_title")?,
            chapter_title: row.try_get("chapter_title")?,
            at: row.try_get("detected_at")?,
            code: None,
        });
    }
    for row in tasks {
        let status: String = row.try_get("status")?;
        items.push(ActivityItemDto {
            kind: if status == "failed" {
                "failed".to_string()
            } else {
                "downloaded".to_string()
            },
            manga_id: row.try_get("manga_id")?,
            manga_title: row.try_get("manga_title")?,
            chapter_title: row.try_get("chapter_title")?,
            at: row.try_get("at")?,
            code: row.try_get("error_code")?,
        });
    }
    // Two sources, one ordered feed: the timestamps are the same ISO format the
    // rest of the app writes, so lexical order is chronological.
    items.sort_by(|a, b| b.at.cmp(&a.at));
    items.truncate(limit as usize);
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};

    async fn seed(pool: &SqlitePool) -> (String, Vec<String>) {
        ensure_source(pool, "src-a", "Source A").await.unwrap();
        let (manga, _) = upsert_manga(pool, "src-a", "m1", "Truyện A", None, None, "ongoing")
            .await
            .unwrap();
        let mut chapters = Vec::new();
        for number in 1..=3 {
            let (chapter, _) = upsert_chapter(
                pool,
                &manga,
                &format!("c{number}"),
                &format!("Chương {number}"),
                Some(number as f64),
                None,
            )
            .await
            .unwrap();
            chapters.push(chapter);
        }
        (manga, chapters)
    }

    async fn task(
        pool: &SqlitePool,
        id: &str,
        chapter: &str,
        status: &str,
        at: &str,
        code: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, \
             completed_pages, total_pages, attempts, error_code, finished_at, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'cbz', 0, 1, 1, 0, ?4, ?5, ?5, ?5)",
        )
        .bind(id)
        .bind(chapter)
        .bind(status)
        .bind(code)
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn feed_merges_alerts_and_download_outcomes_newest_first() {
        let (_dir, pool) = crate::db::test_pool().await;
        let (manga, chapters) = seed(&pool).await;

        task(
            &pool,
            "t-ok",
            &chapters[0],
            "completed",
            "2026-10-26T02:00:00.000Z",
            None,
        )
        .await;
        task(
            &pool,
            "t-bad",
            &chapters[1],
            "failed",
            "2026-10-26T01:00:00.000Z",
            Some("SOURCE_NOT_FOUND"),
        )
        .await;
        // Queued/paused rows are not history and must not show up.
        task(
            &pool,
            "t-queued",
            &chapters[2],
            "queued",
            "2026-10-26T03:00:00.000Z",
            None,
        )
        .await;
        sqlx::query(
            "INSERT INTO chapter_alerts (manga_id, chapter_id, detected_at) VALUES (?1, ?2, ?3)",
        )
        .bind(&manga)
        .bind(&chapters[2])
        .bind("2026-10-26T04:00:00.000Z")
        .execute(&pool)
        .await
        .unwrap();

        let feed = recent(&pool, 20).await.unwrap();
        let kinds: Vec<&str> = feed.iter().map(|item| item.kind.as_str()).collect();
        assert_eq!(kinds, vec!["alert", "downloaded", "failed"], "newest first");
        assert_eq!(feed[0].chapter_title, "Chương 3");
        assert_eq!(feed[2].code.as_deref(), Some("SOURCE_NOT_FOUND"));
        assert!(feed.iter().all(|item| item.manga_id == manga));

        let capped = recent(&pool, 1).await.unwrap();
        assert_eq!(capped.len(), 1);
        assert_eq!(capped[0].kind, "alert");

        assert!(recent(&pool, 0).await.is_err());
        assert!(recent(&pool, ACTIVITY_MAX_LIMIT + 1).await.is_err());
    }
}
