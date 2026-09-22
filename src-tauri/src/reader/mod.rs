//! Offline chapter reading: CBZ (ZIP) and image-folder access plus page
//! metadata sniffing. Archive contents are treated as UNTRUSTED input:
//! entry names must be basename-only image files, sizes are capped, and bytes
//! are magic-validated before being served to the renderer. The renderer
//! never names a path — chapters are addressed by DB id only.

pub mod cbz;
pub mod image_folder;
pub mod sniff;
#[cfg(test)]
pub mod testkit;

/// Accepted page image extensions (lowercase).
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];

/// Hard cap for a single served page (matches download-side IMAGE_MAX_BYTES).
pub const MAX_PAGE_BYTES: usize = 20 * 1024 * 1024;

pub fn is_image_name(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Basename-only guard for archive entry names: rejects traversal
/// (`..`, separators) and absolute paths regardless of what a ZIP header
/// claims.
pub fn is_safe_entry_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != ".."
        && name != "."
        && !name.contains(':')
        && !name.chars().any(|c| (c as u32) < 0x20)
}

/// Natural (digit-aware, case-insensitive) ordering used for page lists in
/// both CBZ and image-folder modes: `2.png` before `10.png`.
pub fn natural_key(name: &str) -> Vec<(u8, String, u64)> {
    let mut key: Vec<(u8, String, u64)> = Vec::new();
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let number: u64 = str::from_utf8(&bytes[start..i])
                .unwrap_or("0")
                .parse()
                .unwrap_or(u64::MAX);
            key.push((1, String::new(), number));
        } else {
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_digit() {
                i += 1;
            }
            let text: String = String::from_utf8_lossy(&bytes[start..i])
                .chars()
                .flat_map(|c| c.to_lowercase())
                .collect();
            key.push((0, text, 0));
        }
    }
    key
}

pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (ka, kb) = (natural_key(a), natural_key(b));
    let paired = ka.iter().zip(kb.iter());
    for (left, right) in paired {
        let ordering = left.cmp(right);
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    ka.len().cmp(&kb.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort_orders_numbers_not_lexically() {
        let mut names = vec!["page10.png", "page2.png", "page1.png", "cover.png"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            vec!["cover.png", "page1.png", "page2.png", "page10.png"]
        );
    }

    #[test]
    fn entry_name_guard() {
        assert!(is_safe_entry_name("000001.png"));
        assert!(!is_safe_entry_name("../evil.png"));
        assert!(!is_safe_entry_name("a/b.png"));
        assert!(!is_safe_entry_name("C:\\x.png"));
        assert!(!is_safe_entry_name("sub\\name.png"));
    }

    #[test]
    fn image_extension_allowlist_is_case_insensitive() {
        assert!(is_image_name("PAGE.JPG"));
        assert!(is_image_name("001.webp"));
        assert!(!is_image_name("script.bat"));
        assert!(!is_image_name("ComicInfo.xml"));
    }
}
