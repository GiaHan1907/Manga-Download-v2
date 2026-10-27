use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::app_state::AppState;
use crate::error::AppError;
use crate::services::backup_service::{self, BackupSummaryDto};

fn picked_path(picked: Option<FilePath>) -> Result<Option<std::path::PathBuf>, AppError> {
    match picked {
        None => Ok(None),
        Some(FilePath::Path(path)) => Ok(Some(path)),
        Some(FilePath::Url(url)) => url
            .to_file_path()
            .map(Some)
            .map_err(|_| AppError::validation("Đường dẫn file không hợp lệ.")),
    }
}

/// Pick a destination and write the metadata backup there. `None` = the user
/// cancelled, which is not an error.
#[tauri::command]
pub async fn app_export_library_backup(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<BackupSummaryDto>, AppError> {
    let picked = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Lưu bản sao lưu thư viện")
            .set_file_name("manga-downloader-1989-backup.json")
            .add_filter("JSON", &["json"])
            .blocking_save_file()
    })
    .await
    .map_err(|_| AppError::Internal)?;
    let Some(path) = picked_path(picked)? else {
        return Ok(None);
    };
    backup_service::export_to(&state.db, &path).await.map(Some)
}

/// Pick a backup file and restore from it. `None` = cancelled.
#[tauri::command]
pub async fn app_import_library_backup(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<BackupSummaryDto>, AppError> {
    let picked = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Chọn file sao lưu để khôi phục")
            .add_filter("JSON", &["json"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| AppError::Internal)?;
    let Some(path) = picked_path(picked)? else {
        return Ok(None);
    };
    backup_service::import_from(&state.db, &path)
        .await
        .map(Some)
}
