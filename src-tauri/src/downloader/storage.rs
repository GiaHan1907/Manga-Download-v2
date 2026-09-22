//! Approved-root storage layout and Windows-safe path construction.
//!
//! Every writable path used by the engine is derived from one of two roots
//! owned by the app (staging under app-cache, library under app-data) plus
//! generated/sanitized name components. No caller (renderer or provider)
//! supplies filesystem paths; helpers here canonicalize and re-assert the
//! root containment before anything is written.

use std::path::{Component, Path, PathBuf};

use crate::error::AppError;

/// Sub-structure under the app-data directory:
/// `cache/downloads/<task-id>/` staging and `library/` verified outputs.
pub fn staging_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("cache").join("downloads")
}

pub fn task_staging_dir(app_data_dir: &Path, task_id: &str) -> PathBuf {
    staging_root(app_data_dir).join(task_id)
}

pub fn library_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("library")
}

/// Creates the engine-owned directories (idempotent).
pub fn ensure_layout(app_data_dir: &Path) -> Result<(), AppError> {
    std::fs::create_dir_all(staging_root(app_data_dir))?;
    std::fs::create_dir_all(library_root(app_data_dir))?;
    Ok(())
}

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Sanitize one free-text label (manga/chapter title) for safe use as a file
/// or directory name component on Windows: ASCII-ish slug, no separators,
/// no reserved device names, no trailing dots/spaces, bounded length.
pub fn sanitize_label(raw: &str) -> String {
    let mut out: String = String::new();
    let mut last_dash = true; // suppress leading/duplicate separators
    for ch in raw.trim().chars() {
        let mapped = match ch {
            c if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
            // Separators and punctuation collapse into a single dash; other
            // non-ascii (Vietnamese diacritics) is dropped for a
            // normalization-immune ascii slug.
            ' ' | '-' | '_' | '.' | '/' | '\\' | ':' | ';' | '*' | '?' | '"' | '<' | '>' | '|' => {
                Some('-')
            }
            _ => None,
        };
        if let Some(replacement) = mapped {
            if replacement == '-' {
                if !last_dash {
                    out.push('-');
                    last_dash = true;
                }
            } else {
                out.push(replacement);
                last_dash = false;
            }
        }
        if out.len() >= 80 {
            break;
        }
    }
    let trimmed = out.trim_matches(|c| c == '-' || c == '.').to_string();
    let stem = trimmed.split('.').next().unwrap_or("").to_string();
    let reserved_hit = WINDOWS_RESERVED
        .iter()
        .any(|name| stem.eq_ignore_ascii_case(name));
    let candidate = if trimmed.is_empty() {
        "untitled".to_string()
    } else if reserved_hit {
        format!("_{trimmed}")
    } else {
        trimmed
    };
    candidate
}

/// Short stable token from a UUID-ish id for embedding in file names.
pub fn id_token(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
}

/// Library-relative directory for a manga: `<slug-title>-<manga-token>`.
pub fn manga_dir_name(manga_title: &str, manga_id: &str) -> String {
    let token = id_token(manga_id);
    let token = if token.is_empty() {
        "m".to_string()
    } else {
        token
    };
    format!("{}-{token}", sanitize_label(manga_title))
}

/// Library-relative file name for a chapter artifact. The task token makes
/// each artifact traceable to exactly one task (crash recovery + collision
/// policy); no user text ever reaches the name unsanitized.
pub fn chapter_file_name(
    chapter_title: &str,
    chapter_id: &str,
    task_id: &str,
    format: &str,
) -> String {
    let chapter_token = id_token(chapter_id);
    let task_token = id_token(task_id);
    let extension = if format == "folder" {
        String::new()
    } else {
        format!(".{format}")
    };
    format!(
        "ch-{}-{}-t{}{extension}",
        sanitize_label(chapter_title),
        if chapter_token.is_empty() {
            "c".to_string()
        } else {
            chapter_token
        },
        if task_token.is_empty() {
            "t".to_string()
        } else {
            task_token
        },
    )
}

