//! Cover-image pipeline: adapter-guarded fetch + a regenerable on-disk
//! cache under app-data `covers/`. Only opaque keys cross the IPC boundary —
//! full URLs never leave Rust, filesystem paths never enter the renderer,
//! and the cache lives entirely inside the app-owned covers directory.

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::db::repositories::source_repo;
use crate::downloader::image_validate;
use crate::error::AppError;
use crate::services::discovery_service::require_enabled;
use crate::sources::http::COVER_MAX_BYTES;
use crate::sources::registry::SourceRegistry;

/// Keys come from adapters (already host-guarded) but cross the IPC boundary
/// as raw user data — keep them bounded and control-character-free.
pub fn validate_cover_key(cover_key: &str) -> Result<(), AppError> {
    if cover_key.is_empty()
        || cover_key.len() > 512
        || cover_key.chars().any(|c| c.is_control() || c == '\u{7f}')
    {
        return Err(AppError::validation("Khoá ảnh bìa không hợp lệ."));
    }
    Ok(())
}

/// Deterministic cache identity (FNV-1a 64): stable across builds, and the
/// file name never echoes caller input. A theoretical collision would only
/// show a stale thumbnail — reads are confined to covers/ regardless.
fn cache_file(app_data_dir: &Path, source_id: &str, cover_key: &str) -> PathBuf {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in source_id
        .bytes()
        .chain(std::iter::once(b':'))
        .chain(cover_key.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let source_tag: String = source_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    app_data_dir
        .join("covers")
        .join(format!("{source_tag}-{hash:016x}.img"))
}

pub async fn cover_bytes(
    registry: &SourceRegistry,
    pool: &SqlitePool,
    app_data_dir: &Path,
    source_id: &str,
    source_manga_id: &str,
    cover_key: &str,
) -> Result<Vec<u8>, AppError> {
    validate_cover_key(cover_key)?;
    // Same gate every other discovery path uses, checked BEFORE the cache:
    // disabling a source must hide its covers even when a copy is cached.
    let rows = source_repo::list_sources(pool).await?;
    let adapter = require_enabled(registry, &rows, source_id)?;
    let path = cache_file(app_data_dir, source_id, cover_key);
    if let Ok(cached) = std::fs::read(&path) {
        if !cached.is_empty() && cached.len() as u64 <= COVER_MAX_BYTES {
            return Ok(cached);
        }
    }
    let bytes = adapter.fetch_cover(source_manga_id, cover_key).await?;
    image_validate::validate(&bytes, COVER_MAX_BYTES as usize)
        .map_err(|_| AppError::validation("Nguồn không trả về ảnh bìa hợp lệ."))?;
    // The cache is strictly best-effort: a failed write must never break
    // displaying the freshly fetched bytes.
    if let Some(dir) = path.parent() {
        let stem = path
            .file_stem()
            .map_or("cover", |s| s.to_str().unwrap_or("cover"));
        let tmp = dir.join(format!("{stem}.{}.writing", uuid::Uuid::new_v4()));
        if std::fs::create_dir_all(dir).is_ok() && std::fs::write(&tmp, &bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::source_repo;
    use crate::sources::adapter::{SourceAdapter, SourceError};
    use crate::sources::dto::{
        Capability, RemoteMangaDetail, RemoteSearchPage, SourceInfo, SourceStatus,
    };
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use url::Url;

    struct CoverAdapter {
        calls: AtomicUsize,
        payload: Vec<u8>,
    }

    fn png_like(seed: u8) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        bytes.extend(std::iter::repeat_n(seed, 64));
        bytes
    }

    #[async_trait]
    impl SourceAdapter for CoverAdapter {
        fn info(&self) -> SourceInfo {
            SourceInfo {
                id: "covertest".to_string(),
                display_name: "Cover Test".to_string(),
                capabilities: vec![Capability::Details],
                status: SourceStatus::Ready,
                attribution: None,
            }
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
        async fn manga_detail(
            &self,
            _source_manga_id: &str,
        ) -> Result<RemoteMangaDetail, SourceError> {
            Err(SourceError::UnsupportedOperation)
        }
        async fn fetch_cover(
            &self,
            _source_manga_id: &str,
            cover_key: &str,
        ) -> Result<Vec<u8>, SourceError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if cover_key == "__fail" {
                return Err(SourceError::NotFound);
            }
            Ok(self.payload.clone())
        }
    }

    fn registry_with(payload: Vec<u8>) -> (SourceRegistry, std::sync::Arc<CoverAdapter>) {
        let adapter = std::sync::Arc::new(CoverAdapter {
            calls: AtomicUsize::new(0),
            payload,
        });
        let registry = SourceRegistry::with_adapters(vec![adapter.clone()]);
        (registry, adapter)
    }

    #[tokio::test]
    async fn first_call_fetches_second_call_serves_from_cache() {
        let (_db, pool) = crate::db::test_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let (registry, adapter) = registry_with(png_like(7));
        let first = cover_bytes(
            &registry,
            &pool,
            dir.path(),
            "covertest",
            "m1",
            "/a/cover.jpg",
        )
        .await
        .unwrap();
        let second = cover_bytes(
            &registry,
            &pool,
            dir.path(),
            "covertest",
            "m1",
            "/a/cover.jpg",
        )
        .await
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            adapter.calls.load(Ordering::SeqCst),
            1,
            "cache must serve hit #2"
        );
        let cached = cache_file(dir.path(), "covertest", "/a/cover.jpg");
        assert_eq!(std::fs::read(&cached).unwrap(), first);
        // File names never leak the key: only [a-z0-9-] + hex + .img.
        let name = cached.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("covertest-") && name.ends_with(".img"));
        assert!(name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'));
        assert!(!name.contains("jpg") && !name.contains(".img.img"));
    }

    #[tokio::test]
    async fn non_image_payload_is_refused_and_not_cached() {
        let (_db, pool) = crate::db::test_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let (registry, _adapter) = registry_with(b"<html>nope</html>".to_vec());
        let error = cover_bytes(&registry, &pool, dir.path(), "covertest", "m1", "/a/x.jpg")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
        assert!(!cache_file(dir.path(), "covertest", "/a/x.jpg").exists());
    }

    #[tokio::test]
    async fn disabled_source_is_refused_even_with_a_cached_copy() {
        let (_db, pool) = crate::db::test_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let (registry, adapter) = registry_with(png_like(9));
        let bytes = cover_bytes(&registry, &pool, dir.path(), "covertest", "m1", "/a/c.jpg")
            .await
            .unwrap();
        assert!(!bytes.is_empty());
        source_repo::upsert_source(&pool, "covertest", "Cover Test", "fixture", None, None)
            .await
            .unwrap();
        source_repo::set_enabled(&pool, "covertest", false)
            .await
            .unwrap();
        let error = cover_bytes(&registry, &pool, dir.path(), "covertest", "m1", "/a/c.jpg")
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::Source(SourceError::SourceDisabled)
        ));
        assert_eq!(
            adapter.calls.load(Ordering::SeqCst),
            1,
            "the guard must run before any cache/adapter work"
        );
    }

    #[tokio::test]
    async fn invalid_keys_and_unknown_sources_fail_before_fetch() {
        let (_db, pool) = crate::db::test_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let (registry, adapter) = registry_with(png_like(1));
        assert!(
            cover_bytes(&registry, &pool, dir.path(), "covertest", "m1", "")
                .await
                .is_err()
        );
        assert!(
            cover_bytes(&registry, &pool, dir.path(), "covertest", "m1", "/x\u{1}y")
                .await
                .is_err()
        );
        assert!(cover_bytes(
            &registry,
            &pool,
            dir.path(),
            "covertest",
            "m1",
            &format!("/x{}", "y".repeat(600))
        )
        .await
        .is_err());
        assert!(
            cover_bytes(&registry, &pool, dir.path(), "othersrc", "m1", "/ok.jpg")
                .await
                .is_err()
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn adapter_errors_propagate_without_caching() {
        let (_db, pool) = crate::db::test_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let (registry, _adapter) = registry_with(png_like(2));
        let error = cover_bytes(&registry, &pool, dir.path(), "covertest", "m1", "__fail")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Source(SourceError::NotFound)));
        assert!(!cache_file(dir.path(), "covertest", "__fail").exists());
    }

    #[test]
    fn distinct_inputs_get_distinct_files() {
        let a = cache_file(Path::new("/x"), "s1", "/k.jpg");
        let b = cache_file(Path::new("/x"), "s2", "/k.jpg");
        let c = cache_file(Path::new("/x"), "s1", "/k2.jpg");
        assert_ne!(a, b);
        assert_ne!(a, c);
    }
}
