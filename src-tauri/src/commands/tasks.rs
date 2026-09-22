use crate::app_state::AppState;
use crate::db::repositories::task_repo;
use crate::domain::{DownloadTaskDto, PagedResult};
use crate::error::AppError;

/// Read-only task metadata (Phase 06 adds the scheduler that writes it).
#[tauri::command]
pub async fn app_list_local_download_tasks(
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<PagedResult<DownloadTaskDto>, AppError> {
    let limit = limit.unwrap_or(crate::domain::DEFAULT_PAGE_LIMIT);
    let offset = offset.unwrap_or(0);
    crate::domain::validate_page(limit, offset)?;
    let (items, total) = task_repo::list_tasks(&state.db, status.as_deref(), limit, offset).await?;
    Ok(PagedResult {
        items,
        total,
        limit,
        offset,
    })
}