/// Task ids are app-generated uuid-ish tokens; staging directory names must
/// match this shape before any cleanup touches them (defense against DB
/// pollution deleting arbitrary folders).
pub fn valid_task_id_shape(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A name component produced by this module must be a plain file name — no
/// separators, no traversal, no absolute paths, no control characters.
pub fn assert_safe_name_component(name: &str) -> Result<(), AppError> {
    if name.is_empty()
        || name.len() > 160
        || name == ".."
        || name == "."
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.chars().any(|c| (c as u32) < 0x20)
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
    {
        return Err(AppError::validation("Tên tệp không an toàn."));
    }
    Ok(())
}

/// Join `name` under `root` and verify the resolved path cannot escape the
/// root through symlinks/reparse points. `path` may not exist yet: then the
/// deepest existing ancestor is checked instead.
pub fn ensure_under_root(root: &Path, path: &Path) -> Result<PathBuf, AppError> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let resolved = if path.exists() {
        path.canonicalize()
            .map_err(|_| AppError::validation("Không phân giải được đường dẫn."))?
    } else {
        // Canonicalize the deepest existing ancestor, re-append the rest.
        let mut ancestor = path;
        let mut suffix: Vec<std::ffi::OsString> = Vec::new();
        loop {
            match ancestor.parent() {
                Some(_) if !ancestor.exists() => {
                    suffix.push(ancestor.file_name().unwrap_or_default().to_os_string());
                    ancestor = ancestor.parent().unwrap();
                }
                _ => break,
            }
        }
        let base = ancestor
            .canonicalize()
            .map_err(|_| AppError::validation("Không phân giải được đường dẫn."))?;
        let mut rebuilt = base;
        for part in suffix.iter().rev() {
            rebuilt.push(part);
        }
        rebuilt
    };
    if !resolved.starts_with(&canonical_root) {
        return Err(AppError::validation(
            "Đường dẫn nằm ngoài thư mục cho phép.",
        ));
    }
    Ok(resolved)
}

/// Reject any relative path coming from persistence that contains traversal
/// (defense-in-depth when reading `output_relpath` back from the DB).
pub fn validate_relpath(relpath: &str) -> Result<(), AppError> {
    let path = Path::new(relpath);
    if relpath.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(AppError::validation("Đường dẫn lưu trữ không hợp lệ."));
    }
    for component in path.components() {
        if let Component::Normal(name) = component {
            assert_safe_name_component(name.to_string_lossy().as_ref())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t06_19_sanitizer_neutralizes_windows_hazards() {
        assert_eq!(sanitize_label("../../etc/passwd"), "etc-passwd");
        assert_eq!(sanitize_label("CON"), "_con");
        // Dots never survive sanitization, so "CON.txt"-style device names
        // degrade to a safe "con-txt" (only exact stems need the guard).
        assert_eq!(sanitize_label("nul.txt"), "nul-txt");
        assert_eq!(sanitize_label("NUL"), "_nul");
        assert_eq!(
            sanitize_label("Chapter 12.5: Final " // trailing space/dot
            ),
            "chapter-12-5-final"
        );
        assert_eq!(sanitize_label("///"), "untitled");
        assert_eq!(sanitize_label("D:\\evil"), "d-evil");
        let long = sanitize_label(&"a".repeat(500));
        assert!(long.len() <= 80);
        // Vietnamese titles degrade to deterministic ascii slugs.
        assert_eq!(sanitize_label("Ninja Loạn Thị"), "ninja-lon-th");
    }

    #[test]
    fn name_components_are_enforced() {
        assert!(assert_safe_name_component("000001.png").is_ok());
        assert!(assert_safe_name_component("ch-foo-abc12345-tdef67890.cbz").is_ok());
        assert!(assert_safe_name_component("..").is_err());
        assert!(assert_safe_name_component("a/b").is_err());
        assert!(assert_safe_name_component("C:\\x").is_err());
        assert!(assert_safe_name_component("has space").is_err());
        assert!(assert_safe_name_component("đ_t.pdf").is_err());
    }

    #[test]
    fn t09_unicode_titles_produce_safe_path_components() {
        // Vietnamese + punctuation + reserved-word collisions must all come
        // out as plain, ASCII-safe, component-valid names.
        let slug = sanitize_label("Ninja Loạn Thị — Tập 1000: FINAL!?");
        assert!(assert_safe_name_component(&slug).is_ok(), "{slug}");
        let dir = manga_dir_name("Đảo Hải Tặc VOL.99", "abc-123");
        assert!(assert_safe_name_component(&dir).is_ok(), "{dir}");
        assert!(dir.is_ascii());
        // Reserved device names with unicode context still get neutralized.
        let reserved = sanitize_label("NUL tập 1");
        assert!(assert_safe_name_component(&reserved).is_ok());
        assert!(!reserved.is_empty());
    }

    #[test]
    fn relpath_rejects_traversal_and_absolute() {
        assert!(validate_relpath("manga-1/ch-2-t3.cbz").is_ok());
        assert!(validate_relpath("../escape.cbz").is_err());
        assert!(validate_relpath("C:/absolute.cbz").is_err());
        assert!(validate_relpath("a/../../b").is_err());
    }

    #[test]
    fn ensure_under_root_blocks_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("library");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();

        // A plain child passes.
        let inside_file = root.join("ok.cbz");
        std::fs::write(&inside_file, b"x").unwrap();
        assert!(ensure_under_root(&root, &inside_file).is_ok());

        // Path traversal via .. is caught.
        let escape = root.join("..").join("outside").join("bad.cbz");
        assert!(ensure_under_root(&root, &escape).is_err());

        // Non-existent files inside root still resolve to inside.
        let planned = root.join("manga-1").join("planned.cbz");
        assert!(ensure_under_root(&root, &planned).is_ok());
    }
}
