//! Feature plan F1 — watcher commands. All polling goes through the shared
//! `perform_tick` (same code path as the background loop): candidate
//! selection, adapter pacing, per-manga backoff, and ONE aggregate native
//! notification + `library-alerts` event per tick — never per chapter.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::app_state::AppState;
use crate::error::AppError;
use crate::services::{settings_service, watch_service};

/// Manual checks are rate-limited so a clicking user cannot outpace the
/// per-host pacing the adapters enforce. Monotonic clock (`Instant`) —
/// wall-clock steps/NTP must never widen or shrink the window.
const MANUAL_MIN_INTERVAL: Duration = Duration::from_secs(60);
static LAST_MANUAL: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchTickDto {
    pub checked: usize,
    pub alerted_chapters: usize,
    pub failed: usize,
}

/// One watcher pass; emits + notifies when new chapters appeared.
/// `force` bypasses the per-manga due window — used by the manual
/// `watch_run_now` (rate-limited below) so "check now" really checks now.
pub async fn perform_tick(
    handle: &tauri::AppHandle,
    state: &AppState,
    force: bool,
) -> Result<WatchTickDto, AppError> {
    let settings = settings_service::load(&state.db).await?;
    let registry = {
        let guard = state.registry.lock().map_err(|_| AppError::Internal)?;
        guard.clone()
    };
    let report = watch_service::run_tick(
        &state.db,
        &registry,
        settings.watch_enabled,
        watch_service::WATCH_TICK_LIMIT,
        force,
    )
    .await?;
    if report.alerted_chapters > 0 {
        use tauri_plugin_notification::NotificationExt;
        if state.flags.notify_enabled() {
            let _ = handle
                .notification()
                .builder()
                .title("Truyện có chương mới")
                .body(format!(
                    "{} chương mới từ các truyện đang theo dõi.",
                    report.alerted_chapters
                ))
                .show();
        }
        let _ = handle.emit(
            "library-alerts",
            serde_json::json!({ "newChapters": report.alerted_chapters }),
        );
    }
    Ok(WatchTickDto {
        checked: report.checked,
        alerted_chapters: report.alerted_chapters,
        failed: report.failed,
    })
}

#[tauri::command]
pub async fn watch_run_now(
    state: tauri::State<'_, AppState>,
    handle: tauri::AppHandle,
) -> Result<WatchTickDto, AppError> {
    {
        // Hold the lock only for the check-and-stamp (no await inside), so
        // two concurrent clicks serialize and exactly one wins.
        let mut last = LAST_MANUAL.lock().map_err(|_| AppError::Internal)?;
        if let Some(previous) = *last {
            if previous.elapsed() < MANUAL_MIN_INTERVAL {
                return Err(AppError::Conflict);
            }
        }
        *last = Some(Instant::now());
    }
    perform_tick(&handle, state.inner(), true).await
}

#[tauri::command]
pub async fn library_set_watch(
    manga_id: String,
    enabled: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    watch_service::set_watch_enabled(&state.db, &manga_id, enabled).await
}

#[tauri::command]
pub async fn library_list_alerts(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    watch_service::list_alerts(&state.db, &state.app_data_dir, &manga_id).await
}

#[tauri::command]
pub async fn library_ack_alerts(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<i64, AppError> {
    watch_service::ack_alerts(&state.db, &manga_id).await
}

/// Current watcher state for one manga (drives the Library-detail toggle;
/// `enabled: None` means "follow the global setting").
#[tauri::command]
pub async fn library_get_watch(
    manga_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    crate::domain::validate_id(&manga_id, "mangaId")?;
    let row: Option<(Option<i64>, Option<String>, i64)> = sqlx::query_as(
        "SELECT watch_enabled, watched_at, watch_fail_count FROM mangas WHERE id = ?1",
    )
    .bind(&manga_id)
    .fetch_optional(&state.db)
    .await?;
    let (enabled, watched_at, fail_count) = row.ok_or_else(|| AppError::not_found("manga"))?;
    Ok(serde_json::json!({
        "enabled": enabled.map(|v| v != 0),
        "watchedAt": watched_at,
        "failCount": fail_count,
    }))
}

/// Background loop: first pass shortly after startup, then one per interval.
/// Errors are logged only — a failing tick must never affect the app.
pub fn spawn_watcher_loop(app: &tauri::AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut first = true;
        loop {
            let delay = if first {
                first = false;
                Duration::from_secs(90)
            } else {
                Duration::from_secs(watch_service::WATCH_INTERVAL_HOURS as u64 * 3600)
            };
            tokio::time::sleep(delay).await;
            let state = handle.state::<AppState>();
            match perform_tick(&handle, &state, false).await {
                Ok(report) if report.checked > 0 => {
                    crate::app_log!(
                        "watch tick: {} checked, {} new chapters, {} failed",
                        report.checked,
                        report.alerted_chapters,
                        report.failed
                    );
                }
                Err(error) => crate::app_log!("watch tick skipped: {}", error.wire_pair().0),
                _ => {}
            }
        }
    });
}
