use crate::app_state::AppState;
use crate::domain::{CacheCleanupDto, StorageStatusDto};
use crate::error::AppError;
use crate::services::storage_service;

/// Approximate size/health of app-managed roots. Display-only numbers —
/// no absolute path or drive layout crosses the IPC boundary.
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
