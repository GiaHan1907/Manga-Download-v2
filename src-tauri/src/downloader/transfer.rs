//! Streaming transfer of one page into app-managed staging: the body is
//! fetched through the adapter's guarded transport, written to a `*.part`
//! file, flushed, validated from disk, then atomically renamed within the
//! same directory. A page is never considered present until its final
//! renamed file exists and validates — interrupted `.part` data is discarded.

use std::path::Path;
use std::time::Duration;

use crate::downloader::image_validate;
use crate::error::AppError;

/// Zero-padded staging name for a page index, e.g. `000007.png`.
pub fn staged_name(page_index: usize, extension: &str) -> String {
    format!("{page_index:06}.{extension}")
}

#[derive(Debug)]
pub struct StagedPage {
    pub name: String,
    pub size_bytes: i64,
    pub checksum: String,
}

/// Fetch one page image and land it as a validated staged file. The caller
/// short-circuits already-checkpointed pages before invoking this. Returns a
/// safe error on any validation failure so the worker can decide retry/fail.
/// How many times one page may be fetched before its bytes are trusted to be
/// what the source really serves. A CDN occasionally answers a hot image with a
/// challenge page or a half-written edge copy (live 2026-10-27: one page of a
/// 102-page chapter came back non-image once, and the same URL served a valid
/// PNG seconds later); one refetch turns that into a stored page instead of
/// failing the whole chapter on a non-retryable VALIDATION_ERROR.
const PAGE_FETCH_ATTEMPTS: usize = 2;
const PAGE_REFETCH_DELAY: Duration = Duration::from_millis(400);

pub async fn fetch_page<F, Fut>(
    staging_dir: &Path,
    page_index: usize,
    fetch: F,
) -> Result<StagedPage, AppError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<Vec<u8>, crate::sources::adapter::SourceError>>,
{
    let mut attempt = 0usize;
    loop {
        attempt += 1;
        let bytes = fetch().await.map_err(AppError::Source)?;
        match image_validate::validate(&bytes, crate::sources::http::IMAGE_MAX_BYTES as usize) {
            Ok(kind) => {
                let name = staged_name(page_index, kind.extension());
                return write_and_validate(staging_dir, &name, &bytes);
            }
            Err(_) if attempt < PAGE_FETCH_ATTEMPTS => {
                tokio::time::sleep(PAGE_REFETCH_DELAY).await;
            }
            Err(reason) => {
                return Err(rejected_image(reason));
            }
        }
    }
}

fn rejected_image(reason: &'static str) -> AppError {
    AppError::image_rejected(format!("Ảnh trang không hợp lệ ({reason})."))
}

