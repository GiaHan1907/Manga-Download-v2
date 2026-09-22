use async_trait::async_trait;
use url::Url;

use super::dto::{RemoteMangaDetail, RemoteSearchPage, SourceInfo};

/// Distinguishable provider outcomes; commands map these to the AppError
/// envelope so the UI can tell URL problems from source state from transport
/// failures from malformed payloads.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("malformed url")]
    MalformedUrl,
    #[error("url not owned by a registered source")]
    UnsupportedSource,
    #[error("source is disabled or unknown")]
    SourceDisabled,
    /// Part of the documented taxonomy; raised when UI asks an adapter for a
    /// capability it does not declare (UI pre-gates, so not constructed yet).
    #[allow(dead_code)]
    #[error("operation not supported by this source")]
    UnsupportedOperation,
    #[error("content not found at source")]
    NotFound,
    #[error("source rate limited: {0}")]
    RateLimited(String),
    #[error("source unreachable or timed out")]
    Network,
    #[error("source returned a malformed response")]
    Parse,
    #[error("source returned an error")]
    Upstream,
}

impl SourceError {
    pub fn code(&self) -> &'static str {
        match self {
            SourceError::MalformedUrl => "INVALID_URL",
            SourceError::UnsupportedSource => "UNSUPPORTED_SOURCE",
            SourceError::SourceDisabled => "SOURCE_DISABLED",
            SourceError::UnsupportedOperation => "UNSUPPORTED_OPERATION",
            SourceError::NotFound => "SOURCE_NOT_FOUND",
            SourceError::RateLimited(_) => "SOURCE_RATE_LIMITED",
            SourceError::Network => "SOURCE_NETWORK",
            SourceError::Parse => "SOURCE_PARSE_ERROR",
            SourceError::Upstream => "SOURCE_UPSTREAM",
        }
    }

    /// Static, actionable, user-safe message (never echoes provider bodies).
    pub fn safe_message(&self) -> &'static str {
        match self {
            SourceError::MalformedUrl => "URL không hợp lệ — không thể phân tích cú pháp.",
            SourceError::UnsupportedSource => "URL không thuộc nguồn nào được hỗ trợ.",
            SourceError::SourceDisabled => "Nguồn này đang tắt trong Settings → Sources.",
            SourceError::UnsupportedOperation => "Nguồn không hỗ trợ thao tác này.",
            SourceError::NotFound => "Không tìm thấy nội dung trên nguồn (có thể link đã hết hạn).",
            SourceError::RateLimited(_) => "Nguồn đang giới hạn lượt truy vấn — thử lại sau.",
            SourceError::Network => "Không kết nối được nguồn (mạng/chặn theo khu vực).",
            SourceError::Parse => "Phản hồi từ nguồn không đúng định dạng mong đợi.",
            SourceError::Upstream => "Nguồn trả về lỗi tạm thời — thử lại sau.",
        }
    }
}

/// One vetted, app-shipped provider. `resolve_url`/search results carry stable
/// source ids only; the renderer never supplies hosts, headers or endpoints.
#[async_trait]
pub trait SourceAdapter: Send + Sync {
    fn info(&self) -> SourceInfo;
    fn owns_url(&self, url: &Url) -> bool;
    async fn search(&self, query: &str, page: u32) -> Result<RemoteSearchPage, SourceError>;
    /// Extracts the stable manga id from a URL this adapter already owns.
    async fn resolve_url(&self, url: &Url) -> Result<String, SourceError>;
    async fn manga_detail(&self, source_manga_id: &str) -> Result<RemoteMangaDetail, SourceError>;
}
