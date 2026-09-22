use serde::Serialize;
use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::error::AppError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppHealth {
    pub status: String,
    pub database: String,
    /// Highest applied SQLx migration version (0 only if history is empty).
    pub schema_version: i64,
}

#[tauri::command]
pub async fn app_get_info(app: AppHandle) -> Result<AppInfo, AppError> {
    let package_info = app.package_info();
    let name = package_info.name.clone();
    let version = package_info.version.to_string();
    let platform = std::env::consts::OS.to_string();

    Ok(AppInfo {
        name,
        version,
        platform,
    })
}

#[tauri::command]
pub async fn app_health_check(state: State<'_, AppState>) -> Result<AppHealth, AppError> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(AppError::DatabaseInit)?;

    let schema_version = crate::db::schema_version(&state.db).await?;

    Ok(AppHealth {
        status: "ok".to_string(),
        database: "ready".to_string(),
        schema_version,
    })
}
