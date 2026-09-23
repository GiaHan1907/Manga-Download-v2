//! Feature plan F3b — import an externally-obtained CBZ into the Library.
//!
//! Security-critical boundary: the renderer NEVER supplies or receives a path.
//! The native file dialog is opened from the Rust command and the chosen path
//! stays inside Rust. The imported archive is untrusted input — it is parsed
//! through the SAME hardened `cbz::open` (entry-name traversal rejection,
//! per-page size caps, deflate zip-bomb guards) that reads our own exports,
//! and only a successfully-parsed archive with image pages is accepted.
//!
//! The file is copied into the app-owned library root under a freshly
//! generated, fully-sanitized canonical name (never the source name), and the
//! chapter + a synthetic 'completed' task row are written so Library counts,
//! the offline reader and `reconcile` treat an import byte-identically to a
//! real download — no separate code path to drift.

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::domain::{new_id, now_iso, validate_id, validate_text_len, ImportedChapterDto};
use crate::downloader::storage;
use crate::error::AppError;
use crate::reader::cbz;

/// Hard ceiling on an importable archive (guards against copying a huge or
/// pathological file before the per-page caps even apply).
pub const MAX_IMPORT_BYTES: u64 = 512 * 1024 * 1024;

/// Library-relative path + absolute destination for a freshly generated
/// canonical file name under the app-owned library root.
fn canonical_dest(
    app_data_dir: &Path,
    manga_title: &str,
    manga_id: &str,
    file_name: &str,
) -> (String, PathBuf) {
    let dir = storage::manga_dir_name(manga_title, manga_id);
    let relpath = format!("{dir}/{file_name}");
    let abs = storage::library_root(app_data_dir)
        .join(dir)
        .join(file_name);
    (relpath, abs)
}

