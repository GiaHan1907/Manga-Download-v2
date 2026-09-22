use tauri::Emitter;

use crate::app_state::AppState;
use crate::db::repositories::{download_repo, task_repo};
use crate::domain::{now_iso, DownloadTaskDto, DownloadTaskEvent, QueueErrorDto, QueueResultDto};
use crate::downloader::progress::TaskSink;
use crate::error::AppError;
use crate::services::settings_service;
use crate::sources::dto::Capability;

/// Production sink: forwards engine events to the renderer on a fixed
/// channel. Payload carries only persisted-safe DTO fields (design §8).
pub struct TauriEventSink {
    handle: tauri::AppHandle,
}

impl TauriEventSink {
    pub fn new(handle: tauri::AppHandle) -> Self {
        TauriEventSink { handle }
    }
}

impl TaskSink for TauriEventSink {
    fn publish(&self, event: DownloadTaskEvent) {
        let _ = self.handle.emit("download-task-updated", event);
    }
}

/// Batch submit. The renderer sends chapter ids only — never page URLs,
/// headers or paths. Per-item rejections (unknown chapter, source without a
/// lawful page capability) are reported without failing the batch.
#[tauri::command]
pub async fn queue_chapter_downloads(
    chapter_ids: Vec<String>,
    format: Option<String>,
    priority: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<QueueResultDto, AppError> {
    if chapter_ids.len() > download_repo::MAX_BATCH_SIZE {
        return Err(AppError::validation("Tối đa 100 chương mỗi lần xếp hàng."));
    }
    let format = match format.as_deref() {
        None => {
            let settings = settings_service::load(&state.db).await?;
            if settings.output_format == "folder" {
                "folder".to_string()
            } else {
                "cbz".to_string()
            }
        }
        Some(value @ ("cbz" | "folder")) => value.to_string(),
        Some("pdf") => {
            return Err(AppError::validation(
                "PDF chưa hỗ trợ — hãy chọn CBZ hoặc thư mục ảnh.",
            ))
        }
        Some(_) => return Err(AppError::validation("Định dạng xuất không hợp lệ.")),
    };
    let priority = priority.unwrap_or(0);

    let mut tasks: Vec<DownloadTaskDto> = Vec::new();
    let mut reused: Vec<DownloadTaskDto> = Vec::new();
    let mut errors: Vec<QueueErrorDto> = Vec::new();
    // Resolve chapter ownership first (DB IO without any lock held)…
    let mut contexts: Vec<(String, Option<download_repo::ChapterContext>)> = Vec::new();
    for chapter_id in &chapter_ids {
        match download_repo::chapter_context(&state.db, chapter_id).await {
            Ok(ctx) => contexts.push((chapter_id.clone(), ctx)),
            Err(error) => {
                let (code, message) = error.wire_pair();
                errors.push(QueueErrorDto {
                    chapter_id: chapter_id.clone(),
                    code: code.to_string(),
                    message,
                });
            }
        }
    }
    // …then check capabilities under a short-lived registry lock (no await).
    let mut accepted: Vec<String> = Vec::new();
    {
        let registry = state.registry.lock().map_err(|_| AppError::Internal)?;
        for (chapter_id, ctx) in contexts {
            let Some(ctx) = ctx else {
                errors.push(QueueErrorDto {
                    chapter_id,
                    code: "NOT_FOUND".to_string(),
                    message: "Chương không tồn tại trong thư viện local.".to_string(),
                });
                continue;
            };
            let capability_ok = registry
                .get(&ctx.source_id)
                .map(|adapter| adapter.info().capabilities.contains(&Capability::Pages))
                .unwrap_or(false);
            if capability_ok {
                accepted.push(chapter_id);
            } else {
                errors.push(QueueErrorDto {
                    chapter_id,
                    code: "UNSUPPORTED_OPERATION".to_string(),
                    message: "Nguồn của chương chưa được phép tải trang.".to_string(),
                });
            }
        }
    }
    if !accepted.is_empty() {
        let outcome = download_repo::queue_batch(&state.db, &accepted, &format, priority).await?;
        for row in outcome.created {
            tasks.push(task_repo::to_dto(row));
        }
        for row in outcome.reused {
            reused.push(task_repo::to_dto(row));
        }
        for item in outcome.errors {
            errors.push(QueueErrorDto {
                chapter_id: item.chapter_id,
                code: item.code.to_string(),
                message: item.message,
            });
        }
        state.downloads.notify_new_work();
    }
    Ok(QueueResultDto {
        tasks,
        reused,
        errors,
        queued_at: now_iso(),
    })
}

#[tauri::command]
pub async fn get_download_task(
    task_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<DownloadTaskDto, AppError> {
    let row = download_repo::get_task(&state.db, &task_id)
        .await?
        .ok_or_else(|| AppError::not_found("task"))?;
    Ok(task_repo::to_dto(row))
}

#[tauri::command]
pub async fn pause_download(
    task_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<DownloadTaskDto, AppError> {
    state.downloads.pause(&task_id).await
}

#[tauri::command]
pub async fn resume_download(
    task_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<DownloadTaskDto, AppError> {
    state.downloads.resume(&task_id).await
}

#[tauri::command]
pub async fn cancel_download(
    task_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<DownloadTaskDto, AppError> {
    state.downloads.cancel(&task_id).await
}

#[tauri::command]
pub async fn retry_download(
    task_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<DownloadTaskDto, AppError> {
    state.downloads.retry(&task_id).await
}

#[tauri::command]
pub async fn set_download_priority(
    task_id: String,
    priority: i64,
    state: tauri::State<'_, AppState>,
) -> Result<DownloadTaskDto, AppError> {
    state.downloads.set_priority(&task_id, priority).await
}
