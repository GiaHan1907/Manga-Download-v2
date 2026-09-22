//! Streaming transfer of one page into app-managed staging: the body is
//! fetched through the adapter's guarded transport, written to a `*.part`
//! file, flushed, validated from disk, then atomically renamed within the
//! same directory. A page is never considered present until its final
//! renamed file exists and validates — interrupted `.part` data is discarded.

use std::path::Path;

use crate::downloader::image_validate::{self, ImageKind};
use crate::error::AppError;

/// Zero-padded staging name for a page index, e.g. `000007.png`.
pub fn staged_name(page_index: usize, extension: &str) -> String {
    format!("{page_index:06}.{extension}")
}

pub struct StagedPage {
    pub name: String,
    pub size_bytes: i64,
    pub checksum: String,
}

/// Fetch one page image and land it as a validated staged file. The caller
/// short-circuits already-checkpointed pages before invoking this. Returns a
/// safe error on any validation failure so the worker can decide retry/fail.
pub async fn fetch_page(
    staging_dir: &Path,
    page_index: usize,
    fetch: impl std::future::Future<Output = Result<Vec<u8>, crate::sources::adapter::SourceError>>,
) -> Result<StagedPage, AppError> {
    let bytes = fetch.await.map_err(AppError::Source)?;
    let kind: ImageKind =
        image_validate::validate(&bytes, crate::sources::http::IMAGE_MAX_BYTES as usize).map_err(
            |reason| AppError::validation(format!("Ảnh trang không hợp lệ ({reason}).")),
        )?;
    let name = staged_name(page_index, kind.extension());
    write_and_validate(staging_dir, &name, &bytes)
}

/// Write bytes to `<name>.part`, flush, validate the on-disk copy matches,
/// then atomically rename to the final staged name.
pub fn write_and_validate(
    staging_dir: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<StagedPage, AppError> {
    image_validate::validate(bytes, crate::sources::http::IMAGE_MAX_BYTES as usize)
        .map_err(|reason| AppError::validation(format!("Ảnh trang không hợp lệ ({reason}).")))?;
    let final_path = staging_dir.join(name);
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
            "staged page write mismatch",
        )));
    }
    // Replace any prior final file only within this task's own staging dir.
    if final_path.exists() {
        std::fs::remove_file(&final_path)?;
    }
    std::fs::rename(&part_path, &final_path)?;

    Ok(StagedPage {
        name: name.to_string(),
        size_bytes: bytes.len() as i64,
        checksum: image_validate::crc32_hex(bytes),
    })
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
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
        png.extend(std::iter::repeat_n(0x11u8, 48));
        let expected = image_validate::crc32_hex(&png);
        let page = fetch_page(dir.path(), 6, async { Ok(png.clone()) })
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
        let result = fetch_page(dir.path(), 0, async { Ok(b"<html>nope</html>".to_vec()) }).await;
        assert!(result.is_err());
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
