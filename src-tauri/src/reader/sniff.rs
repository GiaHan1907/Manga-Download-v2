//! Page dimension sniffing from image headers only (no decode). Used for the
//! manifest so the reader can lay out virtualized pages before loading bytes.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

pub fn size_of(bytes: &[u8]) -> Option<ImageSize> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) && bytes.len() >= 24 {
        // IHDR follows the signature: width/height at bytes 16..24 (big endian).
        let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        return Some(ImageSize { width, height });
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return jpeg_size(bytes);
    }
    if (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) && bytes.len() >= 10 {
        let width = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let height = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        return Some(ImageSize { width, height });
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return webp_size(bytes);
    }
    None
}

/// Walk JPEG segments to the first SOFn marker holding the frame header.
fn jpeg_size(bytes: &[u8]) -> Option<ImageSize> {
    let mut pos = 2usize;
    while pos + 4 < bytes.len() {
        if bytes[pos] != 0xff {
            pos += 1;
            continue;
        }
        let marker = bytes[pos + 1];
        // Standalone markers (fill, RSTn, SOI, EOI).
        if marker == 0xff || (0xd0..=0xd9).contains(&marker) {
            pos += 2;
            continue;
        }
        let segment_len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        if segment_len < 2 {
            return None;
        }
        // SOF0..SOF15 except DHT (c4) / JPG (c8) / DAC (cc).
        if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            if pos + 9 < bytes.len() {
                let height = u16::from_be_bytes([bytes[pos + 5], bytes[pos + 6]]) as u32;
                let width = u16::from_be_bytes([bytes[pos + 7], bytes[pos + 8]]) as u32;
                return Some(ImageSize { width, height });
            }
            return None;
        }
        pos += 2 + segment_len;
    }
    None
}

fn webp_size(bytes: &[u8]) -> Option<ImageSize> {
    let chunk = bytes.get(12..16)?;
    if chunk == b"VP8X" && bytes.len() >= 30 {
        let w = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
        let h = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
        return Some(ImageSize {
            width: w,
            height: h,
        });
    }
    if chunk == b"VP8L" && bytes.len() >= 25 {
        // Simple format: 14-bit width/height packed after the signature byte.
        let bits = u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
        let width = (bits & 0x3fff) + 1;
        let height = ((bits >> 14) & 0x3fff) + 1;
        return Some(ImageSize { width, height });
    }
    if chunk == b"VP8 " && bytes.len() >= 30 {
        // Lossy: sync code 0x9d012a then 16-bit dimensions at offset 26/28.
        if bytes[23..26] == [0x9d, 0x01, 0x2a] {
            let width = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3fff) as u32;
            let height = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3fff) as u32;
            return Some(ImageSize { width, height });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_dimensions() {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
        bytes.extend([0, 0, 0, 13, b'I', b'H', b'D', b'R']);
        bytes.extend(720u32.to_be_bytes());
        bytes.extend(1080u32.to_be_bytes());
        assert_eq!(
            size_of(&bytes),
            Some(ImageSize {
                width: 720,
                height: 1080
            })
        );
    }

    #[test]
    fn gif_and_jpeg_dimensions() {
        let mut gif = b"GIF89a".to_vec();
        gif.extend([100, 0, 50, 0]);
        assert_eq!(
            size_of(&gif),
            Some(ImageSize {
                width: 100,
                height: 50
            })
        );

        // Minimal SOF0-bearing JPEG-ish byte stream.
        let mut jpg = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x11, 0x22];
        jpg.extend([0xff, 0xc0, 0x00, 0x0d, 0x08]);
        jpg.extend(400u16.to_be_bytes());
        jpg.extend(300u16.to_be_bytes());
        jpg.extend([0x03]);
        assert_eq!(
            size_of(&jpg),
            Some(ImageSize {
                width: 300,
                height: 400
            })
        );
    }

    #[test]
    fn unknown_bytes_yield_no_size() {
        assert_eq!(size_of(b"<html>"), None);
        assert_eq!(size_of(&[]), None);
    }
}
