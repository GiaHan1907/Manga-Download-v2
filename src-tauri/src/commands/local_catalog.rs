use crate::app_state::AppState;
use crate::domain::{MangaDetailDto, MangaDto, PagedResult};
use crate::error::AppError;
use crate::services::catalog_service;

#[tauri::command]
pub async fn app_list_local_mangas(
    query: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<PagedResult<MangaDto>, AppError> {
    catalog_service::list_mangas(
        &state.db,
        query,
        limit.unwrap_or(crate::domain::DEFAULT_PAGE_LIMIT),
        offset.unwrap_or(0),
    )
    .await
}

#[tauri::command]
pub async fn app_get_local_manga(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<MangaDetailDto, AppError> {
    catalog_service::get_manga_detail(&state.db, &manga_id).await
}
