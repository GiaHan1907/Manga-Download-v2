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

/// Open a downloaded chapter's artifact with whatever the OS associates with
/// it. This is the way out for a PDF volume (the in-app reader draws page
/// images from CBZ/folders only) and a convenience for any chapter already on
/// disk — including image folders. The path is resolved in Rust from the DB
/// and re-validated against the library root; no path crosses the IPC boundary.
#[tauri::command]
pub async fn open_chapter_file(
    chapter_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    let artifact =
        reader_service::openable_artifact(&state.db, &state.app_data_dir, &chapter_id).await?;
    crate::commands::downloads::spawn_shell_open(&artifact, false)
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