/// Copy + register one CBZ file (chosen by the caller) under a canonical name.
/// `source` is trusted only in that the native dialog produced it; every byte
/// and every archive claim is still re-validated here. Kept free of the
/// dialog so the whole path is unit-testable.
pub async fn import_cbz(
    pool: &SqlitePool,
    app_data_dir: &Path,
    source: &Path,
    manga_id: &str,
    number: Option<f64>,
    title: Option<&str>,
) -> Result<ImportedChapterDto, AppError> {
    validate_id(manga_id, "mangaId")?;
    if let Some(t) = title {
        validate_text_len(t, 200, "title")?;
    }

    let manga: Option<(String,)> = sqlx::query_as("SELECT title FROM mangas WHERE id = ?1")
        .bind(manga_id)
        .fetch_optional(pool)
        .await?;
    let (manga_title,) = manga.ok_or_else(|| AppError::not_found("manga"))?;

    // The source must be a real, non-empty, size-bounded regular file.
    let meta = std::fs::metadata(source).map_err(AppError::Io)?;
    if !meta.is_file() {
        return Err(AppError::validation("Nguồn nhập phải là một tệp."));
    }
    if meta.len() == 0 || meta.len() > MAX_IMPORT_BYTES {
        return Err(AppError::validation("Tệp vượt giới hạn dung lượng nhập."));
    }

    // Validate the archive AS UNTRUSTED before any copy — this rejects bad
    // traversal names, oversized/bomb pages and non-image payloads.
    let archive = cbz::open(source)?;
    let page_count = archive.entries.len();
    if page_count == 0 {
        return Err(AppError::validation("CBZ không có trang ảnh hợp lệ."));
    }

    let chapter_id = new_id();
    let task_id = new_id();
    let source_chapter_id = format!("import-{task_id}");
    let resolved_number = match number {
        Some(n) => n,
        None => {
            let (max,): (Option<f64>,) =
                sqlx::query_as("SELECT MAX(number) FROM chapters WHERE manga_id = ?1")
                    .bind(manga_id)
                    .fetch_one(pool)
                    .await?;
            max.map(|m| m + 1.0).unwrap_or(1.0)
        }
    };
    let chapter_title = title
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Chương {resolved_number}"));

    let base = storage::chapter_file_name(&chapter_title, &chapter_id, &task_id, "cbz");
    storage::assert_safe_name_component(&base)?;
    let (relpath, dest) = canonical_dest(app_data_dir, &manga_title, manga_id, &base);
    // Re-assert the destination stays under the library root before writing.
    storage::ensure_under_root(&storage::library_root(app_data_dir), &dest)?;

    // Copy to a `.part` sibling then rename — a crash never leaves a partial
    // file at the canonical path that reconcile would later call corrupt.
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = {
        let mut p = dest.clone().into_os_string();
        p.push(".importing");
        PathBuf::from(p)
    };
    let copied = std::fs::copy(source, &part).map_err(AppError::Io)?;
    if std::fs::rename(&part, &dest).is_err() {
        let _ = std::fs::remove_file(&part);
        return Err(AppError::validation(
            "Không ghi được tệp nhập vào thư viện.",
        ));
    }

    let ts = now_iso();
    let size_bytes = copied as i64;
    let tx = pool.begin_with("BEGIN IMMEDIATE").await;
    let mut tx = match tx {
        Ok(t) => t,
        Err(e) => {
            let _ = std::fs::remove_file(&dest);
            return Err(e.into());
        }
    };
    let insert = async {
        sqlx::query(
            "INSERT INTO chapters (id, manga_id, source_chapter_id, title, number, page_count, \
             created_at, updated_at, local_path, output_format, local_size_bytes) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, 'cbz', ?9)",
        )
        .bind(&chapter_id)
        .bind(manga_id)
        .bind(&source_chapter_id)
        .bind(&chapter_title)
        .bind(resolved_number)
        .bind(page_count as i64)
        .bind(&ts)
        .bind(&relpath)
        .bind(size_bytes)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, \
             completed_pages, total_pages, attempts, revision, finished_at, output_relpath, \
             created_at, updated_at) \
             VALUES (?1, ?2, 'completed', 'cbz', 0, ?3, ?3, 1, 1, ?4, ?5, ?4, ?4)",
        )
        .bind(&task_id)
        .bind(&chapter_id)
        .bind(page_count as i64)
        .bind(&ts)
        .bind(&relpath)
        .execute(&mut *tx)
        .await?;
        Ok::<(), AppError>(())
    }
    .await;
    if let Err(error) = insert {
        // Roll back the file too — a DB failure must not orphan an artifact
        // the Library has no row for (reconcile would count it as junk).
        tx.rollback().await.ok();
        let _ = std::fs::remove_file(&dest);
        return Err(error);
    }
    tx.commit().await?;

    Ok(ImportedChapterDto {
        manga_id: manga_id.to_string(),
        chapter_id,
        number: resolved_number,
        title: chapter_title,
        page_count: page_count as i64,
        size_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::downloader::storage;
    use crate::reader::testkit;
    use crate::services::library_service;

    /// Build a real exporter-produced STORED CBZ in a throwaway source root.
    fn make_source_cbz(count: usize) -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let staging = root.path().join("staging");
        let source_root = root.path().join("src");
        std::fs::create_dir_all(&source_root).unwrap();
        testkit::build_cbz(&staging, &source_root, "incoming.cbz", count);
        std::fs::remove_dir_all(&staging).ok();
        (root, source_root.join("incoming.cbz"))
    }

    struct Harness {
        data: tempfile::TempDir,
        pool: SqlitePool,
        manga_id: String,
    }

    async fn harness() -> Harness {
        let (_db, pool) = test_pool().await;
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(storage::library_root(data.path())).unwrap();
        let (manga_id, _) = testkit::seed_manga(&pool, "imp", 0).await;
        Harness {
            data,
            pool,
            manga_id,
        }
    }

    #[tokio::test]
    async fn f3b_import_registers_chapter_task_and_survives_reconcile() {
        let hx = harness().await;
        let (_src, source) = make_source_cbz(3);
        let dto = import_cbz(
            &hx.pool,
            hx.data.path(),
            &source,
            &hx.manga_id,
            Some(9.5),
            Some("Guest Chapter"),
        )
        .await
        .unwrap();
        assert_eq!(dto.page_count, 3);
        assert_eq!(dto.number, 9.5);

        // Chapter row now claims a local CBZ artifact.
        let chapter: (Option<String>, Option<String>) =
            sqlx::query_as("SELECT local_path, output_format FROM chapters WHERE id = ?1")
                .bind(&dto.chapter_id)
                .fetch_one(&hx.pool)
                .await
                .unwrap();
        let relpath = chapter.0.clone().expect("local_path set");
        assert_eq!(chapter.1.as_deref(), Some("cbz"));
        // The copied file exists at the canonical destination.
        assert!(storage::library_root(hx.data.path())
            .join(&relpath)
            .is_file());
        // Its name is fully sanitized + traceable, never the source name.
        assert!(relpath.ends_with(".cbz") && !relpath.contains("incoming"));

        // A synthetic 'completed' task backs it (Library/Downloads parity).
        let (task_status,): (String,) = sqlx::query_as(
            "SELECT status FROM download_tasks WHERE chapter_id = ?1 AND status = 'completed'",
        )
        .bind(&dto.chapter_id)
        .fetch_one(&hx.pool)
        .await
        .unwrap();
        assert_eq!(task_status, "completed");

        // Appears in the downloaded list and reconcile calls it healthy.
        let listed = library_service::downloaded_chapters(&hx.pool, hx.data.path(), &hx.manga_id)
            .await
            .unwrap();
        assert!(listed
            .iter()
            .any(|c| c["id"].as_str() == Some(dto.chapter_id.as_str())));
        let report = library_service::reconcile(&hx.pool, hx.data.path())
            .await
            .unwrap();
        assert_eq!(report.checked, 1);
        assert_eq!(report.healthy, 1);
        assert_eq!(report.missing, 0);
        assert_eq!(report.corrupted, 0);
    }

    #[tokio::test]
    async fn f3b_number_defaults_to_max_plus_one() {
        let hx = harness().await;
        let (_src, source) = make_source_cbz(1);
        let first = import_cbz(&hx.pool, hx.data.path(), &source, &hx.manga_id, None, None)
            .await
            .unwrap();
        assert_eq!(first.number, 1.0);
        let second = import_cbz(&hx.pool, hx.data.path(), &source, &hx.manga_id, None, None)
            .await
            .unwrap();
        assert_eq!(second.number, 2.0);
    }

    #[tokio::test]
    async fn f3b_rejects_non_archive_bad_manga_and_bad_id() {
        let (_db, pool) = test_pool().await;
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(storage::library_root(data.path())).unwrap();
        let (manga_id, _) = testkit::seed_manga(&pool, "bad", 0).await;

        // A non-ZIP file must fail through cbz::open, not be copied in.
        let junk_dir = tempfile::tempdir().unwrap();
        let junk = junk_dir.path().join("junk.cbz");
        std::fs::write(&junk, b"definitely not a zip archive").unwrap();
        assert!(import_cbz(&pool, data.path(), &junk, &manga_id, None, None)
            .await
            .is_err());

        // Unknown manga → not found; malformed id → validation error.
        assert!(
            import_cbz(&pool, data.path(), &junk, "does-not-exist", None, None)
                .await
                .is_err()
        );
        assert!(import_cbz(&pool, data.path(), &junk, "../evil", None, None)
            .await
            .is_err());
    }
}
