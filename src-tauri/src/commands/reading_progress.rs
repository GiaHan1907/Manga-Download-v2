use crate::app_state::AppState;
use crate::domain::ReadingProgressDto;
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
