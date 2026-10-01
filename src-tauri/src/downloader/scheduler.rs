//! Bounded queue coordinator. Owns the dispatcher loop and the cooperative
//! control surface (pause/resume/cancel/retry/priority) that commands call.
//!
//! Concurrency is capped per settings (`chapterConcurrency` active chapter
//! workers; `imageConcurrency` pages per worker). Database transactions are
//! never held across network IO — every repo call completes before the next
//! await point that performs IO.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::SqlitePool;

use crate::db::repositories::{download_repo, task_repo};
use crate::domain::DownloadTaskDto;
use crate::downloader::progress::ProgressEmitter;
use crate::downloader::{storage, worker};
use crate::error::AppError;
use crate::services::settings_service;
use crate::sources::registry::SourceRegistry;

/// Dispatcher cadence: retry windows and missed wakeups self-heal within a
/// second; wakes from actions make the common path instant.
const TICK: Duration = Duration::from_secs(1);
/// Upper bound on simultaneously live workers even if settings are odd.
const MAX_CHAPTER_WORKERS: i64 = 5;
/// Consecutive chapters that failed because the SOURCE COULD NOT BE REACHED
/// before the queue stops itself. Without this, one network/DPI blip spends the
/// whole retry budget of every queued chapter: a live incident burned 45
/// chapters in five seconds while the source refused connections.
const NETWORK_FAILURE_BREAKER: usize = 3;

struct ActiveWorker {
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}

pub struct Coordinator {
    pool: SqlitePool,
    registry: Arc<Mutex<SourceRegistry>>,
    app_data_dir: std::path::PathBuf,
    events: Arc<ProgressEmitter>,
    wake: tokio::sync::Notify,
    active: Mutex<HashMap<String, ActiveWorker>>,
    /// Shared with every worker: set on app exit. Workers exit WITHOUT
    /// writing a terminal state so startup recovery re-queues them.
    shutdown: Arc<AtomicBool>,
    /// Global queue gate: while set the dispatcher claims nothing new.
    queue_paused: AtomicBool,
    /// Chapters that just failed on an unreachable source, oldest first. Its
    /// length is the streak the breaker counts, and its contents are the rows
    /// handed back to the queue when it trips.
    network_streak: Mutex<Vec<String>>,
}

impl Coordinator {
    pub fn new(
        pool: SqlitePool,
        registry: Arc<Mutex<SourceRegistry>>,
        app_data_dir: std::path::PathBuf,
        events: Arc<ProgressEmitter>,
    ) -> Arc<Self> {
        Arc::new(Coordinator {
            pool,
            registry,
            app_data_dir,
            events,
            wake: tokio::sync::Notify::new(),
            active: Mutex::new(HashMap::new()),
            shutdown: Arc::new(AtomicBool::new(false)),
            queue_paused: AtomicBool::new(false),
            network_streak: Mutex::new(Vec::new()),
        })
    }

