//! T09-44..48 measurable performance probes. These run inside the normal
//! suite with deliberately loose upper bounds (this runner's SSD/CPU) — the
//! point is reproducible NUMBERS printed as evidence (captured into
//! docs/phase-09/PERFORMANCE_BASELINE.md) plus a regression tripwire, not CI
//! micro-benchmarks. `cargo test --release perf` is NOT required: these run
//! in debug on purpose (worst-case, still bounded).

#![cfg(test)]

use std::time::Instant;

use sqlx::SqlitePool;

use crate::db::test_pool;
use crate::domain::now_iso;
use crate::reader::testkit;

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::raw_sql(sql).execute(pool).await.unwrap();
}

async fn seed_library(pool: &SqlitePool, mangas: usize, chapters_each: usize) {
    // One batched script: 1000 separate implicit transactions measured ~48 s
    // on this disk (per-commit fsync), which is a durability property, not a
    // query-shape signal. The seed itself is timed separately as evidence.
    let ts = now_iso();
    let mut script = String::from("BEGIN TRANSACTION;");
    script.push_str(&format!(
        "INSERT INTO sources (id, display_name, adapter, base_url, enabled, created_at, updated_at) \
         VALUES ('src-p','Perf','madara','https://perf.invalid',1,'{ts}','{ts}');"
    ));
    for m in 0..mangas {
        let manga_id = format!("pm-{m}");
        script.push_str(&format!(
            "INSERT INTO mangas (id, source_id, source_manga_id, title, author, status, created_at, updated_at) \
             VALUES ('{manga_id}','src-p','ps-{m}','Perf Manga {m}','Author {m}','ongoing','{ts}','{ts}');"
        ));
        for c in 0..chapters_each {
            let chapter_id = format!("{manga_id}-c{c}");
            let local = if (m + c) % 3 == 0 {
                format!("'pm-dir-{m}/ch-{c}.cbz','cbz',{}", 40_000_000 + c as i64)
            } else {
                "NULL,NULL,NULL".to_string()
            };
            script.push_str(&format!(
                "INSERT INTO chapters (id, manga_id, source_chapter_id, title, number, created_at, updated_at, local_path, output_format, local_size_bytes) \
                 VALUES ('{chapter_id}','{manga_id}','pc-{m}-{c}','Chapter {c}',{c}.0,'{ts}','{ts}',{local});"
            ));
        }
    }
    script.push_str("COMMIT;");
    exec(pool, &script).await;
}

/// T09-44/45: library aggregate + LIKE search over 1.000 manga / 10.000
/// chapters (plan dataset). Prints per-operation medians.
#[tokio::test]
async fn t09_perf_library_aggregates_and_search() {
    let (_dir, pool) = test_pool().await;
    let seed_start = Instant::now();
    seed_library(&pool, 1000, 10).await;
    let seed_ms = seed_start.elapsed().as_secs_f64() * 1000.0;

    let mut list_times = Vec::new();
    for _ in 0..3 {
        let start = Instant::now();
        let (items, total) = crate::services::library_service::list_mangas(
            &pool,
            None,
            Some("title"),
            Some("downloaded"),
            24,
            0,
        )
        .await
        .unwrap();
        list_times.push(start.elapsed().as_secs_f64() * 1000.0);
        assert!(!items.is_empty() && total > 900);
    }
    let mut search_times = Vec::new();
    for _ in 0..3 {
        let start = Instant::now();
        let (items, _) = crate::services::library_service::list_mangas(
            &pool,
            Some("Perf Manga 999"),
            Some("lastRead"),
            None,
            24,
            0,
        )
        .await
        .unwrap();
        search_times.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(items.len(), 1);
    }
    let sort_ms = |v: &mut Vec<f64>| v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sort_ms(&mut list_times);
    sort_ms(&mut search_times);
    println!(
        "PERF library: seed 1k×10 = {seed_ms:.0} ms | list median = {:.1} ms | search median = {:.1} ms",
        list_times[1], search_times[1]
    );
    // Loose tripwires (debug build, temp file DB): catch order-of-magnitude
    // regressions, not noise.
    assert!(
        list_times[1] < 1500.0,
        "library list median {list_times:?} ms"
    );
    assert!(
        search_times[1] < 500.0,
        "search median {search_times:?} ms — idx_mangas_title should carry the LIKE prefix-free scan only linearly"
    );
}

/// T09-46: opening a 300-page CBZ must be O(central directory), and a single
/// page read O(one entry) — the tail-window/seek design in one measurement.
#[test]
fn t09_perf_cbz300_open_and_page_read_on_demand() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library");
    std::fs::create_dir_all(&library).unwrap();
    let build_start = Instant::now();
    crate::reader::testkit::build_cbz(&dir.path().join("stg"), &library, "p/big.cbz", 300);
    let path = library.join("p/big.cbz");
    let archive_bytes = std::fs::metadata(&path).unwrap().len();

    let start = Instant::now();
    let archive = crate::reader::cbz::open(&path).unwrap();
    let open_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(archive.entries.len(), 300);

    let start = Instant::now();
    let page = crate::reader::cbz::read_entry(&path, &archive.entries[299]).unwrap();
    let page_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert!(!page.is_empty());

    println!(
        "PERF cbz: built 300 pages ({archive_bytes} bytes) in {:.0} ms | open = {open_ms:.2} ms | page read = {page_ms:.2} ms",
        build_start.elapsed().as_secs_f64() * 1000.0
    );
    assert!(
        open_ms < 250.0,
        "open must not scale with payload: {open_ms} ms"
    );
    assert!(page_ms < 50.0, "single page read {page_ms} ms");
}

/// T09-47/48 (DB-contention half): 2000 checkpoint writes through the shared
/// pool. Real 20-task live-throughput RAM/CPU numbers stay manual.
#[tokio::test]
async fn t09_perf_checkpoint_write_throughput() {
    let (_dir, pool) = test_pool().await;
    let (manga_id, chapters) = testkit::seed_manga(&pool, "perfq", 1).await;
    let _ = manga_id;
    let chapter_id = chapters[0].clone();
    let ts = now_iso();
    exec(
        &pool,
        &format!(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, completed_pages, total_pages, attempts, revision, created_at, updated_at) \
             VALUES ('pt-1','{chapter_id}','downloading','cbz',0,0,2000,0,1,'{ts}','{ts}');"
        ),
    )
    .await;
    let mut script = String::new();
    for index in 0..2000 {
        script.push_str(&format!(
            "INSERT INTO download_pages (task_id, page_index, status, created_at, updated_at) VALUES ('pt-1',{index},'pending','{ts}','{ts}');"
        ));
    }
    exec(&pool, &script).await;

    let start = Instant::now();
    for index in 0..2000u32 {
        sqlx::query(
            "UPDATE download_pages SET status='stored', storage_key='k', size_bytes=64, checksum='ab' \
             WHERE task_id='pt-1' AND page_index=?1",
        )
        .bind(index as i64)
        .execute(&pool)
        .await
        .unwrap();
    }
    let write_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "PERF checkpoint: 2000 single-row updates = {write_ms:.0} ms ({:.2} ms/write) — every update is its own durable commit by design (crash-safe checkpoints); on this runner fsync dominates.",
        write_ms / 2000.0
    );
    // Honest bound for per-page fsync on mechanical/AV-heavy disks. Changing
    // the commit-per-page durability contract is NOT on the table in P09.
    assert!(write_ms < 30_000.0, "checkpoint writes {write_ms:.0} ms");
}
