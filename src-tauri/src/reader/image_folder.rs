//! Image-folder chapter access. The directory is app-owned (produced by the
//! Phase 06 folder exporter); enumeration is non-recursive, filters to known
//! image extensions and natural-sorts for a stable manifest identical in shape
//! to CBZ mode. Symlinked entries are rejected to prevent escaping the root.

use std::path::Path;

use crate::error::{AppError, LibraryError};
use crate::reader::{is_image_name, natural_cmp};

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
    if !is_image_name(name) || name.contains('/') || name.contains('\\') || name == ".." {
        return Err(LibraryError::OutsideRoot.into());
    }
    let bytes = std::fs::read(dir.join(name))?;
    Ok(bytes)
}
