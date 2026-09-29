use async_trait::async_trait;
use url::Url;

use super::dto::{PageManifest, RemoteHomeFeed, RemoteMangaDetail, RemoteSearchPage, SourceInfo};

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
    /// capability it does not declare.
    #[error("operation not supported by this source")]
    UnsupportedOperation,
    #[error("content not found at source")]
    NotFound,
    /// 401/403-class rejection: never auto-retried, never bypassed.
    #[error("source denied access")]
    PermissionDenied,
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
            SourceError::PermissionDenied => "SOURCE_FORBIDDEN",
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
            SourceError::PermissionDenied => {
                "Nguồn từ chối truy cập (yêu cầu đăng nhập, trả phí hoặc chặn client)."
            }
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
    /// The source's own front page rails (featured / newest / most-read).
    /// Default: unsupported — only sources whose homepage is actually parsed
    /// by a vetted adapter can feed the Home screen.
    async fn home_feed(&self) -> Result<RemoteHomeFeed, SourceError> {
        Err(SourceError::UnsupportedOperation)
    }
    /// Extracts the stable manga id from a URL this adapter already owns.
    async fn resolve_url(&self, url: &Url) -> Result<String, SourceError>;
    /// Recognizes a CHAPTER link of this source and returns
    /// (source_manga_id, source_chapter_id). Default: not supported — manga
    /// level import keeps using `resolve_url`.
    fn chapter_from_url(&self, _url: &Url) -> Option<(String, String)> {
        None
    }
    async fn manga_detail(&self, source_manga_id: &str) -> Result<RemoteMangaDetail, SourceError>;
    /// Ordered page manifest for one chapter. Adapters override only when the
    /// source lawfully exposes page images; the default refuses so an
    /// unvetted capability can never be live by accident.
    async fn page_manifest(
        &self,
        _source_manga_id: &str,
        _source_chapter_id: &str,
    ) -> Result<PageManifest, SourceError> {
        Err(SourceError::UnsupportedOperation)
    }
    /// Fetches one page image through the adapter's own guarded transport.
    /// The url must come from a manifest this adapter produced.
    async fn fetch_image(&self, _url: &str) -> Result<Vec<u8>, SourceError> {
        Err(SourceError::UnsupportedOperation)
    }
    /// Fetches a cover image for a `cover_key` this adapter previously
    /// published (adapters rebuild the real URL internally — the key never
    /// round-trips a full URL). Default: unsupported; the UI degrades to a
    /// placeholder instead of ever widening hosts.
    async fn fetch_cover(
        &self,
        _source_manga_id: &str,
        _cover_key: &str,
    ) -> Result<Vec<u8>, SourceError> {
        Err(SourceError::UnsupportedOperation)
    }
}
