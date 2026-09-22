use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::domain::{now_iso, validate_id, ReadingProgressDto};
use crate::error::AppError;

pub async fn get_progress(
    pool: &SqlitePool,
    manga_id: &str,
) -> Result<Option<ReadingProgressDto>, AppError> {
    let row = sqlx::query("SELECT manga_id, chapter_id, page_index, updated_at FROM reading_progress WHERE manga_id = ?1")
        .bind(manga_id)
        .fetch_optional(pool)
        .await?;

    row.map(|r| {
        Ok(ReadingProgressDto {
            manga_id: r.try_get("manga_id")?,
            chapter_id: r.try_get("chapter_id")?,
            page_index: r.try_get("page_index")?,
            updated_at: r.try_get("updated_at")?,
        })
    })
    .transpose()
}

/// A plain FK cannot express "chapter belongs to this manga"; enforce the pair
/// inside one transaction so a rejected write leaves prior progress intact.
pub async fn save_progress(
    pool: &SqlitePool,
    manga_id: &str,
    chapter_id: &str,
    page_index: i64,
) -> Result<ReadingProgressDto, AppError> {
    validate_id(manga_id, "mangaId")?;
    validate_id(chapter_id, "chapterId")?;
    if page_index < 0 {
        return Err(AppError::validation("pageIndex phải không âm."));
    }

    let mut tx = pool.begin().await?;
    let result = save_progress_tx(tx.as_mut(), manga_id, chapter_id, page_index).await;
    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

async fn save_progress_tx(
    conn: &mut SqliteConnection,
    manga_id: &str,
    chapter_id: &str,
    page_index: i64,
) -> Result<ReadingProgressDto, AppError> {
    let owned: Option<(String,)> =
        sqlx::query_as("SELECT id FROM chapters WHERE id = ?1 AND manga_id = ?2")
            .bind(chapter_id)
            .bind(manga_id)
            .fetch_optional(&mut *conn)
            .await?;
    if owned.is_none() {
        return Err(AppError::validation("chapter không thuộc manga này."));
    }

    let ts = now_iso();
    sqlx::query(
        "INSERT INTO reading_progress (manga_id, chapter_id, page_index, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(manga_id) DO UPDATE SET chapter_id = excluded.chapter_id, page_index = excluded.page_index, updated_at = excluded.updated_at",
    )
    .bind(manga_id)
    .bind(chapter_id)
    .bind(page_index)
    .bind(&ts)
    .execute(&mut *conn)
    .await?;

    Ok(ReadingProgressDto {
        manga_id: manga_id.to_string(),
        chapter_id: chapter_id.to_string(),
        page_index,
        updated_at: ts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};

    async fn fixture() -> (tempfile::TempDir, SqlitePool, String, String) {
        let (dir, pool) = crate::db::test_pool().await;
        ensure_source(&pool, "src-a", "Source A").await.unwrap();
        let (manga_id, _) = upsert_manga(&pool, "src-a", "m1", "T", None, None, "ongoing")
            .await
            .unwrap();
        let (chapter_id, _) = upsert_chapter(&pool, &manga_id, "c-1", "C1", Some(1.0), Some(5))
            .await
            .unwrap();
        (dir, pool, manga_id, chapter_id)
    }

    #[tokio::test]
    async fn progress_roundtrip_and_overwrite() {
        let (_dir, pool, manga_id, chapter_id) = fixture().await;
        let saved = save_progress(&pool, &manga_id, &chapter_id, 3)
            .await
            .unwrap();
        assert_eq!(saved.page_index, 3);
        let loaded = get_progress(&pool, &manga_id).await.unwrap().unwrap();
        assert_eq!(loaded.chapter_id, chapter_id);
        assert_eq!(loaded.page_index, 3);

        save_progress(&pool, &manga_id, &chapter_id, 4)
            .await
            .unwrap();
        let loaded = get_progress(&pool, &manga_id).await.unwrap().unwrap();
        assert_eq!(loaded.page_index, 4);
    }

    #[tokio::test]
    async fn t04_07_wrong_pairing_and_negative_index_rejected_existing_intact() {
        let (_dir, pool, manga_id, chapter_id) = fixture().await;
        save_progress(&pool, &manga_id, &chapter_id, 2)
            .await
            .unwrap();

        let (manga_b, _) = upsert_manga(&pool, "src-a", "m2", "T2", None, None, "ongoing")
            .await
            .unwrap();
        let error = save_progress(&pool, &manga_b, &chapter_id, 1)
            .await
            .unwrap_err();
        assert!(
            matches!(error, AppError::Validation(_)),
            "cross-manga pairing must be rejected"
        );

        let existing = get_progress(&pool, &manga_id).await.unwrap().unwrap();
        assert_eq!(
            existing.page_index, 2,
            "rejected write must not corrupt progress"
        );

        assert!(save_progress(&pool, &manga_id, &chapter_id, -1)
            .await
            .is_err());
        assert!(save_progress(&pool, "bad id!", &chapter_id, 1)
            .await
            .is_err());
    }
}
