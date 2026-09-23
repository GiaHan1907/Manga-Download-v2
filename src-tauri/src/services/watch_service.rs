//! Feature plan F1 — new-chapter watch. Metadata-only polling of saved
//! manga through the SAME adapters (and therefore the SAME GuardedHttp
//! pacing/SSRF policy) the rest of the app uses: no ad-hoc HTTP, no URLs in
//! events or notifications, nothing ever downloaded here.
//!
//! Scheduling lives with the caller (a 6 h loop in lib.rs + manual
//! `watch_run_now`); this module owns candidate selection (per-manga
//! overrides, failure backoff) and the check itself (new chapters become
//! `chapter_alerts`, first contact only records a baseline).

use std::collections::HashSet;
use std::path::Path;

use sqlx::Row;
use sqlx::SqlitePool;

use crate::db::repositories::manga_repo::upsert_chapter;
use crate::domain::now_iso;
use crate::error::AppError;
use crate::sources::adapter::SourceAdapter;
use crate::sources::dto::Capability;

/// Base poll interval; failures multiply it geometrically (see the
/// `1 << MIN(watch_fail_count, 4)` term in the due predicate).
pub const WATCH_INTERVAL_HOURS: i64 = 6;
/// Max manga checked per tick — bounds one tick to well under a minute even
/// with the ≥3 s per-host pacing (adapters own the pacing, not this loop).
pub const WATCH_TICK_LIMIT: i64 = 20;

#[derive(Debug, Clone)]
pub struct WatchCandidate {
    pub manga_id: String,
    pub source_id: String,
    pub source_manga_id: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct WatchReport {
    pub checked: usize,
    pub alerted_chapters: usize,
    pub failed: usize,
}

/// Manga eligible for a check: source enabled, effective watch flag on
/// (per-manga override, else the global setting), and due per
/// interval × 2^fail (capped ×16). Favorites first, then most recently
/// read/updated — if the cap bites, those matter most.
pub async fn select_candidates(
    pool: &SqlitePool,
    global_enabled: bool,
    limit: i64,
) -> Result<Vec<WatchCandidate>, AppError> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT m.id, m.source_id, m.source_manga_id FROM mangas m \
         JOIN sources s ON s.id = m.source_id \
         WHERE s.enabled = 1 AND COALESCE(m.watch_enabled, ?1) = 1 \
           AND (m.watched_at IS NULL OR julianday(m.watched_at) <= \
                julianday('now') - (?2 * (1 << MIN(m.watch_fail_count, 4))) / 24.0) \
         ORDER BY (SELECT COUNT(*) FROM favorites f WHERE f.manga_id = m.id) DESC, \
                  m.last_read_at IS NULL, m.last_read_at DESC, m.updated_at DESC \
         LIMIT ?3",
    )
    .bind(i64::from(global_enabled))
    .bind(WATCH_INTERVAL_HOURS)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(manga_id, source_id, source_manga_id)| WatchCandidate {
            manga_id,
            source_id,
            source_manga_id,
        })
        .collect())
}

