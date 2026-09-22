//! Byte-level validation of downloaded page images and CRC32 integrity
//! markers. A page is only ever checkpointed after these checks pass — an
//! HTTP 200 that carries HTML, is empty/truncated/oversized is rejected.

/// Accepted image types, detected from magic bytes (never from the URL
/// suffix). Extension is chosen from the verified type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Jpeg,
    Png,
    Gif,
    WebP,
}

impl ImageKind {
    pub fn extension(self) -> &'static str {
        match self {
            ImageKind::Jpeg => "jpg",
            ImageKind::Png => "png",
            ImageKind::Gif => "gif",
            ImageKind::WebP => "webp",
        }
    }
}

const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
const GIF87A: &[u8] = b"GIF87a";
const GIF89A: &[u8] = b"GIF89a";

/// Smallest byte length we will accept for a real (non-degenerate) page.
pub const MIN_IMAGE_BYTES: usize = 24;

/// Detect the image type from magic bytes; None means "not an accepted image"
/// (which includes HTML error pages that returned HTTP 200).
pub fn detect(bytes: &[u8]) -> Option<ImageKind> {
    if bytes.starts_with(PNG_MAGIC) {
        Some(ImageKind::Png)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(ImageKind::Jpeg)
    } else if bytes.starts_with(GIF87A) || bytes.starts_with(GIF89A) {
        Some(ImageKind::Gif)
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(ImageKind::WebP)
    } else {
        None
    }
}

/// Validate a fetched page body: bounded size, real image magic, and a
/// rejection of markup. Returns the detected kind on success.
pub fn validate(bytes: &[u8], max_bytes: usize) -> Result<ImageKind, &'static str> {
    if bytes.is_empty() {
        return Err("EMPTY");
    }
    if bytes.len() > max_bytes {
        return Err("OVERSIZED");
    }
    // A document that starts like markup is never a page image, even if a
    // provider returned it with HTTP 200. Checked before the size floor:
    // error pages are commonly small.
    let head = &bytes[..bytes.len().min(64)];
    let as_text = String::from_utf8_lossy(head);
    let lower = as_text.trim_start().to_ascii_lowercase();
    if lower.starts_with("<!doctype") || lower.starts_with('<') {
        return Err("NOT_IMAGE");
    }
    if bytes.len() < MIN_IMAGE_BYTES {
        return Err("TRUNCATED");
    }
    detect(bytes).ok_or("UNSUPPORTED_TYPE")
}

/// CRC32 (IEEE) — used as the cheap integrity marker persisted per staged
/// page and as the ZIP entry checksum during export verification.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (!(crc & 1)).wrapping_add(1); // 0 if lsb set else 1
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

pub fn crc32_hex(bytes: &[u8]) -> String {
    format!("{:08x}", crc32(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_magic() {
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
        assert_eq!(detect(&png), Some(ImageKind::Png));
        assert_eq!(detect(&[0xff, 0xd8, 0xff, 0xe0]), Some(ImageKind::Jpeg));
        assert_eq!(detect(b"GIF89a...."), Some(ImageKind::Gif));
        let mut webp = *b"RIFF\0\0\0\0WEBPVP8 ";
        webp[4..8].copy_from_slice(&10u32.to_le_bytes());
        assert_eq!(detect(&webp), Some(ImageKind::WebP));
        assert_eq!(detect(b"<html>"), None);
    }

    #[test]
    fn t06_08_rejects_html_empty_truncated_oversized() {
        assert_eq!(validate(b"<!DOCTYPE html><html>", 1024), Err("NOT_IMAGE"));
        assert_eq!(validate(b"   <p>error</p>", 1024), Err("NOT_IMAGE"));
        assert_eq!(validate(&[], 1024), Err("EMPTY"));
        assert_eq!(validate(b"tiny", 1024), Err("TRUNCATED"));
        let big = vec![0u8; 100];
        assert_eq!(validate(&big, 50), Err("OVERSIZED"));
    }

    #[test]
    fn valid_image_passes_and_kind_drives_extension() {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
        png.extend(std::iter::repeat_n(0x41, 40));
        let kind = validate(&png, 1024).unwrap();
        assert_eq!(kind.extension(), "png");
    }

    #[test]
    fn crc32_matches_known_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b"abc"), 0x3524_41c2);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414f_a339
        );
        assert_eq!(crc32_hex(b"123456789"), "cbf43926");
    }
}
