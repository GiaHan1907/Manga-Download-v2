//! End-to-end engine tests over a scripted adapter and disposable SQLite:
//! full pipeline, resume-from-checkpoint, cooperative pause/cancel, retry
//! budget, unsupported/disabled sources and hostile content — no network.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::SqlitePool;
use url::Url;

use crate::db::repositories::download_repo;
use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};
use crate::db::test_pool;
use crate::downloader::progress::{CollectingSink, ProgressEmitter};
use crate::downloader::{storage, worker};
use crate::error::AppError;
use crate::sources::adapter::{SourceAdapter, SourceError};
use crate::sources::dto::{
    Capability, PageDescriptor, PageManifest, RemoteMangaDetail, RemoteSearchPage, SourceInfo,
    SourceStatus,
};
use crate::sources::registry::SourceRegistry;

fn image_bytes(seed: u8) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
    bytes.extend(std::iter::repeat_n(seed, 40));
    bytes
}

struct Script {
    pages: usize,
    declare_pages: bool,
    pause_during_fetch: bool,
    fail_first_fetches: usize,
    poison_pages: Vec<usize>,
    /// page_manifest waits on `gate` then fails: exercises the flag
    /// precedence inside `fail_or_requeue` deterministically.
    manifest_gate_fail: bool,
    fetches: Arc<AtomicUsize>,
    gate: Arc<AtomicBool>,
}

impl Script {
    fn new(pages: usize) -> Self {
        Script {
            pages,
            declare_pages: true,
            pause_during_fetch: false,
            fail_first_fetches: 0,
            poison_pages: Vec::new(),
            manifest_gate_fail: false,
            fetches: Arc::new(AtomicUsize::new(0)),
            gate: Arc::new(AtomicBool::new(false)),
        }
    }
}

struct ScriptedAdapter {
    info: SourceInfo,
    script: Arc<Script>,
}

impl ScriptedAdapter {
    fn new(script: Arc<Script>) -> Self {
        let mut capabilities = vec![
            Capability::Search,
            Capability::ResolveUrl,
            Capability::Details,
        ];
        if script.declare_pages {
            capabilities.push(Capability::Pages);
        }
        ScriptedAdapter {
            info: SourceInfo {
                id: "scripted".to_string(),
                display_name: "Scripted".to_string(),
                capabilities,
                status: SourceStatus::Ready,
                attribution: None,
            },
            script,
        }
    }
}

