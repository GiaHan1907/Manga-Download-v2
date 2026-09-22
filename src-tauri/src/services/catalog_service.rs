use sqlx::SqlitePool;

use crate::db::repositories::manga_repo;
use crate::domain::{
    validate_id, validate_page, validate_text_len, MangaDetailDto, MangaDto, PagedResult,
};
use crate::error::AppError;

pub async fn list_mangas(
    pool: &SqlitePool,
    query: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<PagedResult<MangaDto>, AppError> {
    validate_page(limit, offset)?;
    let trimmed = match query.as_deref().map(str::trim) {
        None => None,
        Some("") => None,
        Some(value) => {
            validate_text_len(value, crate::domain::MAX_QUERY_LEN, "query")?;
            Some(value.to_string())
        }
    };

    let (items, total) = manga_repo::list_mangas(pool, trimmed.as_deref(), limit, offset).await?;
    Ok(PagedResult {
        items,
        total,
        limit,
        offset,
    })
}

pub async fn get_manga_detail(
    pool: &SqlitePool,
    manga_id: &str,
) -> Result<MangaDetailDto, AppError> {
    validate_id(manga_id, "mangaId")?;
    let manga = manga_repo::get_manga(pool, manga_id)
        .await?
        .ok_or_else(|| AppError::not_found("manga"))?;
    let chapters = manga_repo::list_chapters(pool, manga_id).await?;
    Ok(MangaDetailDto { manga, chapters })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn oversized_query_and_malformed_ids_rejected() {
        let (_dir, pool) = crate::db::test_pool().await;

        let long_query = "x".repeat(400);
        let error = list_mangas(&pool, Some(long_query), 20, 0)
            .await
            .unwrap_err();
        assert!(matches!(error, crate::error::AppError::Validation(_)));

        assert!(matches!(
            get_manga_detail(&pool, "id with spaces").await.unwrap_err(),
            crate::error::AppError::Validation(_)
        ));
        assert!(matches!(
            get_manga_detail(&pool, "").await.unwrap_err(),
            crate::error::AppError::Validation(_)
        ));
        assert!(matches!(
            list_mangas(&pool, None, 0, 0).await.unwrap_err(),
            crate::error::AppError::Validation(_)
        ));
        assert!(matches!(
            list_mangas(&pool, None, 101, 0).await.unwrap_err(),
            crate::error::AppError::Validation(_)
        ));
    }
}
