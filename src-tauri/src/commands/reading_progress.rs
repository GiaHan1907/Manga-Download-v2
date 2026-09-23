use crate::app_state::AppState;
use crate::db::repositories::history_repo;
use crate::domain::{PagedResult, ReadHistoryDto, ReadingProgressDto};
use crate::error::AppError;
use crate::services::reading_service;

#[tauri::command]
pub async fn app_get_reading_progress(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<ReadingProgressDto>, AppError> {
    reading_service::get_progress(&state.db, &manga_id).await
}

#[tauri::command]
pub async fn app_save_reading_progress(
    manga_id: String,
    chapter_id: String,
    page_index: i64,
    scroll_offset: Option<f64>,
    page_total: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<ReadingProgressDto, AppError> {
    reading_service::save_progress(
        &state.db,
        &manga_id,
        &chapter_id,
        page_index,
        scroll_offset,
        page_total,
    )
    .await
}

/// Feature plan F4 — per-chapter read history, newest first. `mangaId`
/// scopes to one series; omit for the global stream (Home).
#[tauri::command]
pub async fn app_list_read_history(
    manga_id: Option<String>,
    limit: i64,
    offset: i64,
    state: tauri::State<'_, AppState>,
) -> Result<PagedResult<ReadHistoryDto>, AppError> {
    history_repo::list(&state.db, manga_id.as_deref(), limit, offset).await
}

/// Feature plan F4 — wipe one manga's read history (metadata only; never
/// touches files, progress rows, favorites or downloads).
#[tauri::command]
pub async fn app_clear_read_history(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<i64, AppError> {
    history_repo::clear(&state.db, &manga_id).await
}
