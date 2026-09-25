use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::app_state::AppState;
use crate::domain::{CacheCleanupDto, StorageStatusDto};
use crate::error::AppError;
use crate::services::storage_root::{self, RootInfoDto};
use crate::services::storage_service;

/// Approximate size/health of app-managed roots. Display-only numbers —
/// no arbitrary path or drive layout crosses the IPC boundary (the custom
/// download root label is the user's own previously-chosen folder, shown
/// inert as text in Settings only).
#[tauri::command]
pub async fn storage_status(
    state: tauri::State<'_, AppState>,
) -> Result<StorageStatusDto, AppError> {
    storage_service::status(&state.db, &state.app_data_dir).await
}

/// Delete only regenerable cache: staging dirs of terminal/unknown tasks and
/// the covers cache. Verified library artifacts, DB rows and active-task
/// staging/checkpoints are never touched. Idempotent; safe to call twice.
#[tauri::command]
pub async fn clear_regenerable_cache(
    state: tauri::State<'_, AppState>,
) -> Result<CacheCleanupDto, AppError> {
    storage_service::clear_regenerable_cache(&state.db, &state.app_data_dir).await
}

/// Where verified chapters live now: default app-data or a user-chosen
/// folder (junction-backed). `linkOk=false` on custom means the link or the
/// chosen folder went missing — the UI must warn, not show an empty library.
#[tauri::command]
pub async fn get_download_root(state: State<'_, AppState>) -> Result<RootInfoDto, AppError> {
    storage_root::current(&state.db, &state.app_data_dir).await
}

/// Prompt for a folder (dialog runs in Rust) and relocate the verified
/// library into it: copy + byte-verify, then junction. Cancel → `Ok(None)`.
#[tauri::command]
pub async fn choose_download_root(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Option<RootInfoDto>, AppError> {
    let picked = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Chọn thư mục lưu truyện đã tải")
            .blocking_pick_folder()
    })
    .await
    .map_err(|_| AppError::Internal)?;

    let chosen = match picked {
        None => return Ok(None),
        Some(FilePath::Path(path)) => path,
        Some(FilePath::Url(url)) => url
            .to_file_path()
            .map_err(|_| AppError::validation("Đường dẫn thư mục không hợp lệ."))?,
    };
    let info = storage_root::relocate(&state.db, &state.app_data_dir, &chosen).await?;
    Ok(Some(info))
}

/// Copy the library back to app-data and clear the custom root. The user's
/// previously chosen folder is left untouched on disk.
#[tauri::command]
pub async fn reset_download_root(state: State<'_, AppState>) -> Result<RootInfoDto, AppError> {
    storage_root::reset_to_default(&state.db, &state.app_data_dir).await
}
