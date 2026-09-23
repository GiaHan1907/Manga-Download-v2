//! Feature plan F4 — per-chapter read history queries. The write path lives
//! in `progress_repo::save_progress_tx` (same transaction as progress);
//! this repo owns listing and the user-initiated per-manga wipe.

use sqlx::SqlitePool;

use crate::domain::{now_iso, PagedResult, ReadHistoryDto};
use crate::error::AppError;

pub const MAX_PAGE: i64 = 100;

pub async fn list(
    pool: &SqlitePool,
    manga_id: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<PagedResult<ReadHistoryDto>, AppError> {
    let limit = limit.clamp(1, MAX_PAGE);
    crate::domain::validate_page(limit, offset)?;
    if let Some(id) = manga_id {
        crate::domain::validate_id(id, "mangaId")?;
    }
    let (total,): (i64,) = if let Some(id) = manga_id {
        sqlx::query_as("SELECT COUNT(*) FROM read_history WHERE manga_id = ?1")
            .bind(id)
            .fetch_one(pool)
            .await?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM read_history")
            .fetch_one(pool)
            .await?
    };
    let rows = sqlx::query(
        "SELECT h.chapter_id, h.manga_id, h.read_at, c.title AS chapter_title, c.number AS chapter_number, \
                m.title AS manga_title \
         FROM read_history h \
         JOIN chapters c ON c.id = h.chapter_id \
         JOIN mangas m ON m.id = h.manga_id \
         WHERE ($1 IS NULL OR h.manga_id = $1) \
         ORDER BY h.read_at DESC, h.chapter_id DESC LIMIT $2 OFFSET $3",
    )
    .bind(manga_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    use sqlx::Row;
    let items = rows
        .iter()
        .map(|row| ReadHistoryDto {
            manga_id: row.try_get("manga_id").unwrap_or_default(),
            manga_title: row.try_get("manga_title").unwrap_or_default(),
            chapter_id: row.try_get("chapter_id").unwrap_or_default(),
            chapter_title: row.try_get("chapter_title").unwrap_or_default(),
            chapter_number: row.try_get("chapter_number").unwrap_or_default(),
            read_at: row.try_get("read_at").unwrap_or_default(),
        })
        .collect();
    Ok(PagedResult {
        items,
        total,
        limit,
        offset,
    })
}

/// User-initiated wipe of one manga's history rows (idempotent). Progress,
/// favorites, downloads and files are untouched by design.
pub async fn clear(pool: &SqlitePool, manga_id: &str) -> Result<i64, AppError> {
    crate::domain::validate_id(manga_id, "mangaId")?;
    let changed = sqlx::query("DELETE FROM read_history WHERE manga_id = ?1")
        .bind(manga_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed as i64)
}

/// UPSERT inside the caller's transaction: latest read wins, one row per chapter.
pub async fn mark_read_tx(
    conn: &mut sqlx::SqliteConnection,
    manga_id: &str,
    chapter_id: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO read_history (manga_id, chapter_id, read_at) VALUES (?1, ?2, ?3) \
         ON CONFLICT(manga_id, chapter_id) DO UPDATE SET read_at = excluded.read_at",
    )
    .bind(manga_id)
    .bind(chapter_id)
    .bind(now_iso())
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};
    use crate::db::test_pool;

    async fn fixture() -> (tempfile::TempDir, SqlitePool, String, String) {
        let (dir, pool) = test_pool().await;
        ensure_source(&pool, "src-a", "A").await.unwrap();
        let (manga_id, _) = upsert_manga(&pool, "src-a", "m1", "T", None, None, "ongoing")
            .await
            .unwrap();
        let (chapter_id, _) = upsert_chapter(&pool, &manga_id, "c-1", "C1", Some(1.5), Some(5))
            .await
            .unwrap();
        (dir, pool, manga_id, chapter_id)
    }

    #[tokio::test]
    async fn t07_history_mark_list_clear_and_completion_gate() {
        let (_dir, pool, manga_id, chapter_id) = fixture().await;
        // Not read yet: mid-chapter progress saves nothing.
        crate::db::repositories::progress_repo::save_progress(
            &pool,
            &manga_id,
            &chapter_id,
            1,
            None,
            Some(5),
        )
        .await
        .unwrap();
        assert_eq!(list(&pool, None, 10, 0).await.unwrap().total, 0);
        // Reached the last pages → recorded once; re-saving updates, never duplicates.
        crate::db::repositories::progress_repo::save_progress(
            &pool,
            &manga_id,
            &chapter_id,
            3,
            None,
            Some(5),
        )
        .await
        .unwrap();
        crate::db::repositories::progress_repo::save_progress(
            &pool,
            &manga_id,
            &chapter_id,
            4,
            None,
            Some(5),
        )
        .await
        .unwrap();
        let page = list(&pool, Some(&manga_id), 10, 0).await.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].chapter_number, Some(1.5));
        assert_eq!(clear(&pool, &manga_id).await.unwrap(), 1);
        assert_eq!(list(&pool, None, 10, 0).await.unwrap().total, 0);
        assert_eq!(clear(&pool, &manga_id).await.unwrap(), 0, "idempotent");
    }
}
