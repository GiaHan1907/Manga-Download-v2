//! Discover Phase 2 — recent search queries. Every write returns the fresh
//! list so the renderer never has to guess what the table now holds.

use crate::app_state::AppState;
use crate::db::repositories::search_history_repo;
use crate::domain::RecentSearchDto;
use crate::error::AppError;

#[tauri::command]
pub async fn search_history_record(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RecentSearchDto>, AppError> {
    search_history_repo::record(&state.db, &query).await
}

#[tauri::command]
pub async fn search_history_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RecentSearchDto>, AppError> {
    search_history_repo::list(&state.db).await
}

#[tauri::command]
pub async fn search_history_remove(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RecentSearchDto>, AppError> {
    search_history_repo::remove(&state.db, &query).await
}

#[tauri::command]
pub async fn search_history_clear(state: tauri::State<'_, AppState>) -> Result<i64, AppError> {
    search_history_repo::clear(&state.db).await
}