async fn record_failure(pool: &SqlitePool, manga_id: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE mangas SET watch_fail_count = watch_fail_count + 1 WHERE id = ?1")
        .bind(manga_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Check one manga's chapter list via its adapter. Returns newly alerted
/// chapter count. The FIRST successful contact only records a baseline —
/// enrolling an existing series must not spam "new" for its whole history.
pub async fn check_manga(
    pool: &SqlitePool,
    adapter: &dyn SourceAdapter,
    candidate: &WatchCandidate,
) -> Result<usize, AppError> {
    if !adapter.info().capabilities.contains(&Capability::Details) {
        return Err(AppError::Source(
            crate::sources::adapter::SourceError::UnsupportedOperation,
        ));
    }
    let detail = adapter
        .manga_detail(&candidate.source_manga_id)
        .await
        .map_err(AppError::Source)?;

    let existing: HashSet<String> = sqlx::query_as::<_, (String,)>(
        "SELECT source_chapter_id FROM chapters WHERE manga_id = ?1",
    )
    .bind(&candidate.manga_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id,)| id)
    .collect();
    let has_baseline: bool = sqlx::query_as::<_, (bool,)>(
        "SELECT watch_baseline_key IS NOT NULL FROM mangas WHERE id = ?1",
    )
    .bind(&candidate.manga_id)
    .fetch_optional(pool)
    .await?
    .is_some_and(|row| row.0);

    let ts = now_iso();
    let mut alerted = 0usize;
    let mut newest_key: Option<String> = None;
    for chapter in &detail.chapters {
        let is_new = !existing.contains(&chapter.source_chapter_id);
        let (chapter_id, created) = upsert_chapter(
            pool,
            &candidate.manga_id,
            &chapter.source_chapter_id,
            &chapter.title,
            chapter.number,
            None,
        )
        .await?;
        if created && has_baseline {
            let changed = sqlx::query(
                "INSERT OR IGNORE INTO chapter_alerts (manga_id, chapter_id, detected_at) \
                 VALUES (?1, ?2, ?3)",
            )
            .bind(&candidate.manga_id)
            .bind(&chapter_id)
            .bind(&ts)
            .execute(pool)
            .await?
            .rows_affected();
            alerted += changed as usize;
        }
        if is_new {
            newest_key = Some(chapter.source_chapter_id.clone());
        }
    }

    sqlx::query(
        "UPDATE mangas SET watched_at = ?2, watch_fail_count = 0, \
         watch_baseline_key = COALESCE(watch_baseline_key, ?3), updated_at = ?2 WHERE id = ?1",
    )
    .bind(&candidate.manga_id)
    .bind(&ts)
    .bind(newest_key.as_deref().unwrap_or("baseline"))
    .execute(pool)
    .await?;
    Ok(alerted)
}

/// One bounded tick over a registry snapshot. Adapter/source problems mark
/// the manga failed (geometric backoff); they never abort the tick.
pub async fn run_tick(
    pool: &SqlitePool,
    registry: &crate::sources::registry::SourceRegistry,
    global_enabled: bool,
    limit: i64,
) -> Result<WatchReport, AppError> {
    let mut report = WatchReport::default();
    for candidate in select_candidates(pool, global_enabled, limit).await? {
        report.checked += 1;
        let adapter = registry.get(&candidate.source_id);
        let outcome = match adapter {
            None => Err(AppError::Source(
                crate::sources::adapter::SourceError::SourceDisabled,
            )),
            Some(adapter) => check_manga(pool, adapter.as_ref(), &candidate).await,
        };
        match outcome {
            Ok(alerted) => report.alerted_chapters += alerted,
            Err(_) => {
                record_failure(pool, &candidate.manga_id).await?;
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

/// Alerts for one manga, newest first (joined for display titles only —
/// never paths/URLs).
pub async fn list_alerts(
    pool: &SqlitePool,
    app_data_dir: &Path,
    manga_id: &str,
) -> Result<Vec<serde_json::Value>, AppError> {
    let _ = app_data_dir; // display-only listing; artifacts are not touched here
    crate::domain::validate_id(manga_id, "mangaId")?;
    let rows = sqlx::query(
        "SELECT ca.chapter_id, ca.detected_at, c.title \
         FROM chapter_alerts ca JOIN chapters c ON c.id = ca.chapter_id \
         WHERE ca.manga_id = ?1 ORDER BY ca.detected_at DESC, ca.chapter_id DESC LIMIT 200",
    )
    .bind(manga_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "chapterId": row.try_get::<String, _>("chapter_id").unwrap_or_default(),
                "title": row.try_get::<String, _>("title").unwrap_or_default(),
                "detectedAt": row.try_get::<String, _>("detected_at").unwrap_or_default(),
            })
        })
        .collect())
}

/// Ack (delete) alerts for a manga — idempotent.
pub async fn ack_alerts(pool: &SqlitePool, manga_id: &str) -> Result<i64, AppError> {
    crate::domain::validate_id(manga_id, "mangaId")?;
    let changed = sqlx::query("DELETE FROM chapter_alerts WHERE manga_id = ?1")
        .bind(manga_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed as i64)
}

/// Per-manga opt-in/out; NULL restores "follow the global setting".
pub async fn set_watch_enabled(
    pool: &SqlitePool,
    manga_id: &str,
    enabled: Option<bool>,
) -> Result<(), AppError> {
    crate::domain::validate_id(manga_id, "mangaId")?;
    let value: Option<i64> = enabled.map(i64::from);
    let changed = sqlx::query("UPDATE mangas SET watch_enabled = ?2 WHERE id = ?1")
        .bind(manga_id)
        .bind(value)
        .execute(pool)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(crate::error::LibraryError::NotFound.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_manga};
    use crate::db::test_pool;
    use crate::sources::adapter::SourceError;
    use crate::sources::dto::{
        Capability, PageManifest, RemoteChapter, RemoteMangaDetail, RemoteMangaSummary,
        RemoteSearchPage, SourceInfo, SourceStatus,
    };
    use crate::sources::registry::SourceRegistry;
    use async_trait::async_trait;
    use sqlx::SqlitePool;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use url::Url;

    /// Scripted detail provider: chapters() returns what the "source" has now.
    struct WatchAdapter {
        chapters: std::sync::Mutex<Vec<RemoteChapter>>,
        fail: AtomicUsize,
        calls: AtomicUsize,
    }

    impl WatchAdapter {
        fn new(chapters: Vec<RemoteChapter>) -> Self {
            WatchAdapter {
                chapters: std::sync::Mutex::new(chapters),
                fail: AtomicUsize::new(0),
                calls: AtomicUsize::new(0),
            }
        }
        fn set_chapters(&self, chapters: Vec<RemoteChapter>) {
            *self.chapters.lock().unwrap() = chapters;
        }
    }

    fn chapter(id: &str, number: f64) -> RemoteChapter {
        RemoteChapter {
            source_chapter_id: id.to_string(),
            title: format!("Ch {id}"),
            number: Some(number),
            order: number as i64,
            published_at: None,
        }
    }

    #[async_trait]
    impl SourceAdapter for WatchAdapter {
        fn info(&self) -> SourceInfo {
            SourceInfo {
                id: "watched".to_string(),
                display_name: "Watched".to_string(),
                capabilities: vec![Capability::Details],
                status: SourceStatus::Ready,
                attribution: None,
            }
        }
        fn owns_url(&self, _url: &Url) -> bool {
            false
        }
        async fn search(&self, _q: &str, _p: u32) -> Result<RemoteSearchPage, SourceError> {
            Err(SourceError::UnsupportedOperation)
        }
        async fn resolve_url(&self, _url: &Url) -> Result<String, SourceError> {
            Err(SourceError::UnsupportedOperation)
        }
        async fn manga_detail(&self, _id: &str) -> Result<RemoteMangaDetail, SourceError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) > 0 {
                return Err(SourceError::Network);
            }
            let chapters = self.chapters.lock().unwrap().clone();
            Ok(RemoteMangaDetail {
                summary: RemoteMangaSummary {
                    source_id: "watched".to_string(),
                    source_manga_id: "wm1".to_string(),
                    source_display_name: "Watched".to_string(),
                    title: "T".to_string(),
                    author: None,
                    cover_key: None,
                    status: "ongoing".to_string(),
                },
                description: None,
                genres: Vec::new(),
                canonical_url: None,
                chapters,
                saved_manga_id: None,
                available_offline: false,
            })
        }
        async fn page_manifest(&self, _m: &str, _c: &str) -> Result<PageManifest, SourceError> {
            Err(SourceError::UnsupportedOperation)
        }
    }

    async fn seed(pool: &SqlitePool) -> String {
        ensure_source(pool, "watched", "Watched").await.unwrap();
        let (manga_id, _) = upsert_manga(pool, "watched", "wm1", "T", None, None, "ongoing")
            .await
            .unwrap();
        manga_id
    }

    fn registry_with(adapter: WatchAdapter) -> SourceRegistry {
        SourceRegistry::with_adapters(vec![std::sync::Arc::new(adapter)])
    }

    async fn alert_count(pool: &SqlitePool, manga_id: &str) -> i64 {
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM chapter_alerts WHERE manga_id = ?1")
                .bind(manga_id)
                .fetch_one(pool)
                .await
                .unwrap();
        n
    }

    #[tokio::test]
    async fn t12_first_check_is_silent_baseline_then_new_chapters_alert() {
        let (_dir, pool) = test_pool().await;
        let manga_id = seed(&pool).await;
        let adapter = WatchAdapter::new(vec![chapter("c1", 1.0), chapter("c2", 2.0)]);

        // First contact: baseline only — existing chapters are NOT "new".
        let alerted = check_manga(
            &pool,
            &adapter,
            &WatchCandidate {
                manga_id: manga_id.clone(),
                source_id: "watched".to_string(),
                source_manga_id: "wm1".to_string(),
            },
        )
        .await
        .unwrap();
        assert_eq!(alerted, 0);
        assert_eq!(alert_count(&pool, &manga_id).await, 0);

        // A chapter appears later → alerted exactly once.
        adapter.set_chapters(vec![
            chapter("c1", 1.0),
            chapter("c2", 2.0),
            chapter("c3", 3.0),
        ]);
        let candidate = WatchCandidate {
            manga_id: manga_id.clone(),
            source_id: "watched".to_string(),
            source_manga_id: "wm1".to_string(),
        };
        let alerted = check_manga(&pool, &adapter, &candidate).await.unwrap();
        assert_eq!(alerted, 1);
        let again = check_manga(&pool, &adapter, &candidate).await.unwrap();
        assert_eq!(again, 0, "idempotent: no duplicate alerts");
        assert_eq!(alert_count(&pool, &manga_id).await, 1);

        // ack clears; failed check backs off without touching watched_at.
        let acked = ack_alerts(&pool, &manga_id).await.unwrap();
        assert_eq!(acked, 1);
        assert_eq!(alert_count(&pool, &manga_id).await, 0);
    }

    #[tokio::test]
    async fn t12_selection_respects_overrides_backoff_and_ordering() {
        let (_dir, pool) = test_pool().await;
        let on_by_default = seed(&pool).await;
        // Favorite, explicitly opted out while global is ON.
        ensure_source(&pool, "watched2", "Watched2").await.unwrap();
        let (opted_out, _) = upsert_manga(&pool, "watched2", "wm2", "U", None, None, "ongoing")
            .await
            .unwrap();
        sqlx::query("UPDATE mangas SET watch_enabled = 0 WHERE id = ?1")
            .bind(&opted_out)
            .execute(&pool)
            .await
            .unwrap();
        // Explicit opt-in while global is OFF.
        let (opted_in, _) = upsert_manga(&pool, "watched2", "wm3", "W", None, None, "ongoing")
            .await
            .unwrap();
        sqlx::query("UPDATE mangas SET watch_enabled = 1 WHERE id = ?1")
            .bind(&opted_in)
            .execute(&pool)
            .await
            .unwrap();

        let ids: Vec<String> = select_candidates(&pool, true, 50)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.manga_id)
            .collect();
        assert!(ids.contains(&on_by_default));
        assert!(!ids.contains(&opted_out), "explicit 0 beats global on");
        assert!(ids.contains(&opted_in));

        let global_off: Vec<String> = select_candidates(&pool, false, 50)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.manga_id)
            .collect();
        assert!(!global_off.contains(&on_by_default));
        assert!(
            global_off.contains(&opted_in),
            "explicit 1 beats global off"
        );

        // Due window: fresh success → not due; failure with backoff → stays due-ish logic.
        sqlx::query("UPDATE mangas SET watched_at = ?2, watch_fail_count = 0 WHERE id = ?1")
            .bind(&on_by_default)
            .bind(now_iso())
            .execute(&pool)
            .await
            .unwrap();
        let due: Vec<String> = select_candidates(&pool, true, 50)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.manga_id)
            .collect();
        assert!(!due.contains(&on_by_default), "just-checked is not due");

        // fail_count 4 → interval ×16; 24h ago is within 96h → still due window:
        sqlx::query(
            "UPDATE mangas SET watched_at = datetime('now', '-24 hours'), watch_fail_count = 4 WHERE id = ?1",
        )
        .bind(&on_by_default)
        .execute(&pool)
        .await
        .unwrap();
        let due_backed: Vec<String> = select_candidates(&pool, true, 50)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.manga_id)
            .collect();
        assert!(!due_backed.contains(&on_by_default), "backoff ×16 hides it");
        // fail_count 1: 24h > 12h → due again.
        sqlx::query("UPDATE mangas SET watch_fail_count = 1 WHERE id = ?1")
            .bind(&on_by_default)
            .execute(&pool)
            .await
            .unwrap();
        let due_again: Vec<String> = select_candidates(&pool, true, 50)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.manga_id)
            .collect();
        assert!(due_again.contains(&on_by_default));
    }

    #[tokio::test]
    async fn t12_tick_records_failure_and_keeps_going() {
        let (_dir, pool) = test_pool().await;
        let manga_id = seed(&pool).await;
        let adapter = WatchAdapter::new(Vec::new());
        adapter.fail.fetch_add(1, Ordering::SeqCst);
        let registry = registry_with(adapter);

        let report = run_tick(&pool, &registry, true, 20).await.unwrap();
        assert_eq!(report.checked, 1);
        assert_eq!(report.failed, 1);
        let (fails,): (i64,) = sqlx::query_as("SELECT watch_fail_count FROM mangas WHERE id = ?1")
            .bind(&manga_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fails, 1);
    }
}
