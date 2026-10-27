use tauri::State;

use crate::app_state::AppState;
use crate::error::AppError;
use crate::services::activity_service::{self, ActivityItemDto};

/// Read-only merged feed: watcher alerts + finished/failed downloads.
#[tauri::command]
pub async fn app_list_recent_activity(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<ActivityItemDto>, AppError> {
    activity_service::recent(
        &state.db,
        limit.unwrap_or(activity_service::ACTIVITY_DEFAULT_LIMIT),
    )
    .await
}
