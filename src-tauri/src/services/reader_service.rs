//! Reader orchestration: chapter identity from SQLite, artifact bytes from the
//! app-owned library root. The renderer addresses pages by (chapterId,
//! pageIndex) only — storage paths exist solely inside Rust.

use std::path::{Path, PathBuf};

use sqlx::{Row, SqlitePool};

use crate::domain::{now_iso, ReaderManifestDto, ReaderPageDto};
use crate::downloader::storage;
use crate::error::{AppError, LibraryError};
use crate::reader::{cbz, image_folder, sniff};

struct ChapterRecord {
    chapter_id: String,
    chapter_title: String,
    manga_id: String,
    manga_title: String,
    local_path: String,
    output_format: String,
}

async fn load_downloaded_chapter(
    pool: &SqlitePool,
    chapter_id: &str,
) -> Result<ChapterRecord, AppError> {
    crate::domain::validate_id(chapter_id, "chapterId")?;
    let row = sqlx::query(
        "SELECT c.id AS chapter_id, c.title AS chapter_title, c.local_path, c.output_format, \
                m.id AS manga_id, m.title AS manga_title \
         FROM chapters c JOIN mangas m ON m.id = c.manga_id \
         WHERE c.id = ?1 AND c.local_path IS NOT NULL",
    )
    .bind(chapter_id)
    .fetch_optional(pool)
    .await?;
    let row = row.ok_or_else(|| AppError::from(LibraryError::NotFound))?;
    let local_path: String = row.try_get("local_path")?;
    storage::validate_relpath(&local_path)?;
    Ok(ChapterRecord {
        chapter_id: row.try_get("chapter_id")?,
        chapter_title: row.try_get("chapter_title")?,
        manga_id: row.try_get("manga_id")?,
        manga_title: row.try_get("manga_title")?,
        local_path,
        output_format: row
            .try_get::<Option<String>, _>("output_format")?
            .unwrap_or_else(|| "cbz".to_string()),
    })
}

fn resolve_artifact(app_data_dir: &Path, relpath: &str) -> Result<PathBuf, AppError> {
    let library_root = storage::library_root(app_data_dir);
    let candidate = library_root.join(relpath);
    storage::ensure_under_root(&library_root, &candidate)
}

/// Ordered DOWNLOADED chapters of one manga (same comparator as list_chapters).
async fn local_chapter_neighbours(
    pool: &SqlitePool,
    manga_id: &str,
    chapter_id: &str,
) -> Result<(Option<String>, Option<String>), AppError> {
    let rows = sqlx::query(
        "SELECT id FROM chapters WHERE manga_id = ?1 AND local_path IS NOT NULL \
         ORDER BY number IS NULL ASC, number ASC, created_at ASC, id ASC",
    )
    .bind(manga_id)
    .fetch_all(pool)
    .await?;
    let ids: Vec<String> = rows
        .iter()
        .map(|r| r.try_get("id").unwrap_or_default())
        .collect();
    let position = ids.iter().position(|id| id == chapter_id);
    match position {
        Some(index) => Ok((
            index.checked_sub(1).map(|i| ids[i].clone()),
            if index + 1 < ids.len() {
                Some(ids[index + 1].clone())
            } else {
                None
            },
        )),
        None => Ok((None, None)),
    }
}

