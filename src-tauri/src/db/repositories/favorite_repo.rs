use sqlx::SqlitePool;

use crate::domain::{now_iso, MangaDto};
use crate::error::AppError;

use super::manga_repo::{map_manga, MANGA_SELECT};

/// Idempotent favorite set/unset for a registered manga. Unknown manga ids are
/// rejected so favorites can never dangle.
pub async fn set_favorite(
    pool: &SqlitePool,
    manga_id: &str,
    favorite: bool,
) -> Result<bool, AppError> {
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM mangas WHERE id = ?1")
        .bind(manga_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::not_found("manga"));
    }

    if favorite {
        sqlx::query("INSERT INTO favorites (manga_id, created_at) VALUES (?1, ?2) ON CONFLICT(manga_id) DO NOTHING")
            .bind(manga_id)
            .bind(now_iso())
            .execute(pool)
            .await?;
    } else {
        sqlx::query("DELETE FROM favorites WHERE manga_id = ?1")
            .bind(manga_id)
            .execute(pool)
            .await?;
    }
    Ok(favorite)
}

pub async fn list_favorites(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<MangaDto>, i64), AppError> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites")
        .fetch_one(pool)
        .await?;

    let rows = sqlx::query(&format!(
        "{MANGA_SELECT} JOIN favorites f ON f.manga_id = m.id \
         ORDER BY f.created_at DESC, m.id ASC LIMIT ?1 OFFSET ?2"
    ))
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((
        rows.iter().map(map_manga).collect::<Result<_, _>>()?,
        total.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_manga};

    #[tokio::test]
    async fn t04_06_favorite_roundtrip_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::db::init_pool(dir.path()).await.unwrap();
        ensure_source(&pool, "src-a", "Source A").await.unwrap();
        let (manga_id, _) = upsert_manga(&pool, "src-a", "m1", "T", None, None, "ongoing")
            .await
            .unwrap();

        assert!(set_favorite(&pool, &manga_id, true).await.unwrap());
        // Idempotent: second set does not duplicate.
        assert!(set_favorite(&pool, &manga_id, true).await.unwrap());
        let (items, total) = list_favorites(&pool, 10, 0).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].id, manga_id);

        pool.close().await;
        let reopened = crate::db::init_pool(dir.path()).await.unwrap();
        let (items, total) = list_favorites(&reopened, 10, 0).await.unwrap();
        assert_eq!(total, 1, "favorites must survive reopen (T04-06)");
        assert_eq!(items[0].id, manga_id);

        assert!(!set_favorite(&reopened, &manga_id, false).await.unwrap());
        let (_, total) = list_favorites(&reopened, 10, 0).await.unwrap();
        assert_eq!(total, 0);
    }

    #[tokio::test]
    async fn unknown_manga_favorite_rejected() {
        let (_dir, pool) = crate::db::test_pool().await;
        let error = set_favorite(&pool, "ghost", true).await.unwrap_err();
        assert!(matches!(error, AppError::NotFound(_)));
    }
}
