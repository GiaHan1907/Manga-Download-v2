//! Manual LIVE probe for the R-1 journey (release-test-matrix blocker).
//! Drives the REAL production path end to end — source registration (live
//! Madara validation), search, manga save, chapter queueing, the actual
//! Coordinator workers over GuardedHttp, and the verified-CBZ export —
//! against a COPY of the app data directory. Never touches the live one.
//!
//! Requires network + explicit opt-in:
//!   MANGA_LIVE_APP_DATA="<copy dir>" \
//!   MANGA_LIVE_SOURCE_URL="https://example.org" MANGA_LIVE_QUERY="title" \
//!   cargo test --lib -- --ignored --nocapture live_r1
//!
//! NOTE on policy: a source whose theme differs from Madara parses 0
//! chapters, and images hosted on any host OTHER than the registered one
//! fail the manifest by design (single exact-host allowlist per source) —
//! both are honest rejections, not transport failures.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::db::init_pool;
use crate::db::repositories::{download_repo, source_repo, task_repo};
use crate::downloader::progress::{CollectingSink, ProgressEmitter};
use crate::downloader::scheduler::Coordinator;
use crate::downloader::storage;
use crate::reader::cbz;
use crate::services::discovery_service;
use crate::sources::registry::SourceRegistry;

fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

fn looks_like_chapter_one(title: &str, number: Option<f64>) -> bool {
    if number == Some(1.0) {
        return true;
    }
    let normalized = title
        .to_lowercase()
        .replace("chương", " ")
        .replace("chapter", " ")
        .replace("tập", " ")
        .replace("tap", " ")
        .replace(':', " ");
    normalized.split_whitespace().next() == Some("1")
}

