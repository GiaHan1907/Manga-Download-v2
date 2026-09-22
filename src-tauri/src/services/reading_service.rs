use sqlx::SqlitePool;

use crate::db::repositories::progress_repo;
use crate::domain::{validate_id, ReadingProgressDto};
use crate::error::AppError;

pub async fn get_progress(
    pool: &SqlitePool,
    manga_id: &str,
) -> Result<Option<ReadingProgressDto>, AppError> {
    validate_id(manga_id, "mangaId")?;
    progress_repo::get_progress(pool, manga_id).await
}

pub async fn save_progress(
    pool: &SqlitePool,
    manga_id: &str,
    chapter_id: &str,
    page_index: i64,
) -> Result<ReadingProgressDto, AppError> {
    if page_index > 10_000 {
        return Err(AppError::validation("pageIndex vượt giới hạn hợp lệ."));
    }
    progress_repo::save_progress(pool, manga_id, chapter_id, page_index).await
}