#[async_trait]
impl SourceAdapter for ScriptedAdapter {
    fn info(&self) -> SourceInfo {
        self.info.clone()
    }
    fn owns_url(&self, _url: &Url) -> bool {
        false
    }
    async fn search(&self, _query: &str, _page: u32) -> Result<RemoteSearchPage, SourceError> {
        Err(SourceError::UnsupportedOperation)
    }
    async fn resolve_url(&self, _url: &Url) -> Result<String, SourceError> {
        Err(SourceError::UnsupportedOperation)
    }
    async fn manga_detail(&self, _id: &str) -> Result<RemoteMangaDetail, SourceError> {
        Err(SourceError::UnsupportedOperation)
    }
    async fn page_manifest(
        &self,
        _source_manga_id: &str,
        source_chapter_id: &str,
    ) -> Result<PageManifest, SourceError> {
        if self.script.manifest_gate_fail {
            for _ in 0..2500 {
                if self.script.gate.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
            return Err(SourceError::Network);
        }
        let pages = (0..self.script.pages)
            .map(|index| PageDescriptor {
                index,
                url: format!("https://engine.test/pages/{source_chapter_id}/{index}.png"),
            })
            .collect();
        Ok(PageManifest {
            source_chapter_id: source_chapter_id.to_string(),
            pages,
        })
    }
    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>, SourceError> {
        let script = &self.script;
        let count = script.fetches.fetch_add(1, Ordering::SeqCst);
        if script.pause_during_fetch {
            // Hold until the test flips the gate (bounded, never hangs).
            for _ in 0..2500 {
                if script.gate.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
        }
        if count < script.fail_first_fetches {
            return Err(SourceError::Network);
        }
        let index = url
            .rsplit('/')
            .next()
            .and_then(|tail| tail.split('.').next())
            .and_then(|digits| digits.parse::<usize>().ok())
            .unwrap_or(usize::MAX);
        if script.poison_pages.contains(&index) {
            return Ok(b"<html>error page</html>".to_vec());
        }
        Ok(image_bytes((index as u8).wrapping_add(1)))
    }
}

/// Per-test world: disposable DB + app-data dir + scripted registry.
struct World {
    pool: SqlitePool,
    data_dir: tempfile::TempDir,
    _db_dir: tempfile::TempDir,
    script: Arc<Script>,
    events: Arc<ProgressEmitter>,
    sink: CollectingSink,
}

impl World {
    async fn new(script: Script) -> Self {
        let (db_dir, pool) = test_pool().await;
        let data_dir = tempfile::tempdir().unwrap();
        ensure_source(&pool, "scripted", "Scripted").await.unwrap();
        let script = Arc::new(script);
        let sink = CollectingSink::default();
        let events = Arc::new(ProgressEmitter::new(Arc::new(sink.clone())));
        World {
            pool,
            data_dir,
            _db_dir: db_dir,
            script,
            events,
            sink,
        }
    }

    fn registry(&self) -> Arc<SourceRegistry> {
        Arc::new(SourceRegistry::with_adapters(vec![Arc::new(
            ScriptedAdapter::new(self.script.clone()),
        )]))
    }

    async fn seed_chapter(&self, slug: &str) -> String {
        let (manga_id, _) = upsert_manga(
            &self.pool,
            "scripted",
            "m1",
            "Manga Title",
            None,
            None,
            "ongoing",
        )
        .await
        .unwrap();
        let (chapter_id, _) =
            upsert_chapter(&self.pool, &manga_id, slug, "Chapter 1", Some(1.0), None)
                .await
                .unwrap();
        chapter_id
    }

    async fn queue(&self, chapter_id: &str, format: &str) -> download_repo::TaskRow {
        download_repo::queue_batch(&self.pool, &[chapter_id.to_string()], format, 0)
            .await
            .unwrap()
            .created
            .into_iter()
            .next()
            .expect("created task")
    }

    /// Claim + run the worker exactly once, mirroring what the dispatcher does.
    async fn run_once(
        &self,
        pause: Arc<AtomicBool>,
        cancel: Arc<AtomicBool>,
    ) -> Result<crate::domain::DownloadTaskDto, AppError> {
        self.run_once_flags(pause, cancel, Arc::new(AtomicBool::new(false)))
            .await
    }

    async fn run_once_flags(
        &self,
        pause: Arc<AtomicBool>,
        cancel: Arc<AtomicBool>,
        shutdown: Arc<AtomicBool>,
    ) -> Result<crate::domain::DownloadTaskDto, AppError> {
        let claimed = download_repo::claim_next(&self.pool)
            .await?
            .expect("claimable task");
        let deps = Arc::new(worker::WorkerDeps {
            pool: self.pool.clone(),
            registry: self.registry(),
            app_data_dir: self.data_dir.path().to_path_buf(),
            events: self.events.clone(),
            flags: worker::WorkerFlags {
                cancel,
                pause,
                shutdown,
            },
            image_concurrency: 2,
        });
        worker::run_task(claimed, deps)
            .await
            .ok_or(AppError::Internal)
    }
}

fn no_flags() -> (Arc<AtomicBool>, Arc<AtomicBool>) {
    (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    )
}

#[tokio::test]
async fn full_pipeline_produces_verified_cbz_and_completes() {
    let world = World::new(Script::new(3)).await;
    let chapter_id = world.seed_chapter("ch-full").await;
    let task = world.queue(&chapter_id, "cbz").await;
    let (pause, cancel) = no_flags();
    let dto = world.run_once(pause, cancel).await.unwrap();
    assert_eq!(dto.status, "completed");
    assert_eq!(dto.completed_pages, 3);
    assert_eq!(dto.total_pages, 3);
    let relpath = dto.output_relpath.clone().unwrap();

    // Artifact exists under the library root and contains all 3 entries.
    let artifact = storage::library_root(world.data_dir.path()).join(&relpath);
    assert!(artifact.exists(), "cbz missing at {artifact:?}");
    let bytes = std::fs::read(&artifact).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04"));
    for index in 0..3 {
        let name = format!("{index:06}.png");
        assert!(
            bytes
                .windows(name.len())
                .any(|window| window == name.as_bytes()),
            "entry {name} missing"
        );
    }

    // Chapter marked available offline; staging cleaned; events monotonic.
    let local: (Option<String>,) = sqlx::query_as("SELECT local_path FROM chapters WHERE id = ?1")
        .bind(&chapter_id)
        .fetch_one(&world.pool)
        .await
        .unwrap();
    assert_eq!(local.0.as_deref(), Some(relpath.as_str()));
    assert!(!storage::task_staging_dir(world.data_dir.path(), &task.id).exists());
    let events = world.sink.0.lock().unwrap();
    let revisions: Vec<i64> = events.iter().map(|event| event.task.revision).collect();
    assert!(
        revisions.windows(2).all(|pair| pair[0] <= pair[1]),
        "revisions must be monotonic: {revisions:?}"
    );
    assert_eq!(events.last().unwrap().task.status, "completed");
    assert!(
        events
            .iter()
            .any(|event| event.task.status == "downloading"),
        "lifecycle transitions must be observable"
    );
}

#[tokio::test]
async fn cancel_before_work_stops_cleanly() {
    let world = World::new(Script::new(3)).await;
    let chapter_id = world.seed_chapter("ch-cancel").await;
    world.queue(&chapter_id, "cbz").await;
    let (pause, _) = no_flags();
    let dto = world
        .run_once(pause, Arc::new(AtomicBool::new(true)))
        .await
        .unwrap();
    assert_eq!(dto.status, "cancelled");
    assert_eq!(
        world.script.fetches.load(Ordering::SeqCst),
        0,
        "no fetch after cancel"
    );
}

#[tokio::test]
async fn cancel_observed_midflight_lands_cancelled_not_failed() {
    // Regression (review round 3): a cancel that a page-fetch observes must
    // surface as `cancelled`, never a spurious `failed`/INTERNAL.
    let mut script = Script::new(3);
    script.pause_during_fetch = true; // hold fetches on a gate
    let world = World::new(script).await;
    let chapter_id = world.seed_chapter("ch-cancel-mid").await;
    world.queue(&chapter_id, "cbz").await;

    let cancel = Arc::new(AtomicBool::new(false));
    let gate = world.script.gate.clone();
    let fetches = world.script.fetches.clone();
    let cancel_flag = cancel.clone();
    let waiter = tokio::spawn(async move {
        for _ in 0..2500 {
            if fetches.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        cancel_flag.store(true, Ordering::SeqCst); // cancel while a fetch is in flight
        gate.store(true, Ordering::SeqCst); // release held fetches
    });
    let dto = world
        .run_once(Arc::new(AtomicBool::new(false)), cancel)
        .await
        .unwrap();
    waiter.await.unwrap();
    assert_eq!(dto.status, "cancelled", "must be cancelled, not failed");
    assert_ne!(dto.error_code.as_deref(), Some("INTERNAL_ERROR"));
}

#[tokio::test]
async fn shutdown_after_error_keeps_task_resumable_not_failed() {
    // Regression (whole-project review round 2): app shutdown previously
    // reused the cancel flag, so workers persisted a terminal `cancelled`
    // the user never asked for. Shutdown must leave the task active and
    // error-free so startup recovery re-queues it with checkpoints intact.
    let mut script = Script::new(3);
    script.manifest_gate_fail = true;
    let world = World::new(script).await;
    let chapter_id = world.seed_chapter("ch-shutdown").await;
    world.queue(&chapter_id, "cbz").await;

    let shutdown = Arc::new(AtomicBool::new(false));
    let waiter = {
        let shutdown = shutdown.clone();
        let gate = world.script.gate.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            shutdown.store(true, Ordering::SeqCst);
            gate.store(true, Ordering::SeqCst); // manifest fails AFTER shutdown
        })
    };
    let dto = world
        .run_once_flags(
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            shutdown,
        )
        .await
        .unwrap();
    waiter.await.unwrap();
    assert_eq!(dto.status, "resolving", "shutdown must not terminalize");
    assert_ne!(dto.status, "failed");
    assert_ne!(dto.status, "cancelled");
    assert!(dto.error_code.is_none(), "no failure may be persisted");
    // Startup recovery re-queues it.
    let report = crate::downloader::recovery::reconcile_startup(&world.pool, world.data_dir.path())
        .await
        .unwrap();
    assert_eq!(report.normalized_tasks, 1);
    let after = download_repo::get_task(&world.pool, &dto.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "queued");
}

#[tokio::test]
async fn pending_cancel_after_error_lands_cancelled() {
    // fail_or_requeue must honor a cancel that raced the error path: a
    // user-requested cancel never turns into `failed`/requeue noise.
    let mut script = Script::new(3);
    script.manifest_gate_fail = true;
    let world = World::new(script).await;
    let chapter_id = world.seed_chapter("ch-cancel-err").await;
    world.queue(&chapter_id, "cbz").await;

    let cancel = Arc::new(AtomicBool::new(false));
    let waiter = {
        let cancel = cancel.clone();
        let gate = world.script.gate.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            cancel.store(true, Ordering::SeqCst);
            gate.store(true, Ordering::SeqCst);
        })
    };
    let dto = world
        .run_once_flags(
            Arc::new(AtomicBool::new(false)),
            cancel,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    waiter.await.unwrap();
    assert_eq!(dto.status, "cancelled");
    assert!(dto.error_code.is_none());
}

