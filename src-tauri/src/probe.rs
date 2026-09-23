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
