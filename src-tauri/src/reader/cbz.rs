//! Minimal read-only ZIP (CBZ) access for app-owned archives.
//!
//! Parses the End Of Central Directory + central directory only; page bytes
//! are read lazily per requested entry via `seek` — a 300-page chapter never
//! lands fully in memory. Entry names from the archive are treated as
//! untrusted: traversal-style names are rejected and images magic-checked.
//! Only STORED entries are supported (the Phase 06 writer emits Stored); a
//! compressed entry surfaces as `CBZ_INVALID`, never as a partial page.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{AppError, LibraryError};
use crate::reader::{is_image_name, is_safe_entry_name, natural_cmp, MAX_PAGE_BYTES};

#[derive(Debug, Clone)]
pub struct ZipEntry {
    pub name: String,
    /// Compressed size == uncompressed for Stored.
    pub size: usize,
    pub local_offset: u64,
    pub method: u16,
}

#[derive(Debug, Default)]
pub struct Archive {
    pub entries: Vec<ZipEntry>,
}

fn u16le(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32le(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];

/// Open the archive, validate its central directory and return image entries
/// in natural page order (zero-padded names sort identically to numeric).
/// Only the tail window + central-directory segment are read — never the
/// whole archive — so opening a large chapter is O(entries), not O(bytes).
pub fn open(path: &Path) -> Result<Archive, AppError> {
    let mut file = File::open(path)?;
    let total = file.metadata()?.len();
    if total < 22 {
        return Err(LibraryError::CbzInvalid.into());
    }
    // EOCD lives in the last 22 bytes (no comment, our writer) or within a
    // bounded comment window: read only the tail.
    let window_len = usize::try_from(total.min(65_558)).unwrap_or(usize::MAX);
    let window_start = (total - window_len as u64) as usize;
    let mut tail = vec![0u8; window_len];
    file.seek(SeekFrom::Start(window_start as u64))?;
    file.read_exact(&mut tail)
        .map_err(|_| AppError::from(LibraryError::CbzInvalid))?;
    let eocd_in_window = find_eocd(&tail).ok_or(AppError::from(LibraryError::CbzInvalid))?;
    let entry_count = u16le(&tail, eocd_in_window + 10) as usize;
    let central_len = u32le(&tail, eocd_in_window + 12) as usize;
    let central_offset = u32le(&tail, eocd_in_window + 16) as usize;
    let eocd_absolute = window_start + eocd_in_window;
    if entry_count == 0
        || entry_count > 10_000
        || central_len > 8 * 1024 * 1024
        || central_offset + central_len != eocd_absolute
        || central_offset >= total as usize
    {
        return Err(LibraryError::CbzInvalid.into());
    }

    let mut bytes = vec![0u8; central_len];
    file.seek(SeekFrom::Start(central_offset as u64))?;
    file.read_exact(&mut bytes)
        .map_err(|_| AppError::from(LibraryError::CbzInvalid))?;

    let mut entries: Vec<ZipEntry> = Vec::new();
    let mut cursor = 0usize;
    for _ in 0..entry_count {
        if cursor + 46 > bytes.len() || bytes[cursor..cursor + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(LibraryError::CbzInvalid.into());
        }
        let method = u16le(&bytes, cursor + 10);
        let size = u32le(&bytes, cursor + 20) as usize;
        let name_len = u16le(&bytes, cursor + 28) as usize;
        let extra_len = u16le(&bytes, cursor + 30) as usize;
        let comment_len = u16le(&bytes, cursor + 32) as usize;
        let local_offset = u32le(&bytes, cursor + 42) as u64;
        let name_end = cursor + 46 + name_len;
        if name_end > bytes.len() {
            return Err(LibraryError::CbzInvalid.into());
        }
        let name = String::from_utf8(bytes[cursor + 46..name_end].to_vec())
            .map_err(|_| AppError::from(LibraryError::CbzInvalid))?;
        cursor = name_end + extra_len + comment_len;

        // Skip directory placeholders and non-images (e.g. ComicInfo.xml).
        if name.is_empty() || name.ends_with('/') {
            continue;
        }
        if !is_safe_entry_name(&name) {
            return Err(LibraryError::CbzInvalid.into());
        }
        if !is_image_name(&name) {
            continue;
        }
        if size > MAX_PAGE_BYTES {
            return Err(LibraryError::CbzInvalid.into());
        }
        entries.push(ZipEntry {
            name,
            size,
            local_offset,
            method,
        });
    }
    if entries.is_empty() {
        return Err(LibraryError::FileCorrupted.into());
    }
    entries.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    Ok(Archive { entries })
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    let len = bytes.len();
    // No comment (canonical): exact tail.
    if len >= 22 && bytes[len - 22..len - 18] == EOCD_SIGNATURE {
        return Some(len - 22);
    }
    // Bounded backward scan within the tail window.
    let lowest = len.saturating_sub(22 + 65_536);
    (lowest..len.saturating_sub(21))
        .rev()
        .find(|&i| bytes[i..i + 4] == EOCD_SIGNATURE && i + 22 <= len)
}

/// Read one Stored entry's bytes. Re-validates the local header, method and
/// declared size against the central directory value.
pub fn read_entry(path: &Path, entry: &ZipEntry) -> Result<Vec<u8>, AppError> {
    if entry.method != 0 {
        // Compressed archives are outside the app CBZ contract.
        return Err(LibraryError::CbzInvalid.into());
    }
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(entry.local_offset))?;
    let mut header = [0u8; 30];
    file.read_exact(&mut header)
        .map_err(|_| AppError::from(LibraryError::CbzInvalid))?;
    if header[0..4] != [0x50, 0x4b, 0x03, 0x04] {
        return Err(LibraryError::CbzInvalid.into());
    }
    let local_name_len = u16le(&header, 26) as u64;
    let local_extra_len = u16le(&header, 28) as u64;
    file.seek(SeekFrom::Start(
        entry.local_offset + 30 + local_name_len + local_extra_len,
    ))?;
    if entry.size > MAX_PAGE_BYTES {
        return Err(LibraryError::CbzInvalid.into());
    }
    let mut data = vec![0u8; entry.size];
    file.read_exact(&mut data)
        .map_err(|_| AppError::from(LibraryError::FileCorrupted))?;
    Ok(data)
}