#[tokio::test]
async fn coordinator_rejects_pause_in_verify_export_commit_window() {
    // Pause must not be silently swallowed: verifying/exporting cannot
    // pause mid-publish, so the coordinator rejects instead of accepting a
    // request that would never be honored.
    use crate::downloader::scheduler::Coordinator;
    use std::sync::Mutex;
    let world = World::new(Script::new(1)).await;
    let chapter_id = world.seed_chapter("ch-pause-window").await;
    let task = world.queue(&chapter_id, "cbz").await;
    for (from, to) in [
        ("queued", "resolving"),
        ("resolving", "downloading"),
        ("downloading", "verifying"),
        ("verifying", "exporting"),
    ] {
        download_repo::transition(&world.pool, &task.id, &[from], to)
            .await
            .unwrap()
            .unwrap();
    }
    let coordinator = Coordinator::new(
        world.pool.clone(),
        Arc::new(Mutex::new(SourceRegistry::with_adapters(vec![]))),
        world.data_dir.path().to_path_buf(),
        world.events.clone(),
    );
    let error = coordinator.pause(&task.id).await.unwrap_err();
    assert!(
        matches!(error, AppError::Validation(_)),
        "expected validation rejection, got {error:?}"
    );
    // downloading still accepts cooperative pause.
    download_repo::transition(&world.pool, &task.id, &["exporting"], "verifying")
        .await
        .unwrap()
        .unwrap();
    download_repo::transition(&world.pool, &task.id, &["verifying"], "downloading")
        .await
        .unwrap()
        .unwrap();
    coordinator
        .pause(&task.id)
        .await
        .expect("downloading accepts pause");
}

