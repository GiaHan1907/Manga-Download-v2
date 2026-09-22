use crate::app_state::AppState;
use crate::domain::ReaderManifestDto;
use crate::error::AppError;
use crate::services::reader_service;

#[tauri::command]
pub async fn reader_open_chapter(
    chapter_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ReaderManifestDto, AppError> {
    reader_service::open_chapter(&state.db, &state.app_data_dir, &chapter_id).await
}

/// Binary page delivery (Tauri v2 ipc::Response): the JS side receives an
/// ArrayBuffer — no base64, no whole-chapter payloads, no paths.
#[tauri::command]
pub async fn reader_get_page_bytes(
    chapter_id: String,
    page_index: usize,
    state: tauri::State<'_, AppState>,
) -> Result<tauri::ipc::Response, AppError> {
    let bytes =
        reader_service::page_bytes(&state.db, &state.app_data_dir, &chapter_id, page_index).await?;
    Ok(tauri::ipc::Response::new(bytes))
}
