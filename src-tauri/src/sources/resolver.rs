use url::Url;

use super::adapter::{SourceAdapter, SourceError};
use super::registry::SourceRegistry;

/// Recognize-then-request: parse bounded user input and match it against
/// registered adapters by exact host + path rules. An unrecognized URL is
/// NEVER fetched, and substring host tricks (`api.example.org.evil`) fail
/// because adapters match parsed hosts only.
pub fn resolve_owned_url(
    registry: &SourceRegistry,
    raw: &str,
) -> Result<(std::sync::Arc<dyn SourceAdapter>, Url), SourceError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 {
        return Err(SourceError::MalformedUrl);
    }
    let parsed = Url::parse(trimmed).map_err(|_| SourceError::MalformedUrl)?;
    for adapter in registry.adapters() {
        if adapter.owns_url(&parsed) {
            return Ok((adapter.clone(), parsed));
        }
    }
    Err(SourceError::UnsupportedSource)
}
