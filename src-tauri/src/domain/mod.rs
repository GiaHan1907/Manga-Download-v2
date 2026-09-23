use serde::{Deserialize, Serialize};

pub mod settings;

pub use settings::AppSettings;

pub const MAX_ID_LEN: usize = 64;
pub const MAX_QUERY_LEN: usize = 200;
pub const MAX_PAGE_LIMIT: i64 = 100;
pub const DEFAULT_PAGE_LIMIT: i64 = 20;

pub fn validate_id(value: &str, field: &str) -> Result<(), crate::error::AppError> {
    let ok = !value.is_empty()
        && value.len() <= MAX_ID_LEN
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(crate::error::AppError::validation(format!(
            "{field} không hợp lệ."
        )))
    }
}

pub fn validate_text_len(
    value: &str,
    max: usize,
    field: &str,
) -> Result<(), crate::error::AppError> {
    if value.len() <= max {
        Ok(())
    } else {
        Err(crate::error::AppError::validation(format!(
            "{field} vượt giới hạn độ dài."
        )))
    }
}

pub fn validate_page(limit: i64, offset: i64) -> Result<(), crate::error::AppError> {
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(crate::error::AppError::validation(
            "limit phải trong khoảng 1..=100.",
        ));
    }
    if !(0..=100_000).contains(&offset) {
        return Err(crate::error::AppError::validation("offset vượt giới hạn."));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaDto {
    pub id: String,
    pub source_id: String,
    pub source_manga_id: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub status: String,
    pub chapter_count: i64,
    pub favorite: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
    pub id: String,
    pub manga_id: String,
    pub source_chapter_id: String,
    pub title: String,
    pub number: Option<f64>,
    pub page_count: Option<i64>,
    /// True only when a verified artifact was committed (local_path set and
    /// confirmed present by startup recovery). Drives the Library badge.
    pub downloaded: bool,
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaDetailDto {
    #[serde(flatten)]
    pub manga: MangaDto,
    pub chapters: Vec<ChapterDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingProgressDto {
    pub manga_id: String,
    pub chapter_id: String,
    pub page_index: i64,
    /// Vertical reader restore hint (fraction 0..1 of the page height).
    pub scroll_offset: Option<f64>,
    /// Page total captured when the chapter was opened (completion %).
    pub page_total: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTaskDto {
    pub id: String,
    pub chapter_id: String,
    pub status: String,
    pub target_format: String,
    pub priority: i64,
    pub completed_pages: i64,
    pub total_pages: i64,
    pub attempts: i64,
    pub next_retry_at: Option<String>,
    pub error_code: Option<String>,
    /// Sanitized, user-safe failure description (never paths/SQL/stack).
    pub error_message: Option<String>,
    /// Monotonic per-task counter for event ordering (out-of-order defense).
    pub revision: i64,
    pub finished_at: Option<String>,
    /// Library-root-relative path of the verified artifact (completed only).
    pub output_relpath: Option<String>,
    /// Display-only names joined server-side (identity stays in the ids).
    pub chapter_title: Option<String>,
    pub manga_title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Event payload pushed on the `download-task-updated` channel. `task` is
/// the authoritative persisted snapshot; byte/speed values are live-only
/// measurements (never persisted, never fabricated when unmeasured).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTaskEvent {
    pub task: DownloadTaskDto,
    pub downloaded_bytes: u64,
    pub speed_bytes_per_second: Option<u64>,
    pub occurred_at: String,
}

/// Result of a batch queue submission (per-item outcomes, no partial lies).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueErrorDto {
    pub chapter_id: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueResultDto {
    pub tasks: Vec<DownloadTaskDto>,
    pub reused: Vec<DownloadTaskDto>,
    pub errors: Vec<QueueErrorDto>,
    pub queued_at: String,
}

/// Feature plan F2 — result of a download-history purge. `skipped_artifact`
/// counts completed tasks whose verified artifact is still on disk (their
/// history row doubles as the offline proof, so it is never deleted);
/// `skipped_other` counts rows a concurrent action moved out of the
/// deletable state mid-purge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearHistoryDto {
    pub deleted: i64,
    pub skipped_artifact: i64,
    pub skipped_other: i64,
}

/// Feature plan F4 — one per-chapter read record (metadata only).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadHistoryDto {
    pub manga_id: String,
    pub manga_title: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub chapter_number: Option<f64>,
    pub read_at: String,
}

/// Phase 07 — reader manifest. Deliberately path-free: pages are addressed by
/// index and served through `reader_get_page_bytes`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderPageDto {
    pub index: usize,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderManifestDto {
    pub chapter_id: String,
    pub manga_id: String,
    pub manga_title: String,
    pub chapter_title: String,
    pub format: String,
    pub page_count: usize,
    pub pages: Vec<ReaderPageDto>,
    pub prev_chapter_id: Option<String>,
    pub next_chapter_id: Option<String>,
    pub start_page: usize,
    pub start_scroll_offset: Option<f64>,
}

/// Phase 07 — library card (aggregated, single query, no N+1).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryProgressDto {
    pub chapter_id: String,
    pub page_index: i64,
    pub page_total: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMangaDto {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub status: String,
    pub favorite: bool,
    pub downloaded_chapters: i64,
    pub total_chapters: i64,
    pub local_size_bytes: Option<i64>,
    pub last_read_at: Option<String>,
    pub last_download_at: Option<String>,
    pub progress: Option<LibraryProgressDto>,
    /// True when reconciliation previously found a recorded artifact missing.
    pub has_missing: bool,
    /// Feature plan F1: unacknowledged new-chapter alerts for this manga.
    pub new_alerts: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReportDto {
    pub checked: i64,
    pub healthy: i64,
    pub missing: i64,
    pub corrupted: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveMangaFilesDto {
    pub removed: Vec<String>,
    pub failed: Vec<String>,
}

/// Input for settings patch — unknown keys are rejected, values validated in service.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatchInput {
    pub default_reader_mode: Option<String>,
    pub default_reading_direction: Option<String>,
    pub chapter_concurrency: Option<i64>,
    pub image_concurrency: Option<i64>,
    pub output_format: Option<String>,
    pub notifications_enabled: Option<bool>,
    pub close_to_tray: Option<bool>,
    pub watch_enabled: Option<bool>,
}

/// Phase 08 — Home overview aggregates (single SQL pass, no N+1).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatsDto {
    pub saved_mangas: i64,
    pub downloaded_chapters: i64,
    pub local_size_bytes: i64,
    pub favorites: i64,
}

/// Phase 08 — storage accounting. Sizes are approximate by construction
/// (DB-recorded artifact sizes + fs walk of app-owned cache roots); no
/// absolute paths cross the IPC boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatusDto {
    pub library_bytes: i64,
    pub cache_bytes: i64,
    pub staging_dirs: i64,
    pub active_tasks: i64,
    pub root_available: bool,
    pub approximate: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheCleanupDto {
    pub removed_dirs: i64,
    pub freed_bytes: i64,
    pub kept_active_dirs: i64,
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[allow(dead_code)] // used by Phase 05 registration paths
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