#[tokio::test]
async fn pause_midflight_then_resume_skips_stored_pages() {
    let mut script = Script::new(4);
    script.pause_during_fetch = true;
    let world = World::new(script).await;
    let chapter_id = world.seed_chapter("ch-pause").await;
    world.queue(&chapter_id, "cbz").await;

    let pause = Arc::new(AtomicBool::new(false));
    // Flip pause once the first fetch is in flight, then release held pages.
    let waiter = tokio::spawn({
        let pause = pause.clone();
        let gate = world.script.gate.clone();
        let fetches = world.script.fetches.clone();
        async move {
            for _ in 0..2500 {
                if fetches.load(Ordering::SeqCst) > 0 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
            pause.store(true, Ordering::SeqCst);
            gate.store(true, Ordering::SeqCst);
        }
    });
    let dto = world
        .run_once(pause.clone(), Arc::new(AtomicBool::new(false)))
        .await
        .unwrap();
    waiter.await.unwrap();
    assert_eq!(dto.status, "paused");
    assert_eq!(dto.completed_pages, 2, "first chunk stored before boundary");

    // Resume: stored pages must not be fetched again.
    pause.store(false, Ordering::SeqCst);
    download_repo::transition(&world.pool, &dto.id, &["paused"], "queued")
        .await
        .unwrap()
        .expect("resume requeue");
    let fetches_before_resume = world.script.fetches.load(Ordering::SeqCst);
    let claimed = download_repo::claim_next(&world.pool)
        .await
        .unwrap()
        .unwrap();
    let deps = Arc::new(worker::WorkerDeps {
        pool: world.pool.clone(),
        registry: world.registry(),
        app_data_dir: world.data_dir.path().to_path_buf(),
        events: world.events.clone(),
        flags: worker::WorkerFlags {
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(AtomicBool::new(false)),
        },
        image_concurrency: 4,
    });
    // The gate stays open (already released), so remaining pages resolve
    // without blocking even though the hold behavior is unchanged.
    let second = worker::run_task(claimed, deps).await.unwrap();
    assert_eq!(second.status, "completed");
    assert_eq!(
        world.script.fetches.load(Ordering::SeqCst) - fetches_before_resume,
        2,
        "only the missing pages re-fetched"
    );
    // T06-11 consistency: paused state was durably observable.
    let statuses: Vec<String> = world
        .sink
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|event| event.task.status.clone())
        .collect();
    assert!(statuses.contains(&"paused".to_string()), "{statuses:?}");
}

