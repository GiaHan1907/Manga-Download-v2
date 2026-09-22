//! One-chapter worker: resolving → downloading → verifying → exporting →
//! completed, with cooperative pause/cancel and bounded automatic retry.
//!
//! The worker owns no long-lived resources: staging files, page checkpoints
//! and the artifact commit protocol make every early exit (pause, cancel,
//! crash, retry) safe to re-enter from `claim_next`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tokio::task::JoinSet;

use crate::db::repositories::{download_repo, task_repo};
use crate::domain::DownloadTaskDto;
use crate::downloader::exporter::ExportPage;
use crate::downloader::progress::ProgressEmitter;
use crate::downloader::{checkpoint, exporter, storage, transfer, MAX_ATTEMPTS};
use crate::error::AppError;
use crate::sources::adapter::SourceAdapter;
use crate::sources::adapter::SourceError;
use crate::sources::dto::Capability;
use crate::sources::registry::SourceRegistry;

pub struct WorkerFlags {
    pub cancel: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
}

pub struct WorkerDeps {
    pub pool: SqlitePool,
    pub registry: Arc<SourceRegistry>,
    pub app_data_dir: PathBuf,
    pub events: Arc<ProgressEmitter>,
    pub flags: WorkerFlags,
    pub image_concurrency: i64,
}

/// Live byte accounting for progress frames (never persisted — design §8).
#[derive(Default)]
struct Live {
    bytes: u64,
    sample_at: Option<Instant>,
    sample_bytes: u64,
    speed: Option<u64>,
}

impl Live {
    fn record(&mut self, added: u64) {
        self.bytes = self.bytes.saturating_add(added);
        let now = Instant::now();
        match self.sample_at {
            Some(previous) => {
                let elapsed = now.duration_since(previous);
                if elapsed >= Duration::from_secs(2) {
                    let delta = self.bytes.saturating_sub(self.sample_bytes);
                    self.speed = Some((delta as f64 / elapsed.as_secs_f64()) as u64);
                    self.sample_at = Some(now);
                    self.sample_bytes = self.bytes;
                }
            }
            None => {
                self.sample_at = Some(now);
                self.sample_bytes = self.bytes;
            }
        }
    }
}

type SharedLive = Arc<Mutex<Live>>;

/// Immutable context shared by every page task of one chapter run.
struct PageJob {
    pool: SqlitePool,
    adapter: Arc<dyn SourceAdapter>,
    staging: PathBuf,
    task_id: String,
    events: Arc<ProgressEmitter>,
    live: SharedLive,
    flags: WorkerFlags,
    running_total: Arc<Mutex<u64>>,
}

pub async fn run_task(
    task: download_repo::TaskRow,
    deps: Arc<WorkerDeps>,
) -> Option<DownloadTaskDto> {
    let live: SharedLive = Arc::new(Mutex::new(Live::default()));
    match attempt_run(&task, &deps, &live).await {
        Ok(dto) => Some(dto),
        Err(error) => fail_or_requeue(&deps, &task.id, &error, &live).await,
    }
}

fn is_transient(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Source(
            SourceError::Network | SourceError::Upstream | SourceError::RateLimited(_)
        )
    )
}

/// Apply the retry policy: transient errors within budget requeue with a
/// backoff timestamp (kept visible in the error fields); anything else, or a
/// budget miss, records a terminal `failed`.
async fn fail_or_requeue(
    deps: &WorkerDeps,
    task_id: &str,
    error: &AppError,
    live: &SharedLive,
) -> Option<DownloadTaskDto> {
    let row = download_repo::get_task(&deps.pool, task_id).await.ok()??;
    if matches!(row.status.as_str(), "cancelled" | "paused" | "completed") {
        return Some(task_repo::to_dto(row));
    }
    let (code, message) = error.wire_pair();
    let requeue = is_transient(error) && row.attempts + 1 < MAX_ATTEMPTS;
    let outcome = if requeue {
        let delay = crate::downloader::backoff_duration(row.attempts + 1, task_id);
        let due = (chrono::Utc::now() + chrono_duration(delay))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        download_repo::requeue_with_backoff(&deps.pool, task_id, Some(&due), code, message.as_str())
            .await
            .ok()
            .flatten()
    } else {
        download_repo::record_failure(&deps.pool, task_id, code, message.as_str())
            .await
            .ok()
            .flatten()
    };
    let dto = task_repo::to_dto(outcome.unwrap_or(row));
    let (bytes, speed) = snapshot(live);
    deps.events.emit_transition(&dto, bytes, speed);
    Some(dto)
}