/// Build the reader manifest: validates the artifact exists and parses,
/// persists the discovered page count and stamps last-read activity.
pub async fn open_chapter(
    pool: &SqlitePool,
    app_data_dir: &Path,
    chapter_id: &str,
) -> Result<ReaderManifestDto, AppError> {
    let record = load_downloaded_chapter(pool, chapter_id).await?;
    let artifact = resolve_artifact(app_data_dir, &record.local_path)?;

    let mut pages: Vec<ReaderPageDto> = Vec::new();
    // Sniff header dimensions only for a bounded prefix of pages — opening a
    // 300-page chapter must not read the whole archive (acceptance §P07-05);
    // the rest lay out via the UI aspect-ratio fallback.
    const SNIFF_PREFIX: usize = 12;
    if record.output_format == "folder" {
        if !artifact.is_dir() {
            return Err(LibraryError::FileMissing.into());
        }
        for (index, page) in image_folder::list_pages(&artifact)?.into_iter().enumerate() {
            if index < SNIFF_PREFIX {
                let bytes = image_folder::read_page(&artifact, &page.name)?;
                pages.push(page_dto_from_bytes(index, &bytes));
            } else {
                pages.push(ReaderPageDto {
                    index,
                    width: None,
                    height: None,
                });
            }
        }
    } else {
        if !artifact.is_file() {
            return Err(LibraryError::FileMissing.into());
        }
        let archive = cbz::open(&artifact)?;
        for (index, entry) in archive.entries.iter().enumerate() {
            if index < SNIFF_PREFIX {
                let bytes = cbz::read_entry(&artifact, entry)?;
                pages.push(page_dto_from_bytes(index, &bytes));
            } else {
                pages.push(ReaderPageDto {
                    index,
                    width: None,
                    height: None,
                });
            }
        }
    }
    if pages.is_empty() {
        return Err(LibraryError::FileCorrupted.into());
    }

    let page_count = pages.len() as i64;
    sqlx::query("UPDATE chapters SET page_count = ?2, updated_at = ?3 WHERE id = ?1")
        .bind(&record.chapter_id)
        .bind(page_count)
        .bind(now_iso())
        .execute(pool)
        .await?;
    let (prev_chapter_id, next_chapter_id) =
        local_chapter_neighbours(pool, &record.manga_id, &record.chapter_id).await?;

    // Resume position (reading_progress keeps one row per manga).
    let progress = sqlx::query(
        "SELECT page_index, scroll_offset FROM reading_progress \
         WHERE manga_id = ?1 AND chapter_id = ?2",
    )
    .bind(&record.manga_id)
    .bind(&record.chapter_id)
    .fetch_optional(pool)
    .await?;
    let (start_page, start_scroll_offset) = match progress {
        Some(row) => {
            let raw: i64 = row.try_get("page_index").unwrap_or(0);
            (
                raw.clamp(0, page_count - 1) as usize,
                row.try_get::<Option<f64>, _>("scroll_offset")
                    .ok()
                    .flatten(),
            )
        }
        None => (0, None),
    };

    Ok(ReaderManifestDto {
        chapter_id: record.chapter_id,
        manga_id: record.manga_id,
        manga_title: record.manga_title,
        chapter_title: record.chapter_title,
        format: record.output_format,
        page_count: pages.len(),
        pages,
        prev_chapter_id,
        next_chapter_id,
        start_page,
        start_scroll_offset,
    })
}

fn page_dto_from_bytes(index: usize, bytes: &[u8]) -> ReaderPageDto {
    let size = sniff::size_of(bytes);
    ReaderPageDto {
        index,
        width: size.map(|s| s.width),
        height: size.map(|s| s.height),
    }
}

/// Serve exactly one page's bytes (validated image). Never a whole chapter.
pub async fn page_bytes(
    pool: &SqlitePool,
    app_data_dir: &Path,
    chapter_id: &str,
    page_index: usize,
) -> Result<Vec<u8>, AppError> {
    let record = load_downloaded_chapter(pool, chapter_id).await?;
    let artifact = resolve_artifact(app_data_dir, &record.local_path)?;
    let bytes = if record.output_format == "folder" {
        let pages = image_folder::list_pages(&artifact)?;
        let page = pages.get(page_index).ok_or(LibraryError::PageNotFound)?;
        image_folder::read_page(&artifact, &page.name)?
    } else {
        let archive = cbz::open(&artifact)?;
        let entry = archive
            .entries
            .get(page_index)
            .ok_or(LibraryError::PageNotFound)?;
        cbz::read_entry(&artifact, entry)?
    };
    if sniff::size_of(&bytes).is_none() && !looks_like_supported_image(&bytes) {
        return Err(LibraryError::UnsupportedImage.into());
    }
    Ok(bytes)
}