#[tokio::test]
async fn transient_failures_requeue_then_exhaust_to_failed() {
    let mut script = Script::new(2);
    script.fail_first_fetches = 100; // always network
    let world = World::new(script).await;
    let chapter_id = world.seed_chapter("ch-retry").await;
    let task = world.queue(&chapter_id, "cbz").await;

    let mut last_status = String::new();
    let mut last_attempts = -1;
    for _round in 0..10 {
        let (pause, cancel) = no_flags();
        let dto = world.run_once(pause, cancel).await.unwrap();
        last_status = dto.status.clone();
        last_attempts = dto.attempts;
        if dto.status == "failed" {
            break;
        }
        assert_eq!(
            dto.status, "queued",
            "transient error requeues with backoff"
        );
        assert!(dto.next_retry_at.is_some(), "retry window persisted");
        assert_eq!(dto.error_code.as_deref(), Some("SOURCE_NETWORK"));
        // Make the next round immediately claimable.
        sqlx::query("UPDATE download_tasks SET next_retry_at = NULL WHERE id = ?1")
            .bind(&dto.id)
            .execute(&world.pool)
            .await
            .unwrap();
    }
    assert_eq!(last_status, "failed");
    assert_eq!(last_attempts, crate::downloader::MAX_ATTEMPTS - 1);
    let _ = task;
}

