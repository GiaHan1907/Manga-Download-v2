use sqlx::SqlitePool;

use crate::db::repositories::favorite_repo;
use crate::domain::{validate_id, validate_page, MangaDto, PagedResult};
use crate::error::AppError;

pub async fn set_favorite(
    pool: &SqlitePool,
    manga_id: &str,
    favorite: bool,
) -> Result<bool, AppError> {
    validate_id(manga_id, "mangaId")?;
    favorite_repo::set_favorite(pool, manga_id, favorite).await
}

pub async fn list_favorites(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<PagedResult<MangaDto>, AppError> {
    validate_page(limit, offset)?;
    let (items, total) = favorite_repo::list_favorites(pool, limit, offset).await?;
    Ok(PagedResult {
        items,
        total,
        limit,
        offset,
    })
}
