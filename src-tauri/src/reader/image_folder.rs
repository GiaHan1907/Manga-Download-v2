//! Image-folder chapter access. The directory is app-owned (produced by the
//! Phase 06 folder exporter); enumeration is non-recursive, filters to known
//! image extensions and natural-sorts for a stable manifest identical in shape
//! to CBZ mode. Symlinked entries are rejected to prevent escaping the root.

use std::path::Path;

use crate::error::{AppError, LibraryError};
use crate::reader::{is_image_name, is_safe_entry_name, natural_cmp, MAX_PAGE_BYTES};

#[derive(Debug, Clone)]
pub struct FolderPage {
    pub name: String,
    pub size_bytes: u64,
}

pub fn list_pages(dir: &Path) -> Result<Vec<FolderPage>, AppError> {
    let mut pages: Vec<FolderPage> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        // Only regular files; never follow symlinks out of the chapter folder.
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_image_name(&name) {
            continue;
        }
        pages.push(FolderPage {
            name,
            size_bytes: entry.metadata()?.len(),
        });
    }
    if pages.is_empty() {
        return Err(LibraryError::FileCorrupted.into());
    }
    pages.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    Ok(pages)
}

pub fn read_page(dir: &Path, name: &str) -> Result<Vec<u8>, AppError> {
    // Same basename guard the CBZ path applies to untrusted entry names, then
    // the per-page size cap BEFORE any bytes are read (CBZ enforces it in
    // cbz::read_entry; a hand-placed oversized file must not be streamed).
    if !is_image_name(name) || !is_safe_entry_name(name) {
        return Err(LibraryError::OutsideRoot.into());
    }
    let path = dir.join(name);
    let size = std::fs::metadata(&path)?.len();
    if size as usize > MAX_PAGE_BYTES {
        return Err(LibraryError::FileCorrupted.into());
    }
    Ok(std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_page_caps_size_and_rejects_unsafe_names() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("000001.png");
        std::fs::write(&good, b"png-ish").unwrap();
        assert_eq!(read_page(dir.path(), "000001.png").unwrap(), b"png-ish");

        // Oversized page is refused before a single byte is streamed.
        let huge = dir.path().join("000002.png");
        let file = std::fs::File::create(&huge).unwrap();
        file.set_len(MAX_PAGE_BYTES as u64 + 1).unwrap();
        assert!(matches!(
            read_page(dir.path(), "000002.png"),
            Err(AppError::Library(LibraryError::FileCorrupted))
        ));

        assert!(matches!(
            read_page(dir.path(), "../escape.png"),
            Err(AppError::Library(LibraryError::OutsideRoot))
        ));
        assert!(read_page(dir.path(), "page.txt").is_err());
    }
}
