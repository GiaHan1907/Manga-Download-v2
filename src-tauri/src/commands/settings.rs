use crate::app_state::AppState;
use crate::domain::{AppSettings, SettingsPatchInput};
use crate::error::AppError;
use crate::services::settings_service;

#[tauri::command]
pub async fn app_get_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, AppError> {
    settings_service::load(&state.db).await
}

#[tauri::command]
pub async fn app_update_settings(
    input: SettingsPatchInput,
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, AppError> {
    settings_service::apply_patch(&state.db, input).await
}

#[tauri::command]
pub async fn app_reset_settings(
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, AppError> {
    settings_service::reset(&state.db).await
}