#[tokio::test]
async fn unsupported_and_disabled_sources_fail_safely() {
    let mut script = Script::new(2);
    script.declare_pages = false;
    let world = World::new(script).await;
    let chapter_id = world.seed_chapter("ch-unsupported").await;
    let task = world.queue(&chapter_id, "cbz").await;
    let (pause, cancel) = no_flags();
    let dto = world.run_once(pause, cancel).await.unwrap();
    assert_eq!(dto.status, "failed");
    assert_eq!(dto.error_code.as_deref(), Some("UNSUPPORTED_OPERATION"));
    assert_eq!(task.status, "queued");
    assert_eq!(
        world.script.fetches.load(Ordering::SeqCst),
        0,
        "no bypass attempts"
    );

    // Explicit user retry keeps the same bounded, honest cycle.
    let retried = download_repo::retry_failed(&world.pool, &dto.id)
        .await
        .unwrap();
    assert_eq!(retried.unwrap().status, "queued");
}

#[tokio::test]
async fn hostile_http200_html_page_fails_without_retry_burn() {
    let mut script = Script::new(2);
    script.poison_pages = vec![0];
    let world = World::new(script).await;
    let chapter_id = world.seed_chapter("ch-poison").await;
    world.queue(&chapter_id, "cbz").await;
    let (pause, cancel) = no_flags();
    let dto = world.run_once(pause, cancel).await.unwrap();
    assert_eq!(dto.status, "failed");
    assert_eq!(dto.error_code, Some("VALIDATION_ERROR".to_string()));
    assert_eq!(dto.attempts, 0, "content errors never auto-retry");
    // Nothing published, staging holds at most page 1.
    let any_output = std::fs::read_dir(storage::library_root(world.data_dir.path()))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(any_output, 0, "no artifact may exist for a failed chapter");
}

#[tokio::test]
async fn disabled_source_blocks_new_network_work() {
    let world = World::new(Script::new(2)).await;
    let chapter_id = world.seed_chapter("ch-disabled").await;
    world.queue(&chapter_id, "cbz").await;
    sqlx::query("UPDATE sources SET enabled = 0 WHERE id = 'scripted'")
        .execute(&world.pool)
        .await
        .unwrap();
    let (pause, cancel) = no_flags();
    let dto = world.run_once(pause, cancel).await.unwrap();
    assert_eq!(dto.error_code.as_deref(), Some("SOURCE_DISABLED"));
    assert_eq!(world.script.fetches.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn folder_output_publishes_only_verified_tree() {
    let world = World::new(Script::new(2)).await;
    let chapter_id = world.seed_chapter("ch-folder").await;
    world.queue(&chapter_id, "folder").await;
    let (pause, cancel) = no_flags();
    let dto = world.run_once(pause, cancel).await.unwrap();
    assert_eq!(dto.status, "completed");
    let relpath = dto.output_relpath.unwrap();
    let dir = storage::library_root(world.data_dir.path()).join(&relpath);
    assert!(dir.is_dir());
    assert!(dir.join("000000.png").exists());
    assert!(dir.join("000001.png").exists());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
}

#[tokio::test]
async fn second_completed_download_never_overwrites_first_artifact() {
    let world = World::new(Script::new(2)).await;
    let first_chapter = world.seed_chapter("ch-one").await;
    world.queue(&first_chapter, "cbz").await;
    let (pause, cancel) = no_flags();
    let first = world.run_once(pause, cancel).await.unwrap();
    let first_path =
        storage::library_root(world.data_dir.path()).join(first.output_relpath.unwrap());
    let first_bytes = std::fs::read(&first_path).unwrap();

    // Same manga, second chapter, new task: distinct file, no overwrite.
    let second_chapter = {
        let (manga_id, _) = upsert_manga(
            &world.pool,
            "scripted",
            "m1",
            "Manga Title",
            None,
            None,
            "ongoing",
        )
        .await
        .unwrap();
        upsert_chapter(
            &world.pool,
            &manga_id,
            "ch-two",
            "Chapter 2",
            Some(2.0),
            None,
        )
        .await
        .unwrap()
        .0
    };
    world.queue(&second_chapter, "cbz").await;
    let (pause2, cancel2) = no_flags();
    let second = world.run_once(pause2, cancel2).await.unwrap();
    assert_eq!(second.status, "completed");
    assert_eq!(
        std::fs::read(&first_path).unwrap(),
        first_bytes,
        "first artifact untouched"
    );
}