fn looks_like_supported_image(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, b'P', b'N', b'G'])
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"GIF8")
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::progress_repo;
    use crate::db::test_pool;
    use crate::error::LibraryError;
    use crate::reader::testkit;

    struct Env {
        pool: SqlitePool,
        data: tempfile::TempDir,
        _db: tempfile::TempDir,
    }

    impl Env {
        fn library(&self) -> PathBuf {
            storage::library_root(self.data.path())
        }
        async fn open(&self, chapter: &str) -> Result<ReaderManifestDto, AppError> {
            open_chapter(&self.pool, self.data.path(), chapter).await
        }
    }

    async fn env_with_cbz() -> (Env, String, Vec<String>) {
        let (_db, pool) = test_pool().await;
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(storage::library_root(data.path())).unwrap();
        let (manga_id, chapters) = testkit::seed_manga(&pool, "m1", 3).await;
        let staging = data.path().join("stg");
        testkit::build_cbz(
            &staging,
            &storage::library_root(data.path()),
            "m1/c1.cbz",
            3,
        );
        testkit::build_cbz(
            &staging,
            &storage::library_root(data.path()),
            "m1/c2.cbz",
            2,
        );
        testkit::mark_downloaded(&pool, &chapters[0], "m1/c1.cbz", "cbz").await;
        testkit::mark_downloaded(&pool, &chapters[1], "m1/c2.cbz", "cbz").await;
        (Env { pool, data, _db }, manga_id, chapters)
    }

    #[tokio::test]
    async fn t07_cbz_manifest_order_sizes_neighbours_and_bytes() {
        let (env, _manga, chapters) = env_with_cbz().await;
        let manifest = env.open(&chapters[0]).await.unwrap();
        assert_eq!(manifest.page_count, 3);
        assert_eq!(manifest.format, "cbz");
        assert_eq!(manifest.prev_chapter_id, None);
        assert_eq!(
            manifest.next_chapter_id.as_deref(),
            Some(chapters[1].as_str())
        );
        // Sniffed PNG header dimensions (40x60).
        assert_eq!(manifest.pages[0].width, Some(40));
        assert_eq!(manifest.pages[0].height, Some(60));
        // page_count persisted
        let stored: (Option<i64>,) =
            sqlx::query_as("SELECT page_count FROM chapters WHERE id = ?1")
                .bind(&chapters[0])
                .fetch_one(&env.pool)
                .await
                .unwrap();
        assert_eq!(stored.0, Some(3));

        let bytes = page_bytes(&env.pool, env.data.path(), &chapters[0], 1)
            .await
            .unwrap();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
        let error = page_bytes(&env.pool, env.data.path(), &chapters[0], 99)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::Library(LibraryError::PageNotFound)
        ));

        // Undownloaded chapter → NotFound library error.
        let error = env.open(&chapters[2]).await.unwrap_err();
        assert!(matches!(error, AppError::Library(LibraryError::NotFound)));
        // ch2 is the last downloaded: prev points back.
        let second = env.open(&chapters[1]).await.unwrap();
        assert_eq!(
            second.prev_chapter_id.as_deref(),
            Some(chapters[0].as_str())
        );
        assert_eq!(second.next_chapter_id, None);
    }

    #[tokio::test]
    async fn t07_missing_corrupt_and_traversal_paths_are_controlled() {
        let (env, _manga, chapters) = env_with_cbz().await;
        // DB points at a file that vanished.
        testkit::mark_downloaded(&env.pool, &chapters[0], "m1/gone.cbz", "cbz").await;
        let error = env.open(&chapters[0]).await.unwrap_err();
        assert!(matches!(
            error,
            AppError::Library(LibraryError::FileMissing)
        ));

        // Garbage archive → CBZ_INVALID, not a panic and not a partial page.
        std::fs::write(env.library().join("m1/bad.cbz"), b"not a zip at all").unwrap();
        testkit::mark_downloaded(&env.pool, &chapters[0], "m1/bad.cbz", "cbz").await;
        let error = env.open(&chapters[0]).await.unwrap_err();
        assert!(matches!(error, AppError::Library(LibraryError::CbzInvalid)));

        // Traversal local_path must be rejected by validation before any read.
        testkit::mark_downloaded(&env.pool, &chapters[0], "../outside.cbz", "cbz").await;
        let error = env.open(&chapters[0]).await.unwrap_err();
        assert!(
            matches!(error, AppError::Validation(_)),
            "traversal relpath rejected: {error:?}"
        );
    }

    #[tokio::test]
    async fn t07_folder_mode_and_resume_position() {
        let (env, manga, chapters) = env_with_cbz().await;
        testkit::build_folder(&env.data.path().join("stg2"), &env.library(), "m1/c3", 2);
        testkit::mark_downloaded(&env.pool, &chapters[2], "m1/c3", "folder").await;
        let manifest = env.open(&chapters[2]).await.unwrap();
        assert_eq!(manifest.format, "folder");
        assert_eq!(manifest.page_count, 2);
        let bytes = page_bytes(&env.pool, env.data.path(), &chapters[2], 0)
            .await
            .unwrap();
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));

        // Progress resume only applies to the same chapter.
        progress_repo::save_progress(&env.pool, &manga, &chapters[2], 1, Some(0.5), Some(2))
            .await
            .unwrap();
        let manifest = env.open(&chapters[2]).await.unwrap();
        assert_eq!(manifest.start_page, 1);
        assert_eq!(manifest.start_scroll_offset, Some(0.5));
        let other = env.open(&chapters[0]).await.unwrap();
        assert_eq!(other.start_page, 0, "progress belongs to c3, not c1");
    }
}
