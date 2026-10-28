use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::error::AppError;

/// What the Settings panel needs to decide between "up to date" and
/// "install vX". `current` always comes from the running binary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckDto {
    pub current: String,
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
}

async fn pending(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, AppError> {
    let updater = app.updater().map_err(|error| {
        crate::app_log!("updater unavailable: {error}");
        AppError::validation("Không khởi tạo được trình cập nhật.")
    })?;
    updater.check().await.map_err(|error| {
        // Details go to the log file; the UI gets one actionable sentence.
        crate::app_log!("update check failed: {error}");
        AppError::validation("Không kiểm tra được bản cập nhật (mạng hoặc GitHub).")
    })
}

#[tauri::command]
pub async fn app_check_update(app: AppHandle) -> Result<UpdateCheckDto, AppError> {
    let current = app.package_info().version.to_string();
    Ok(match pending(&app).await? {
        Some(update) => UpdateCheckDto {
            current,
            available: true,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
        },
        None => UpdateCheckDto {
            current,
            available: false,
            version: None,
            notes: None,
        },
    })
}

/// Download, verify (the plugin checks the signature against the embedded
/// public key) and run the installer, then restart into the new version.
#[tauri::command]
pub async fn app_install_update(app: AppHandle) -> Result<(), AppError> {
    let Some(update) = pending(&app).await? else {
        return Ok(());
    };
    crate::app_log!("installing update {}", update.version);
    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|error| {
            crate::app_log!("update install failed: {error}");
            AppError::validation("Tải hoặc cài bản cập nhật thất bại — thử lại sau.")
        })?;
    app.restart();
}