/// Write bytes to `<name>.part`, flush, validate the on-disk copy matches,
/// then atomically rename to the final staged name.
pub fn write_and_validate(
    staging_dir: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<StagedPage, AppError> {
    image_validate::validate(bytes, crate::sources::http::IMAGE_MAX_BYTES as usize)
        .map_err(rejected_image)?;
    let final_path = staging_dir.join(name);
    let part_path = staging_dir.join(format!("{name}.part"));
    std::fs::create_dir_all(staging_dir)
        .map_err(|error| AppError::io_stage("Không tạo được thư mục staging của chương", error))?;

    use std::io::Write;
    let mut file = std::fs::File::create(&part_path).map_err(|error| {
        AppError::io_stage("Không tạo được file trang (ổ đầy hoặc bị chặn?)", error)
    })?;
    file.write_all(bytes)
        .map_err(|error| AppError::io_stage("Không ghi được trang xuống đĩa", error))?;
    file.flush()
        .map_err(|error| AppError::io_stage("Không đẩy được dữ liệu trang xuống đĩa", error))?;
    drop(file);

    let written = std::fs::read(&part_path)
        .map_err(|error| AppError::io_stage("Không đọc lại được trang vừa ghi", error))?;
    if written.len() != bytes.len()
        || image_validate::crc32(&written) != image_validate::crc32(bytes)
    {
        let _ = std::fs::remove_file(&part_path);
        return Err(AppError::Io(std::io::Error::other(
            "staged page write mismatch",
        )));
    }
    // Replace any prior final file only within this task's own staging dir.
    if final_path.exists() {
        std::fs::remove_file(&final_path)
            .map_err(|error| AppError::io_stage("Không thay được trang cũ", error))?;
    }
    std::fs::rename(&part_path, &final_path)
        .map_err(|error| AppError::io_stage("Không đặt được tên cuối cho trang", error))?;

    Ok(StagedPage {
        name: name.to_string(),
        size_bytes: bytes.len() as i64,
        checksum: image_validate::crc32_hex(bytes),
    })
}

/// Land a single-file chapter (a scanned volume) in staging. Same
/// write-flush-reread-rename discipline as a page, but the bytes ARE the
/// artifact, so the caller validates format magic and size, and the staged
/// name keeps the artifact extension.
pub fn store_single_file(
    staging_dir: &Path,
    extension: &str,
    bytes: &[u8],
) -> Result<StagedPage, AppError> {
    let name = format!("chapter.{extension}");
    let final_path = staging_dir.join(&name);
    let part_path = staging_dir.join(format!("{name}.part"));
    std::fs::create_dir_all(staging_dir)?;

    use std::io::Write;
    let mut file = std::fs::File::create(&part_path)?;
    file.write_all(bytes)?;
    file.flush()?;
    drop(file);

    let written = std::fs::read(&part_path)?;
    if written.len() != bytes.len()
        || image_validate::crc32(&written) != image_validate::crc32(bytes)
    {
        let _ = std::fs::remove_file(&part_path);
        return Err(AppError::Io(std::io::Error::other(
            "staged file write mismatch",
        )));
    }
    if final_path.exists() {
        std::fs::remove_file(&final_path)?;
    }
    std::fs::rename(&part_path, &final_path)?;
    Ok(StagedPage {
        name,
        size_bytes: bytes.len() as i64,
        checksum: image_validate::crc32_hex(bytes),
    })
}

/// Byte-for-byte comparison in bounded chunks: used to verify a copied
/// single-file artifact without holding two whole volumes in memory.
pub fn files_identical(a: &Path, b: &Path) -> Result<bool, AppError> {
    use std::io::Read;
    let (mut fa, mut fb) = (std::fs::File::open(a)?, std::fs::File::open(b)?);
    let mut buf_a = vec![0u8; 64 * 1024];
    let mut buf_b = vec![0u8; 64 * 1024];
    loop {
        let n_a = fa.read(&mut buf_a)?;
        let n_b = fb.read(&mut buf_b)?;
        if n_a != n_b || buf_a[..n_a] != buf_b[..n_b] {
            return Ok(false);
        }
        if n_a == 0 {
            return Ok(true);
        }
    }
}

/// Delete leftover `*.part` files in a task-owned staging directory (bounded
/// to that single directory). Never touches final staged pages.
pub fn discard_part_files(staging_dir: &Path) -> std::io::Result<()> {
    if !staging_dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(staging_dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("part") {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn t06_07_fetch_page_validates_renames_and_reports_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend(std::iter::repeat_n(0x11u8, 48));
        let expected = image_validate::crc32_hex(&png);
        let page = fetch_page(dir.path(), 6, || async { Ok(png.clone()) })
            .await
            .unwrap();
        assert_eq!(page.name, staged_name(6, "png"));
        assert!(dir.path().join("000006.png").exists());
        assert!(!dir.path().join("000006.png.part").exists());
        assert_eq!(page.checksum, expected);
    }

    #[tokio::test]
    async fn html_body_never_lands_as_staged_page() {
        let dir = tempfile::tempdir().unwrap();
        let result = fetch_page(dir.path(), 0, || async {
            Ok(b"<html>nope</html>".to_vec())
        })
        .await;
        assert!(matches!(result, Err(AppError::ImageRejected(_))));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn one_bad_body_is_refetched_before_failing_the_page() {
        let dir = tempfile::tempdir().unwrap();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        png.extend(std::iter::repeat_n(0x22u8, 48));

        let seen = calls.clone();
        let page = fetch_page(dir.path(), 0, move || {
            let seen = seen.clone();
            let png = png.clone();
            async move {
                // First response is a CDN oddity; the retry is the real page.
                if seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    Ok(b"not-an-image".to_vec())
                } else {
                    Ok(png)
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(page.name, staged_name(0, "png"));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn persistent_garbage_is_reported_as_a_rejected_page() {
        let dir = tempfile::tempdir().unwrap();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = calls.clone();
        let error = fetch_page(dir.path(), 0, move || {
            let seen = seen.clone();
            async move {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(b"still-not-an-image".to_vec())
            }
        })
        .await
        .unwrap_err();

        assert!(matches!(error, AppError::ImageRejected(_)));
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            PAGE_FETCH_ATTEMPTS
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn part_cleanup_only_removes_parts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("000001.png"), b"x").unwrap();
        std::fs::write(dir.path().join("000002.png.part"), b"partial").unwrap();
        discard_part_files(dir.path()).unwrap();
        assert!(dir.path().join("000001.png").exists());
        assert!(!dir.path().join("000002.png.part").exists());
    }
}