fn chrono_duration(value: Duration) -> chrono::Duration {
    chrono::Duration::from_std(value).unwrap_or_else(|_| chrono::Duration::seconds(60))
}

fn snapshot(live: &SharedLive) -> (u64, Option<u64>) {
    let guard = live.lock().unwrap();
    (guard.bytes, guard.speed)
}

async fn attempt_run(
    task: &download_repo::TaskRow,
    deps: &WorkerDeps,
    live: &SharedLive,
) -> Result<DownloadTaskDto, AppError> {
    let task_id = task.id.clone();

    // 1. Source enablement is re-checked at execution time; disabling a
    //    source stops new network work for its tasks.
    let (enabled, source_id): (i64, String) = sqlx::query_as(
        "SELECT s.enabled, m.source_id FROM mangas m \
         JOIN sources s ON s.id = m.source_id \
         JOIN chapters c ON c.manga_id = m.id WHERE c.id = ?1",
    )
    .bind(&task.chapter_id)
    .fetch_optional(&deps.pool)
    .await?
    .ok_or_else(|| AppError::not_found("chương của tác vụ"))?;
    if enabled == 0 {
        return Err(AppError::Source(SourceError::SourceDisabled));
    }
    if deps.flags.cancel.load(Ordering::Relaxed) {
        return stop_as_cancelled(deps, &task_id).await;
    }

    let ctx = download_repo::chapter_context(&deps.pool, &task.chapter_id)
        .await?
        .ok_or_else(|| AppError::not_found("chương"))?;
    let adapter = deps
        .registry
        .get(&source_id)
        .ok_or(AppError::Source(SourceError::SourceDisabled))?;
    if !adapter.info().capabilities.contains(&Capability::Pages) {
        return Err(AppError::Source(SourceError::UnsupportedOperation));
    }

    // 2. resolving: manifest (adapter-internal; page URLs never reach the UI).
    let manifest = adapter
        .page_manifest(&ctx.source_manga_id, &ctx.source_chapter_id)
        .await?;
    // Identity guard: the manifest must describe the chapter we asked for,
    // otherwise checkpoints could be assembled from mismatched pages.
    if manifest.source_chapter_id != ctx.source_chapter_id {
        return Err(AppError::validation(
            "Danh sách trang không khớp chương yêu cầu.",
        ));
    }
    validate_manifest(&manifest)?;
    let total_pages = manifest.pages.len();
    download_repo::ensure_page_rows(&deps.pool, &task_id, total_pages).await?;
    let page_urls: Vec<String> = manifest.pages.iter().map(|page| page.url.clone()).collect();

    if advance(deps, &task_id, &["resolving"], "downloading", live)
        .await
        .is_none()
    {
        return current_dto(deps, &task_id).await;
    }

    let staging = storage::task_staging_dir(&deps.app_data_dir, &task_id);
    std::fs::create_dir_all(&staging)?;
    transfer::discard_part_files(&staging)?;
    checkpoint::reconcile(&deps.pool, &task_id, &staging).await?;

    // 3. downloading: pending pages in bounded chunks; cooperative flags are
    //    honored at chunk boundaries (quiesced writes → safe pause/cancel).
    let job = Arc::new(PageJob {
        pool: deps.pool.clone(),
        adapter,
        staging: staging.clone(),
        task_id: task_id.clone(),
        events: deps.events.clone(),
        live: live.clone(),
        flags: WorkerFlags {
            cancel: deps.flags.cancel.clone(),
            pause: deps.flags.pause.clone(),
        },
        running_total: Arc::new(Mutex::new(0u64)),
    });
    loop {
        let pages = download_repo::list_pages(&deps.pool, &task_id).await?;
        let pending: Vec<usize> = pages
            .iter()
            .filter(|p| p.status != "stored" && (p.page_index as usize) < page_urls.len())
            .map(|p| p.page_index as usize)
            .collect();
        if pending.is_empty() {
            break;
        }
        if deps.flags.cancel.load(Ordering::Relaxed) {
            transfer::discard_part_files(&staging)?;
            return stop_as_cancelled(deps, &task_id).await;
        }
        if deps.flags.pause.load(Ordering::Relaxed) {
            return stop_as_paused(deps, &task_id, live).await;
        }
        let chunk_size = deps.image_concurrency.clamp(1, 8) as usize;
        let mut set: JoinSet<Result<(), AppError>> = JoinSet::new();
        for index in pending.into_iter().take(chunk_size) {
            let job = Arc::clone(&job);
            let url = page_urls[index].clone();
            set.spawn(async move { fetch_and_store_page(job, index, url).await });
        }
        while let Some(result) = set.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => return Err(error),
                Err(join_error) if join_error.is_cancelled() => {
                    return stop_as_cancelled(deps, &task_id).await;
                }
                Err(_) => return Err(AppError::Internal),
            }
        }
    }

    // 4. verifying: DB rows AND staged files must prove every page.
    if advance(deps, &task_id, &["downloading"], "verifying", live)
        .await
        .is_none()
    {
        return current_dto(deps, &task_id).await;
    }
    let (report, _) = checkpoint::reconcile(&deps.pool, &task_id, &staging).await?;
    if report.valid != total_pages {
        return Err(AppError::validation(
            "Kiểm tra trang thất bại — thiếu trang đã xác minh.",
        ));
    }

    // 5. exporting: build beside the final location, verify, atomically
    //    publish, and only then commit completion in the DB.
    if advance(deps, &task_id, &["verifying"], "exporting", live)
        .await
        .is_none()
    {
        return current_dto(deps, &task_id).await;
    }
    let stored = download_repo::list_pages(&deps.pool, &task_id).await?;
    let exports: Vec<ExportPage> = stored
        .iter()
        .filter(|p| p.status == "stored" && p.storage_key.is_some() && p.checksum.is_some())
        .map(|p| ExportPage {
            index: p.page_index as usize,
            staged_name: p.storage_key.as_deref().unwrap_or_default(),
            size_bytes: p.size_bytes.unwrap_or_default(),
            checksum_hex: p.checksum.as_deref().unwrap_or_default(),
        })
        .collect();
    if exports.len() != total_pages {
        return Err(AppError::validation("Bộ trang xuất không đầy đủ."));
    }
    let library_root = storage::library_root(&deps.app_data_dir);
    std::fs::create_dir_all(&library_root)?;
    let manga_dir_name = storage::manga_dir_name(&ctx.manga_title, &ctx.manga_id);
    storage::assert_safe_name_component(&manga_dir_name)?;
    let manga_dir = library_root.join(&manga_dir_name);
    storage::ensure_under_root(&library_root, &manga_dir)?;
    std::fs::create_dir_all(&manga_dir)?;

    let base = storage::chapter_file_name(
        &ctx.chapter_title,
        &ctx.chapter_id,
        &task_id,
        &task.target_format,
    );
    storage::assert_safe_name_component(&base)?;
    let relpath = match task.target_format.as_str() {
        "folder" => {
            let final_dir = manga_dir.join(&base);
            let part_dir = manga_dir.join(format!("{base}.part-dir"));
            exporter::export_folder(&staging, &exports, &part_dir, &final_dir)?;
            format!("{manga_dir_name}/{base}")
        }
        _ => {
            let final_path = manga_dir.join(&base);
            let (part_path, _) = exporter::part_paths(&final_path);
            exporter::write_cbz(&staging, &exports, &part_path)?;
            exporter::verify_cbz(&part_path, &exports)?;
            exporter::publish_artifact(&part_path, &final_path, &exports)?;
            format!("{manga_dir_name}/{base}")
        }
    };

    let published =
        download_repo::publish_completed(&deps.pool, &task_id, &relpath, &task.target_format)
            .await?
            .ok_or(AppError::Conflict)?;
    let _ = std::fs::remove_dir_all(&staging);
    let dto = task_repo::to_dto(published);
    let (bytes, speed) = snapshot(live);
    deps.events.emit_transition(&dto, bytes, speed);
    Ok(dto)
}