#[tokio::test]
#[ignore = "live network probe (R-1): needs MANGA_LIVE_APP_DATA pointing at a COPY of the app data dir"]
async fn live_r1_download_journey() {
    let Ok(dir) = std::env::var("MANGA_LIVE_APP_DATA") else {
        panic!("set MANGA_LIVE_APP_DATA to a COPY of the app data directory");
    };
    let source_url = env_or("MANGA_LIVE_SOURCE_URL", "https://truyentuoitho.com");
    let source_name = env_or("MANGA_LIVE_SOURCE_NAME", "Truyen Tuoi Tho");
    let query = env_or("MANGA_LIVE_QUERY", "ninja loan thi");
    let title_hint = env_or("MANGA_LIVE_TITLE_HINT", "ninja");
    let image_hosts: Vec<String> = env_or("MANGA_LIVE_IMAGE_HOSTS", "img.resourcehub.shop")
        .split(',')
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect();
    let app_data = PathBuf::from(dir);
    let pool = init_pool(&app_data).await.expect("init_pool on copy");
    let _ = crate::downloader::recovery::reconcile_startup(&pool, &app_data).await;

    // 1) Register the source through the production service (idempotent —
    //    a previously registered copy returns the existing row's conflict
    //    which we tolerate).
    let rows = source_repo::list_sources(&pool).await.unwrap();
    let registry = SourceRegistry::from_rows(&rows);
    match discovery_service::add_manga_source(
        &registry,
        &pool,
        &source_name,
        &source_url,
        Some(image_hosts),
    )
    .await
    {
        Ok(view) => println!("PROBE source registered: {}", view.info.id),
        Err(error) => println!("PROBE add_source returned {error:?} (continuing)"),
    }
    let rows = source_repo::list_sources(&pool).await.unwrap();
    let registry = SourceRegistry::from_rows(&rows);
    let source_id = rows
        .iter()
        .find(|row| {
            row.base_url
                .as_deref()
                .is_some_and(|base| base.trim_end_matches('/') == source_url.trim_end_matches('/'))
        })
        .map(|row| row.id.clone())
        .expect("source row must exist after registration");
    println!("PROBE using source id {source_id}");

    // 2) Search through the adapter (GuardedHttp pacing active).
    let found =
        discovery_service::search(&registry, &pool, &query, Some(vec![source_id.clone()]), 1)
            .await
            .expect("search");
    for item in &found.items {
        println!("PROBE hit: {} | {}", item.source_manga_id, item.title);
    }
    if let Some(error) = found.source_errors.first() {
        println!(
            "PROBE search source error: {} [{}] {}",
            error.source_id, error.code, error.message
        );
    }
    let hint = title_hint.to_lowercase();
    let target = found
        .items
        .iter()
        .find(|item| item.title.to_lowercase().contains(&hint))
        .expect("no matching result");

    // 3) Save the manga (production path) and fetch chapters.
    let saved = discovery_service::save(&registry, &pool, &source_id, &target.source_manga_id)
        .await
        .expect("save");
    println!(
        "PROBE saved manga {} ({} chapters)",
        saved.manga_id, saved.saved_chapter_count
    );
    let detail =
        discovery_service::remote_detail(&registry, &pool, &source_id, &target.source_manga_id)
            .await
            .expect("remote_detail");
    assert!(
        !detail.detail.chapters.is_empty(),
        "source registered and searched, but the adapter parsed ZERO chapters — \
         its theme does not match the Madara selectors (honest failure)"
    );
    for chapter in &detail.detail.chapters {
        println!(
            "PROBE chapter src={} number={:?} title={}",
            chapter.source_chapter_id, chapter.number, chapter.title
        );
    }
    let picked = detail
        .detail
        .chapters
        .iter()
        .find(|chapter| looks_like_chapter_one(&chapter.title, chapter.number))
        .expect("no chapter-1-looking entry");
    let (chapter_row_id,): (String,) =
        sqlx::query_as("SELECT id FROM chapters WHERE manga_id = ?1 AND source_chapter_id = ?2")
            .bind(&saved.manga_id)
            .bind(&picked.source_chapter_id)
            .fetch_one(&pool)
            .await
            .expect("saved manga must contain the picked chapter");

    // 4) Queue the chapter and start the REAL coordinator over this DB.
    let outcome =
        download_repo::queue_batch(&pool, std::slice::from_ref(&chapter_row_id), "cbz", 0)
            .await
            .expect("queue_batch");
    assert!(
        outcome.errors.is_empty(),
        "queue rejected: {:?}",
        outcome.errors
    );
    let task_row = outcome
        .created
        .first()
        .or(outcome.reused.first())
        .expect("no task produced");
    let task_id = task_row.id.clone();
    println!("PROBE queued task {task_id}");

    let events = Arc::new(ProgressEmitter::new(Arc::new(CollectingSink::default())));
    let shared_registry = Arc::new(Mutex::new(registry));
    let coordinator = Coordinator::new(
        pool.clone(),
        shared_registry,
        app_data.clone(),
        Arc::clone(&events),
    );
    coordinator.start();
    coordinator.notify_new_work();

    // 5) Poll to terminal state (30 min budget: ~30 pages at >=3 s/host plus
    //    retries is minutes, this only guards pathological sources).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30 * 60);
    let mut final_dto = None;
    let mut last_line = String::new();
    while std::time::Instant::now() < deadline {
        let row = download_repo::get_task(&pool, &task_id)
            .await
            .unwrap()
            .expect("task exists");
        let dto = task_repo::to_dto(row);
        let line = format!(
            "{} {}/{} attempts={}",
            dto.status, dto.completed_pages, dto.total_pages, dto.attempts
        );
        if line != last_line {
            println!("PROBE progress: {line}");
            last_line = line;
        }
        if matches!(dto.status.as_str(), "completed" | "failed" | "cancelled") {
            final_dto = Some(dto);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    coordinator.shutdown().await;

    let dto = final_dto.expect("task never reached a terminal state within 30 min");
    println!(
        "PROBE final: status={} pages={}/{} error={:?} relpath={:?}",
        dto.status, dto.completed_pages, dto.total_pages, dto.error_message, dto.output_relpath
    );
    assert_eq!(dto.status, "completed", "live download must complete");

    // 6) The artifact must exist under the library root and parse as CBZ.
    let relpath = dto
        .output_relpath
        .clone()
        .expect("completed without relpath");
    let absolute = storage::library_root(&app_data).join(&relpath);
    let size = std::fs::metadata(&absolute)
        .expect("artifact missing")
        .len();
    let archive = cbz::open(&absolute).expect("exported artifact must parse");
    let first_page = cbz::read_entry(&absolute, &archive.entries[0]).expect("first page readable");
    println!(
        "PROBE artifact: {} bytes, {} pages, page0 {} bytes",
        size,
        archive.entries.len(),
        first_page.len()
    );
    assert_eq!(archive.entries.len() as i64, dto.total_pages);
    assert!(first_page.len() > 20, "page0 too small to be an image");
}

/// Paste-a-link rehearsal against the REAL app data dir while the desktop app
/// is running: this process performs exactly what `queue_chapter_download_by_url`
/// does (classify link, ensure the manga+chapter rows, queue), then waits for
/// THE APP's own dispatcher (1 s tick) to execute the task. Proves the whole
/// live UI path end to end without clicking for the user.
///
///   MANGA_LIVE_APP_DATA="<real appdata dir>" MANGA_LIVE_PASTE_URL="<chapter url>" \
///   cargo test --lib -- --ignored --nocapture live_paste
#[tokio::test]
#[ignore = "live paste-link rehearsal: needs the desktop app running + env"]
async fn live_paste_link_onto_running_app() {
    let dir =
        std::env::var("MANGA_LIVE_APP_DATA").expect("MANGA_LIVE_APP_DATA = real app data dir");
    let url = std::env::var("MANGA_LIVE_PASTE_URL").expect("MANGA_LIVE_PASTE_URL = chapter link");
    let app_data = PathBuf::from(dir);
    let pool = init_pool(&app_data).await.expect("init_pool");
    let rows = source_repo::list_sources(&pool).await.unwrap();
    let registry = SourceRegistry::from_rows(&rows);

    let link = discovery_service::find_chapter_link(&registry, &url)
        .expect("link must classify (source registered? chapter-level https url?)");
    println!("LIVEPASTE link classified: {:?}", link);
    let chapter_id = discovery_service::ensure_chapter_row(&registry, &pool, &link)
        .await
        .expect("manga save + chapter row");
    let outcome = download_repo::queue_batch(&pool, std::slice::from_ref(&chapter_id), "cbz", 0)
        .await
        .expect("queue_batch");
    assert!(
        outcome.errors.is_empty(),
        "queue rejected: {:?}",
        outcome.errors
    );
    let task_id = outcome
        .created
        .first()
        .or(outcome.reused.first())
        .expect("no task")
        .id
        .clone();
    println!("LIVEPASTE queued task {task_id} — app dispatcher takes over");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40 * 60);
    let mut last_line = String::new();
    loop {
        let row = download_repo::get_task(&pool, &task_id)
            .await
            .unwrap()
            .expect("task exists");
        let dto = task_repo::to_dto(row);
        let line = format!("{} {}/{}", dto.status, dto.completed_pages, dto.total_pages);
        if line != last_line {
            println!("LIVEPASTE {line}");
            last_line = line;
        }
        if matches!(dto.status.as_str(), "completed" | "failed" | "cancelled") {
            assert_eq!(
                dto.status, "completed",
                "app worker failed: {:?}",
                dto.error_message
            );
            let relpath = dto
                .output_relpath
                .clone()
                .expect("completed without relpath");
            let abs = storage::library_root(&app_data).join(&relpath);
            let size = std::fs::metadata(&abs).expect("artifact missing").len();
            let archive = cbz::open(&abs).expect("artifact parses");
            println!(
                "LIVEPASTE DONE: {relpath} — {size} bytes, {} pages",
                archive.entries.len()
            );
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "app never executed the task — is the desktop app running?"
        );
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

/// End-to-end proof of the encrypted-reader fix through the app's OWN
/// construction: the built-in adapter is built by `SourceRegistry::from_rows`
/// from a row whose `image_hosts` is NULL (exactly what a merely-toggled source
/// looks like), then the real Coordinator downloads one whole script-rendered
/// chapter and exports a verified CBZ.
#[tokio::test]
#[ignore = "live probe: download a TTT script-rendered chapter end to end"]
async fn live_ttt_encrypted_chapter_downloads() {
    let dir = std::env::var("MANGA_LIVE_APP_DATA").unwrap_or_else(|_| {
        let path = std::env::temp_dir().join("manga1989-ttt-js");
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path.to_string_lossy().to_string()
    });
    let app_data = PathBuf::from(dir);
    let pool = init_pool(&app_data).await.expect("init_pool");
    let rows = vec![crate::db::repositories::source_repo::SourceRow {
        id: "madara-truyentuoitho-com".to_string(),
        display_name: crate::sources::registry::TTT_DISPLAY_NAME.to_string(),
        adapter: "madara".to_string(),
        base_url: None,
        image_hosts: None,
        enabled: true,
    }];
    let registry = SourceRegistry::from_rows(&rows);
    let saved = discovery_service::save(
        &registry,
        &pool,
        "madara-truyentuoitho-com",
        "ninja-loan-thi",
    )
    .await
    .expect("save through the built-in registry");
    println!(
        "JS-Journey saved manga {} ({} chapters)",
        saved.manga_id, saved.saved_chapter_count
    );
    let (chapter_id,): (String,) = sqlx::query_as(
        "SELECT id FROM chapters WHERE manga_id = ?1 AND source_chapter_id = 'tap-56'",
    )
    .bind(&saved.manga_id)
    .fetch_one(&pool)
    .await
    .expect("tap-56 must be saved");
    let outcome = download_repo::queue_batch(&pool, &[chapter_id], "cbz", 0)
        .await
        .expect("queue");
    assert!(
        outcome.errors.is_empty(),
        "queue rejected: {:?}",
        outcome.errors
    );
    let task_id = outcome.created.first().expect("a task").id.clone();

    let events = Arc::new(ProgressEmitter::new(Arc::new(CollectingSink::default())));
    let coordinator = Coordinator::new(
        pool.clone(),
        Arc::new(Mutex::new(registry)),
        app_data.clone(),
        Arc::clone(&events),
    );
    coordinator.start();
    coordinator.notify_new_work();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20 * 60);
    let mut final_dto = None;
    let mut last = String::new();
    while std::time::Instant::now() < deadline {
        let row = download_repo::get_task(&pool, &task_id)
            .await
            .unwrap()
            .expect("task exists");
        let dto = task_repo::to_dto(row);
        let line = format!(
            "{} {}/{} attempts={}",
            dto.status, dto.completed_pages, dto.total_pages, dto.attempts
        );
        if line != last {
            println!("JS-Journey {line}");
            last = line;
        }
        if matches!(dto.status.as_str(), "completed" | "failed" | "cancelled") {
            final_dto = Some(dto);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    coordinator.shutdown().await;

    let dto = final_dto.expect("the chapter never reached a terminal state");
    println!(
        "JS-Journey final: status={} pages={}/{} error={:?} relpath={:?}",
        dto.status, dto.completed_pages, dto.total_pages, dto.error_message, dto.output_relpath
    );
    assert_eq!(
        dto.status, "completed",
        "a JS-rendered chapter must download"
    );
    let relpath = dto.output_relpath.expect("completed task has a path");
    let artifact = storage::library_root(&app_data).join(&relpath);
    let size = std::fs::metadata(&artifact)
        .expect("artifact missing")
        .len();
    let archive = cbz::open(&artifact).expect("exported artifact must parse");
    let first_page = cbz::read_entry(&artifact, &archive.entries[0]).expect("page 1 readable");
    println!(
        "JS-Journey artifact: {size} bytes, {} pages, page0 {} bytes",
        archive.entries.len(),
        first_page.len()
    );
    assert_eq!(archive.entries.len() as i64, dto.total_pages);
    assert!(first_page.len() > 20, "page0 too small to be an image");
}

/// LIVE triage (opt-in, network): does TTT dual-serve the SAME image path on
/// both registered image hosts? Builds the tap-5 manifest, prints every page
/// URL host, then for the first page re-fetches with each image host spliced
/// in. Proves (or disproves) that a host failover can rescue a dead CDN shard.
#[tokio::test]
#[ignore = "live image-host dual-serve triage for TTT"]
async fn live_ttt_image_host_dual_serve_probe() {
    use url::Url;
    let rows = vec![source_repo::SourceRow {
        id: "madara-truyentuoitho-com".to_string(),
        display_name: crate::sources::registry::TTT_DISPLAY_NAME.to_string(),
        adapter: "madara".to_string(),
        base_url: None,
        image_hosts: None,
        enabled: true,
    }];
    let registry = SourceRegistry::from_rows(&rows);
    let adapter = registry.get("madara-truyentuoitho-com").unwrap();
    let manifest = adapter
        .page_manifest("ninja-loan-thi", "tap-5")
        .await
        .expect("tap-5 manifest");
    assert!(!manifest.pages.is_empty(), "tap-5 has no pages");
    let mut hosts: std::collections::BTreeMap<String, usize> = Default::default();
    for page in &manifest.pages {
        if let Ok(u) = Url::parse(&page.url) {
            *hosts
                .entry(u.host_str().unwrap_or("?").to_string())
                .or_default() += 1;
        }
    }
    println!(
        "DUAL manifest hosts: {hosts:?} ({} pages)",
        manifest.pages.len()
    );

    let first = Url::parse(&manifest.pages[0].url).expect("page0 url");
    let candidates = ["img.resourcehub.shop", "truyentuoitho.anooymi.workers.dev"];
    for host in candidates {
        let mut swapped = first.clone();
        let _ = swapped.set_host(Some(host));
        match adapter.fetch_image(swapped.as_str()).await {
            Ok(bytes) => println!("DUAL {host}: OK {} bytes", bytes.len()),
            Err(error) => println!("DUAL {host}: ERR {}", error.code()),
        }
    }
}

/// LIVE triage of the built-in NetTruyen journey: search → detail → chapter
/// manifest → image fetch → cover fetch, all through the shipped adapter and
/// its guarded transport. Optional overrides:
///   MANGA_LIVE_NT_SLUG / MANGA_LIVE_NT_CHAPTER
#[tokio::test]
#[ignore = "live journey probe for the built-in NetTruyen source"]
async fn live_nettruyen_journey_probe() {
    use url::Url;
    let registry = SourceRegistry::from_rows(&[]);
    let adapter = registry.get("nettruyen").expect("nettruyen is built in");

    let search = adapter
        .search("naruto", 1)
        .await
        .expect("NT search must succeed");
    println!(
        "NT search: {} items, has_more={}",
        search.items.len(),
        search.has_more
    );
    for item in search.items.iter().take(3) {
        println!(
            "NT item: {} | id={} | cover={:?}",
            item.title, item.source_manga_id, item.cover_key
        );
    }
    assert!(!search.items.is_empty(), "NT search returned nothing");

    let slug = std::env::var("MANGA_LIVE_NT_SLUG")
        .unwrap_or_else(|_| search.items[0].source_manga_id.clone());
    let detail = adapter
        .manga_detail(&slug)
        .await
        .expect("NT detail must succeed");
    println!(
        "NT detail {slug}: {} chapters, cover={:?}",
        detail.chapters.len(),
        detail.summary.cover_key
    );
    assert!(!detail.chapters.is_empty(), "NT detail has no chapters");

    let chapter = std::env::var("MANGA_LIVE_NT_CHAPTER")
        .unwrap_or_else(|_| detail.chapters[0].source_chapter_id.clone());
    let manifest = adapter
        .page_manifest(&slug, &chapter)
        .await
        .expect("NT manifest must succeed");
    let mut hosts: std::collections::BTreeMap<String, usize> = Default::default();
    for page in &manifest.pages {
        if let Ok(u) = Url::parse(&page.url) {
            *hosts
                .entry(u.host_str().unwrap_or("?").to_string())
                .or_default() += 1;
        }
    }
    println!(
        "NT manifest {chapter}: hosts={:?} ({} pages)",
        hosts,
        manifest.pages.len()
    );
    assert!(!manifest.pages.is_empty(), "NT manifest has no pages");

    let bytes = adapter
        .fetch_image(&manifest.pages[0].url)
        .await
        .expect("NT first page must fetch");
    println!("NT image fetch: {} bytes", bytes.len());
    assert!(bytes.len() > 2_000, "page image too small");

    if let Some(key) = detail.summary.cover_key.clone() {
        match adapter.fetch_cover(&slug, &key).await {
            Ok(bytes) => println!("NT cover fetch: {} bytes", bytes.len()),
            Err(error) => println!("NT cover fetch: ERR {}", error.code()),
        }
    }
}

/// LIVE self-heal of the fractional-chapter numbering fix: run the real watch
/// tick (the same path as the "Kiểm tra chương mới" button) against a COPY of
/// the app data, then check that the TruyenQQ "X.Y" chapters got their decimal
/// numbers back. Proves the stale rows are repaired by re-saving alone, without
/// touching the live database.
///   MANGA_LIVE_APP_DATA="<copy dir>" cargo test --lib -- --ignored --nocapture live_fractional_selfheal
#[tokio::test]
#[ignore = "live self-heal probe for dash-fractional chapter numbers"]
async fn live_fractional_selfheal_probe() {
    use crate::services::watch_service;
    let dir = std::env::var("MANGA_LIVE_APP_DATA").expect("MANGA_LIVE_APP_DATA = COPY of app data");
    let app_data = PathBuf::from(dir);
    let pool = init_pool(&app_data).await.expect("init_pool");
    let rows = source_repo::list_sources(&pool)
        .await
        .expect("list_sources");
    let registry = SourceRegistry::from_rows(&rows);
    let report = watch_service::run_tick(&pool, &registry, true, 50, true)
        .await
        .expect("forced tick");
    println!(
        "HEAL tick: checked={} alerted={} failed={}",
        report.checked, report.alerted_chapters, report.failed
    );
    let numbers: Vec<(String, Option<f64>)> = sqlx::query_as(
        "SELECT c.title, c.number FROM chapters c \
         JOIN mangas m ON m.id = c.manga_id \
         WHERE c.source_chapter_id LIKE '%chap-2-1' OR c.source_chapter_id LIKE '%chap-10-1' \
         ORDER BY c.title",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for (title, number) in &numbers {
        println!("HEAL {title} => {number:?}");
    }
    assert!(
        numbers
            .iter()
            .any(|(_, n)| *n == Some(2.1) || *n == Some(10.1)),
        "fractional numbers must be restored by the tick"
    );
}
