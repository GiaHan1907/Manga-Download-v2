use crate::app_state::AppState;
use crate::domain::{MangaDto, PagedResult};
use crate::error::AppError;
use crate::services::favorites_service;

#[tauri::command]
pub async fn app_set_manga_favorite(
    manga_id: String,
    favorite: bool,
    state: tauri::State<'_, AppState>,
) -> Result<bool, AppError> {
    favorites_service::set_favorite(&state.db, &manga_id, favorite).await
}

#[tauri::command]
pub async fn app_list_favorite_mangas(
    limit: Option<i64>,
    offset: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<PagedResult<MangaDto>, AppError> {
    favorites_service::list_favorites(
        &state.db,
        limit.unwrap_or(crate::domain::DEFAULT_PAGE_LIMIT),
        offset.unwrap_or(0),
    )
    .await
}