/// Fetch one page through the adapter's guarded transport, land it as a
/// validated staged file, then checkpoint it. Enforces the chapter byte cap.
async fn fetch_and_store_page(
    job: Arc<PageJob>,
    index: usize,
    url: String,
) -> Result<(), AppError> {
    if job.flags.cancel.load(Ordering::Relaxed) {
        return Err(AppError::Internal);
    }
    let staged = transfer::fetch_page(&job.staging, index, job.adapter.fetch_image(&url)).await?;
    {
        let mut total = job.running_total.lock().unwrap();
        *total = total.saturating_add(staged.size_bytes as u64);
        if *total > crate::downloader::CHAPTER_MAX_BYTES {
            return Err(AppError::validation("Chương vượt giới hạn dung lượng."));
        }
    }
    download_repo::mark_page_stored(
        &job.pool,
        &job.task_id,
        index,
        &staged.name,
        staged.size_bytes,
        &staged.checksum,
    )
    .await?;
    job.live.lock().unwrap().record(staged.size_bytes as u64);
    // Progress frame for the freshly updated counters.
    if let Some(row) = download_repo::get_task(&job.pool, &job.task_id).await? {
        let dto = task_repo::to_dto(row);
        let (bytes, speed) = {
            let guard = job.live.lock().unwrap();
            (guard.bytes, guard.speed)
        };
        job.events.emit_progress(&dto, bytes, speed);
    }
    Ok(())
}