    /// Spawns the dispatcher loop. Called once from app setup after startup
    /// recovery has normalized persisted states.
    pub fn start(self: &Arc<Self>) {
        let coordinator = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            loop {
                if coordinator.shutdown.load(Ordering::Relaxed) {
                    break;
                }
                coordinator.dispatch_available().await;
                tokio::select! {
                    () = coordinator.wake.notified() => {}
                    () = tokio::time::sleep(TICK) => {}
                }
            }
        });
    }

    pub fn notify_new_work(&self) {
        self.wake.notify_one();
    }

    /// Whether the global queue gate currently blocks new dispatches.
    pub fn is_queue_paused(&self) -> bool {
        self.queue_paused.load(Ordering::Relaxed)
    }

    /// Global pause/resume. Pause: gate the dispatcher, cooperatively pause
    /// every live worker and bulk-move `queued` → `paused`. Resume: clear
    /// the gate, drop worker pause flags and requeue every `paused` task —
    /// by design this also restarts tasks the user paused individually.
    /// Returns the number of persisted rows moved.
    pub async fn set_queue_paused(&self, paused: bool) -> Result<u32, AppError> {
        self.queue_paused.store(paused, Ordering::Relaxed);
        {
            let active = self.active.lock().unwrap();
            for handle in active.values() {
                handle.pause.store(paused, Ordering::Relaxed);
            }
        }
        let (from, to) = if paused {
            ("queued", "paused")
        } else {
            ("paused", "queued")
        };
        let moved = download_repo::transition_status_all(&self.pool, from, to).await?;
        for row in &moved {
            self.publish_now(row);
        }
        if !paused {
            self.notify_new_work();
        }
        Ok(moved.len() as u32)
    }

    async fn dispatch_available(self: &Arc<Self>) {
        if self.queue_paused.load(Ordering::Relaxed) {
            return;
        }
        let settings = match settings_service::load(&self.pool).await {
            Ok(settings) => settings,
            Err(_) => return,
        };
        let capacity = settings
            .chapter_concurrency
            .clamp(1, MAX_CHAPTER_WORKERS)
            .min(crate::downloader::MAX_TRACKED_TASKS as i64) as usize;
        loop {
            let free = {
                let active = self.active.lock().unwrap();
                capacity.saturating_sub(active.len())
            };
            if free == 0 || self.shutdown.load(Ordering::Relaxed) {
                return;
            }
            let claimed = match download_repo::claim_next(&self.pool).await {
                Ok(claimed) => claimed,
                Err(_) => return,
            };
            let Some(task) = claimed else { return };
            // dispatch already gated capacity, so spawn_worker cannot reject:
            // a claimed task always gets a worker (no `resolving` zombie).
            self.spawn_worker(task, settings.image_concurrency);
        }
    }

    fn spawn_worker(self: &Arc<Self>, task: download_repo::TaskRow, image_concurrency: i64) {
        let cancel = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active.lock().unwrap();
            active.insert(
                task.id.clone(),
                ActiveWorker {
                    cancel: Arc::clone(&cancel),
                    pause: Arc::clone(&pause),
                },
            );
        }
        let deps = Arc::new(worker::WorkerDeps {
            pool: self.pool.clone(),
            registry: Arc::clone(&self.registry_snapshot()),
            app_data_dir: self.app_data_dir.clone(),
            events: Arc::clone(&self.events),
            flags: worker::WorkerFlags {
                cancel: Arc::clone(&cancel),
                pause: Arc::clone(&pause),
                shutdown: Arc::clone(&self.shutdown),
            },
            image_concurrency,
        });
        // Weak handle: worker completion bookkeeping disappears with the
        // coordinator; the worker's own DB effects stay valid regardless.
        let weak = Arc::downgrade(self);
        let task_id = task.id.clone();
        let observed_id = task_id.clone();
        tauri::async_runtime::spawn(async move {
            // RAII cleanup releases the active slot even if the worker future
            // panics mid-await (prevents a leaked worker permit).
            struct SlotGuard {
                coordinator: std::sync::Weak<Coordinator>,
                task_id: String,
            }
            impl Drop for SlotGuard {
                fn drop(&mut self) {
                    if let Some(coordinator) = self.coordinator.upgrade() {
                        coordinator.worker_finished(&self.task_id);
                    }
                }
            }
            let _guard = SlotGuard {
                coordinator: std::sync::Weak::clone(&weak),
                task_id,
            };
            worker::run_task(task, deps).await;
            // After the guard's slot release would be too late to read the
            // settled row, so the breaker observes here.
            if let Some(coordinator) = weak.upgrade() {
                coordinator.observe_network_failure(&observed_id).await;
            }
        });
    }

    fn registry_snapshot(&self) -> Arc<SourceRegistry> {
        // Clone the registry out of the shared state; the worker keeps a
        // stable view for this run (source edits apply to the next run).
        let guard = self.registry.lock().unwrap();
        Arc::new(guard.clone())
    }

    // ----- command-facing actions ------------------------------------------

    pub async fn pause(&self, task_id: &str) -> Result<DownloadTaskDto, AppError> {
        let row = self.require(task_id).await?;
        match row.status.as_str() {
            "queued" => {
                let moved = download_repo::transition(&self.pool, task_id, &["queued"], "paused")
                    .await?
                    .ok_or(AppError::Conflict)?;
                self.publish_now(&moved);
                Ok(task_repo::to_dto(moved))
            }
            "resolving" | "downloading" => {
                // Cooperative: worker acknowledges once writes quiesce.
                if let Some(handle) = self.active.lock().unwrap().get(task_id) {
                    handle.pause.store(true, Ordering::Relaxed);
                }
                Ok(task_repo::to_dto(row))
            }
            // Verify+export is a short commit window: the worker cannot
            // pause mid-publish, so reject honestly instead of silently
            // dropping the request.
            "verifying" | "exporting" => Err(invalid_transition()),
            "pausing" | "paused" => Ok(task_repo::to_dto(row)),
            _ => Err(invalid_transition()),
        }
    }

    pub async fn resume(&self, task_id: &str) -> Result<DownloadTaskDto, AppError> {
        let moved = download_repo::transition(&self.pool, task_id, &["paused"], "queued")
            .await?
            .ok_or_else(invalid_transition)?;
        if let Some(handle) = self.active.lock().unwrap().get(task_id) {
            handle.pause.store(false, Ordering::Relaxed);
        }
        self.publish_now(&moved);
        self.notify_new_work();
        Ok(task_repo::to_dto(moved))
    }

    pub async fn cancel(&self, task_id: &str) -> Result<DownloadTaskDto, AppError> {
        let row = self.require(task_id).await?;
        match row.status.as_str() {
            "queued" | "paused" => {
                let moved = download_repo::transition(
                    &self.pool,
                    task_id,
                    &["queued", "paused"],
                    "cancelled",
                )
                .await?
                .ok_or(AppError::Conflict)?;
                self.cleanup_task_staging(task_id);
                self.publish_now(&moved);
                Ok(task_repo::to_dto(moved))
            }
            "resolving" | "downloading" | "pausing" | "verifying" | "exporting" => {
                if let Some(handle) = self.active.lock().unwrap().get(task_id) {
                    handle.cancel.store(true, Ordering::Relaxed);
                    handle.pause.store(false, Ordering::Relaxed);
                }
                Ok(task_repo::to_dto(row))
            }
            _ => Err(invalid_transition()),
        }
    }

    pub async fn retry(&self, task_id: &str) -> Result<DownloadTaskDto, AppError> {
        let moved = download_repo::retry_failed(&self.pool, task_id)
            .await?
            .ok_or_else(invalid_transition)?;
        self.publish_now(&moved);
        self.notify_new_work();
        Ok(task_repo::to_dto(moved))
    }

    pub async fn set_priority(
        &self,
        task_id: &str,
        priority: i64,
    ) -> Result<DownloadTaskDto, AppError> {
        let moved = download_repo::set_priority(&self.pool, task_id, priority)
            .await?
            .ok_or_else(invalid_transition)?;
        self.publish_now(&moved);
        self.notify_new_work();
        Ok(task_repo::to_dto(moved))
    }

    /// Cooperative app-shutdown: signal every worker with the SHUTDOWN flag
    /// (never `cancel` — that would persist a terminal `cancelled` state the
    /// user never asked for), wait briefly for in-flight page writes to
    /// quiesce, then return. Workers exit with their task still active;
    /// startup recovery re-queues active states and checkpoints remain.
    pub async fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        for _ in 0..20 {
            if self.active.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn require(&self, task_id: &str) -> Result<download_repo::TaskRow, AppError> {
        download_repo::get_task(&self.pool, task_id)
            .await?
            .ok_or_else(|| AppError::not_found("task"))
    }

    fn publish_now(&self, row: &download_repo::TaskRow) {
        let dto = task_repo::to_dto(row.clone());
        self.events.emit_transition(&dto, 0, None);
    }

    fn cleanup_task_staging(&self, task_id: &str) {
        let dir = storage::task_staging_dir(&self.app_data_dir, task_id);
        if dir.exists() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    fn worker_finished(&self, task_id: &str) {
        self.active.lock().unwrap().remove(task_id);
        self.wake.notify_one();
    }

    /// Network-failure breaker seam: called once per finished worker with the
    /// row as it settled in the database. `SOURCE_NETWORK` is the only failure
    /// that implicates the SOURCE rather than one chapter, so only it extends
    /// the streak; anything else (a success, a content error) proves the source
    /// is answering and clears it. On the third consecutive network failure the
    /// queue pauses itself and the chapters it cost are put back in the queue
    /// instead of being left `failed`.
    pub(crate) async fn observe_network_failure(&self, task_id: &str) {
        let Ok(Some(row)) = download_repo::get_task(&self.pool, task_id).await else {
            return;
        };
        if row.status != "failed" || row.error_code.as_deref() != Some("SOURCE_NETWORK") {
            self.network_streak.lock().unwrap().clear();
            return;
        }
        let tripped = {
            let mut streak = self.network_streak.lock().unwrap();
            if !streak.iter().any(|seen| seen == task_id) {
                streak.push(task_id.to_string());
            }
            streak.len() >= NETWORK_FAILURE_BREAKER
        };
        if !tripped {
            return;
        }
        let cost: Vec<String> = std::mem::take(&mut *self.network_streak.lock().unwrap());
        self.queue_paused.store(true, Ordering::Relaxed);
        for id in cost {
            if let Ok(Some(moved)) = download_repo::retry_failed(&self.pool, &id).await {
                self.publish_now(&moved);
            }
        }
        eprintln!(
            "download queue auto-paused after {NETWORK_FAILURE_BREAKER} consecutive SOURCE_NETWORK failures"
        );
    }
}

fn invalid_transition() -> AppError {
    AppError::validation("Tác vụ không ở trạng thái cho phép thao tác này.")
}
