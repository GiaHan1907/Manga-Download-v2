use std::sync::Arc;

use tauri::Emitter;

use crate::app_state::{AppState, RuntimeFlags};
use crate::db::repositories::{download_repo, task_repo};
use crate::domain::{
    now_iso, ClearHistoryDto, DownloadTaskDto, DownloadTaskEvent, QueueErrorDto, QueueResultDto,
};
use crate::downloader::progress::TaskSink;
use crate::error::AppError;
use crate::services::{discovery_service, download_history_service, settings_service};
use crate::sources::dto::Capability;

/// Production sink: forwards engine events to the renderer on a fixed
/// channel. Payload carries only persisted-safe DTO fields (design §8).
/// Chapter-terminal transitions additionally raise a native notification
/// (Phase 08) when the user enabled them — one per task completion/failure,
/// never per image, and never carrying paths or URLs.
pub struct TauriEventSink {
    handle: tauri::AppHandle,
    flags: Arc<RuntimeFlags>,
}

impl TauriEventSink {
    pub fn new(handle: tauri::AppHandle, flags: Arc<RuntimeFlags>) -> Self {
        TauriEventSink { handle, flags }
    }
}

impl TaskSink for TauriEventSink {
    fn publish(&self, event: DownloadTaskEvent) {
        if self.flags.notify_enabled() {
            notify_terminal(&self.handle, &event.task);
        }
        let _ = self.handle.emit("download-task-updated", event);
    }
}

fn notify_terminal(handle: &tauri::AppHandle, task: &DownloadTaskDto) {
    let (title, body) = match task.status.as_str() {
        "completed" => (
            "Download hoàn tất",
            format!(
                "{} — {}",
                task.manga_title.clone().unwrap_or_default(),
                task.chapter_title.clone().unwrap_or_default()
            ),
        ),
        "failed" => (
            "Download thất bại",
            format!(
                "{} — {}: {}",
                task.manga_title.clone().unwrap_or_default(),
                task.chapter_title.clone().unwrap_or_default(),
                task.error_message.as_deref().unwrap_or("lỗi không rõ")
            ),
        ),
        _ => return,
    };
    use tauri_plugin_notification::NotificationExt;
    let _ = handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
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
    submit_chapter_ids(&state, chapter_ids, format, priority).await
}

/// Shared queue path for the id-batch command AND the paste-a-link command,
/// so capability gating, format validation and dedup semantics can never
/// drift between entry points.
async fn submit_chapter_ids(
    state: &AppState,
    chapter_ids: Vec<String>,
    format: Option<String>,
    priority: Option<i64>,
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

/// Paste-a-chapter-link quick download. The renderer sends a URL the USER
/// typed/pasted (same documented exception class as `resolve_manga_url`);
/// Rust resolves it through registered adapters only (exact host + the
/// adapter's path grammar), saves the manga through the normal discovery
/// path if it is not in the library yet, then queues the chapter through
/// the SAME gate as `queue_chapter_downloads`. No URL ever returns; the
/// response is the standard path-free QueueResultDto.
#[tauri::command]
pub async fn queue_chapter_download_by_url(
    url: String,
    format: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<QueueResultDto, AppError> {
    let registry = {
        let guard = state.registry.lock().map_err(|_| AppError::Internal)?;
        guard.clone()
    };
    let link = discovery_service::find_chapter_link(&registry, &url)?;
    let chapter_id = discovery_service::ensure_chapter_row(&registry, &state.db, &link).await?;
    submit_chapter_ids(&state, vec![chapter_id], format, None).await
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

/// Feature plan F2 — purge download-history rows (terminal states only).
/// Completed tasks whose verified artifact still exists on disk are skipped
/// by design; no file is ever deleted here (that is Library's job).
#[tauri::command]
pub async fn clear_download_history(
    statuses: Option<Vec<String>>,
    older_than_days: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<ClearHistoryDto, AppError> {
    download_history_service::clear(&state.db, &state.app_data_dir, statuses, older_than_days).await
}

/// Feature plan F5 — bulk-requeue every `failed` task (FIFO, capped).
/// Each row goes through the same guarded retry transition as a single
/// retry, so races with the dispatcher or a concurrent cancel are safe.
#[tauri::command]
pub async fn retry_all_failed_downloads(
    state: tauri::State<'_, AppState>,
) -> Result<i64, AppError> {
    const CAP: usize = 200;
    let ids = download_repo::retryable_failed_ids(&state.db, CAP).await?;
    let mut requeued = 0i64;
    for id in ids {
        if state.downloads.retry(&id).await.is_ok() {
            requeued += 1;
        }
    }
    Ok(requeued)
}

/// Feature plan F5 — request cancellation of every unfinished task owned by
/// one manga. Active workers observe the same cooperative cancel flag as a
/// per-task cancel (the response counts accepted requests, not settled
/// states; the authoritative result arrives via task events).
#[tauri::command]
pub async fn cancel_downloads_for_manga(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<i64, AppError> {
    let ids = download_repo::active_task_ids_for_manga(&state.db, &manga_id).await?;
    let mut accepted = 0i64;
    for id in ids {
        if state.downloads.cancel(&id).await.is_ok() {
            accepted += 1;
        }
    }
    Ok(accepted)
}
