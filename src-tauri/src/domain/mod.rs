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

/// Input for settings patch — unknown keys are rejected, values validated in service.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatchInput {
    pub default_reader_mode: Option<String>,
    pub default_reading_direction: Option<String>,
    pub chapter_concurrency: Option<i64>,
    pub image_concurrency: Option<i64>,
    pub output_format: Option<String>,
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[allow(dead_code)] // used by Phase 05 registration paths
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