fn validate_manifest(manifest: &crate::sources::dto::PageManifest) -> Result<(), AppError> {
    if manifest.pages.is_empty() || manifest.pages.len() > crate::sources::http::MAX_PAGE_COUNT {
        return Err(AppError::validation("Danh sách trang vượt giới hạn."));
    }
    for (position, page) in manifest.pages.iter().enumerate() {
        if page.index != position || page.url.is_empty() || page.url.len() > 2048 {
            return Err(AppError::validation(
                "Danh sách trang không liên tục hoặc quá dài.",
            ));
        }
    }
    Ok(())
}

async fn advance(
    deps: &WorkerDeps,
    task_id: &str,
    from: &[&str],
    to: &str,
    live: &SharedLive,
) -> Option<download_repo::TaskRow> {
    let row = download_repo::transition(&deps.pool, task_id, from, to)
        .await
        .ok()??;
    let dto = task_repo::to_dto(row.clone());
    let (bytes, speed) = snapshot(live);
    deps.events.emit_transition(&dto, bytes, speed);
    Some(row)
}

async fn current_dto(deps: &WorkerDeps, task_id: &str) -> Result<DownloadTaskDto, AppError> {
    let row = download_repo::get_task(&deps.pool, task_id)
        .await?
        .ok_or_else(|| AppError::not_found("task"))?;
    Ok(task_repo::to_dto(row))
}

async fn stop_as_cancelled(deps: &WorkerDeps, task_id: &str) -> Result<DownloadTaskDto, AppError> {
    let from = &[
        "queued",
        "resolving",
        "downloading",
        "pausing",
        "paused",
        "verifying",
        "exporting",
    ];
    let moved = download_repo::transition(&deps.pool, task_id, from, "cancelled").await?;
    let row = match moved {
        Some(row) => row,
        None => download_repo::get_task(&deps.pool, task_id)
            .await?
            .ok_or_else(|| AppError::not_found("task"))?,
    };
    let dto = task_repo::to_dto(row);
    deps.events.emit_transition(&dto, 0, None);
    Ok(dto)
}

async fn stop_as_paused(
    deps: &WorkerDeps,
    task_id: &str,
    live: &SharedLive,
) -> Result<DownloadTaskDto, AppError> {
    // Durable `pausing` first (in-flight writes have quiesced at this point).
    let _ = download_repo::transition(
        &deps.pool,
        task_id,
        &["queued", "resolving", "downloading"],
        "pausing",
    )
    .await;
    let moved = download_repo::transition(
        &deps.pool,
        task_id,
        &["resolving", "downloading", "pausing"],
        "paused",
    )
    .await?;
    let row = match moved {
        Some(row) => row,
        None => download_repo::get_task(&deps.pool, task_id)
            .await?
            .ok_or_else(|| AppError::not_found("task"))?,
    };
    let dto = task_repo::to_dto(row);
    let (bytes, speed) = snapshot(live);
    deps.events.emit_transition(&dto, bytes, speed);
    Ok(dto)
}
