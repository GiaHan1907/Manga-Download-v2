//! Library domain operations: aggregated listing over the existing tables
//! (no second catalog), non-destructive reconciliation and path-safe removal.
//! "Downloaded" always means `chapters.local_path IS NOT NULL`; reconcile is
//! the component that re-proves the artifact against the filesystem.

use std::path::Path;

use sqlx::Row;
use sqlx::SqlitePool;

use crate::domain::{
    now_iso, LibraryGenreDto, LibraryMangaDto, LibraryProgressDto, LibraryStatsDto,
    ReconcileReportDto, RemoveMangaDto, RemoveMangaFilesDto,
};
use crate::downloader::storage;
use crate::error::{AppError, LibraryError};
use crate::reader::{cbz, image_folder};

pub const MAX_LIBRARY_PAGE: i64 = 100;
pub const RECONCILE_SCAN_LIMIT: i64 = 5000;
/// Unit separator — cannot appear in a genre name coming off a manga page.
/// The listing SQL spells the same byte as `char(31)`.
const GENRE_SEPARATOR: char = '\u{1f}';

pub async fn list_mangas(
    pool: &SqlitePool,
    query: Option<&str>,
    sort: Option<&str>,
    filter: Option<&str>,
    genre: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<LibraryMangaDto>, i64), AppError> {
    let limit = limit.clamp(1, MAX_LIBRARY_PAGE);
    crate::domain::validate_page(limit, offset)?;
    let sort = match sort.unwrap_or("lastRead") {
        "lastRead" => "m.last_read_at IS NULL, m.last_read_at DESC",
        "recentDownload" => "last_download_at IS NULL, last_download_at DESC",
        "chapters" => "downloaded_chapters DESC, m.title COLLATE NOCASE ASC",
        "title" => "m.title COLLATE NOCASE ASC",
        other => {
            return Err(AppError::validation(format!(
                "Kiểu sắp xếp không hợp lệ ({other})."
            )))
        }
    };
    let filter = match filter.unwrap_or("all") {
        "all" => "",
        "downloaded" => "AND downloaded_chapters > 0",
        "partial" => "AND downloaded_chapters > 0 AND downloaded_chapters < total_chapters",
        "favorite" => "AND EXISTS (SELECT 1 FROM favorites f WHERE f.manga_id = m.id)",
        // Feature plan F1: manga with unacknowledged new-chapter alerts.
        "alerts" => "AND EXISTS (SELECT 1 FROM chapter_alerts ca WHERE ca.manga_id = m.id)",
        // Library quick filters — each mirrors a published stat count exactly.
        "reading" => "AND EXISTS (SELECT 1 FROM reading_progress rp WHERE rp.manga_id = m.id)",
        "completed" => "AND m.status = 'completed'",
        "missing" => {
            "AND EXISTS (SELECT 1 FROM download_tasks dt JOIN chapters c ON c.id = dt.chapter_id \
                      WHERE c.manga_id = m.id AND dt.status = 'completed' \
                      AND dt.output_relpath IS NOT NULL AND c.local_path IS NULL)"
        }
        other => {
            return Err(AppError::validation(format!(
                "Bộ lọc không hợp lệ ({other})."
            )))
        }
    };
    let pattern = query.map(|value| {
        let escaped = value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        format!("%{}%", escaped.to_lowercase())
    });
    let search = if pattern.is_some() {
        "AND (lower(m.title) LIKE ? ESCAPE '\\' OR lower(IFNULL(m.author,'')) LIKE ? ESCAPE '\\')"
    } else {
        ""
    };
    // An empty string means "no genre chosen", so the placeholder (and its
    // bind) only exists when a genre is actually selected.
    let genre_sql = if genre.is_some() {
        "AND EXISTS (SELECT 1 FROM manga_genres mg WHERE mg.manga_id = m.id AND mg.genre = ?)"
    } else {
        ""
    };

    let base = "SELECT m.id, m.title, m.author, m.status, m.description, m.source_id, m.source_manga_id, m.cover_key, m.last_read_at, \
         IFNULL((SELECT group_concat(mg.genre, char(31)) FROM manga_genres mg WHERE mg.manga_id = m.id), '') AS genres, \
         (SELECT COUNT(*) FROM favorites f WHERE f.manga_id = m.id) AS favorite, \
         (SELECT COUNT(*) FROM chapters c WHERE c.manga_id = m.id AND c.local_path IS NOT NULL) AS downloaded_chapters, \
         (SELECT COUNT(*) FROM chapters c WHERE c.manga_id = m.id) AS total_chapters, \
         (SELECT SUM(c.local_size_bytes) FROM chapters c WHERE c.manga_id = m.id AND c.local_path IS NOT NULL) AS local_size_bytes, \
         (SELECT MAX(c.updated_at) FROM chapters c WHERE c.manga_id = m.id AND c.local_path IS NOT NULL) AS last_download_at, \
         (SELECT rp.chapter_id FROM reading_progress rp JOIN chapters rpc ON rpc.id = rp.chapter_id WHERE rp.manga_id = m.id AND rpc.local_path IS NOT NULL) AS progress_chapter_id, \
         (SELECT rp.page_index FROM reading_progress rp JOIN chapters rpc ON rpc.id = rp.chapter_id WHERE rp.manga_id = m.id AND rpc.local_path IS NOT NULL) AS progress_page_index, \
         (SELECT rp.page_total FROM reading_progress rp JOIN chapters rpc ON rpc.id = rp.chapter_id WHERE rp.manga_id = m.id AND rpc.local_path IS NOT NULL) AS progress_page_total, \
         (SELECT rpc.title FROM reading_progress rp JOIN chapters rpc ON rpc.id = rp.chapter_id WHERE rp.manga_id = m.id AND rpc.local_path IS NOT NULL) AS progress_chapter_title, \
         (SELECT rp.updated_at FROM reading_progress rp JOIN chapters rpc ON rpc.id = rp.chapter_id WHERE rp.manga_id = m.id AND rpc.local_path IS NOT NULL) AS progress_updated_at, \
         EXISTS (SELECT 1 FROM download_tasks dt JOIN chapters c ON c.id = dt.chapter_id \
                 WHERE c.manga_id = m.id AND dt.status = 'completed' \
                 AND dt.output_relpath IS NOT NULL AND c.local_path IS NULL) AS has_missing, \
         (SELECT COUNT(*) FROM chapter_alerts ca WHERE ca.manga_id = m.id) AS new_alerts \
         FROM mangas m";
    let where_sql = format!("WHERE 1=1 {search} {genre_sql} {filter}");
    let total_sql = format!("SELECT COUNT(*) FROM ({base} {where_sql})");
    let mut total_query = sqlx::query_scalar::<_, i64>(&total_sql);
    if let Some(value) = &pattern {
        // Two anonymous `?` occurrences in the search clause.
        total_query = total_query.bind(value).bind(value);
    }
    if let Some(value) = genre {
        total_query = total_query.bind(value);
    }
    let total = total_query.fetch_one(pool).await?;

    // Anonymous placeholders bind in textual order: search, genre, limit, offset.
    let rows_sql = format!("{base} {where_sql} ORDER BY {sort} LIMIT ? OFFSET ?");
    let mut query_builder = sqlx::query(&rows_sql);
    if let Some(value) = &pattern {
        query_builder = query_builder.bind(value).bind(value);
    }
    if let Some(value) = genre {
        query_builder = query_builder.bind(value);
    }
    let rows = query_builder
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let items = rows
        .iter()
        .map(|row| {
            let favorite: i64 = row.try_get("favorite")?;
            let genres: String = row.try_get("genres")?;
            let progress = match row.try_get::<Option<String>, _>("progress_chapter_id")? {
                Some(chapter_id) => Some(LibraryProgressDto {
                    chapter_id,
                    chapter_title: row.try_get("progress_chapter_title")?,
                    page_index: row
                        .try_get::<Option<i64>, _>("progress_page_index")?
                        .unwrap_or(0),
                    page_total: row.try_get("progress_page_total")?,
                    updated_at: row
                        .try_get::<Option<String>, _>("progress_updated_at")?
                        .unwrap_or_default(),
                }),
                None => None,
            };
            Ok(LibraryMangaDto {
                id: row.try_get("id")?,
                title: row.try_get("title")?,
                description: row.try_get("description")?,
                genres: genres
                    .split(GENRE_SEPARATOR)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect(),
                author: row.try_get("author")?,
                status: row.try_get("status")?,
                source_id: row.try_get("source_id")?,
                source_manga_id: row.try_get("source_manga_id")?,
                cover_key: row.try_get("cover_key")?,
                favorite: favorite != 0,
                downloaded_chapters: row.try_get("downloaded_chapters")?,
                total_chapters: row.try_get("total_chapters")?,
                local_size_bytes: row.try_get("local_size_bytes")?,
                last_read_at: row.try_get("last_read_at")?,
                last_download_at: row.try_get("last_download_at")?,
                progress,
                has_missing: row.try_get::<bool, _>("has_missing")?,
                new_alerts: row.try_get::<i64, _>("new_alerts")?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok((items, total))
}

/// Genres present in the local library, with how many saved stories carry
/// each. Counts are our own rows, not the source's, so they can be shown.
pub async fn genres(pool: &SqlitePool) -> Result<Vec<LibraryGenreDto>, AppError> {
    let rows = sqlx::query(
        "SELECT mg.genre AS genre, COUNT(DISTINCT mg.manga_id) AS manga_count \
         FROM manga_genres mg JOIN mangas m ON m.id = mg.manga_id \
         GROUP BY mg.genre ORDER BY manga_count DESC, mg.genre COLLATE NOCASE ASC",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(LibraryGenreDto {
                genre: row.try_get("genre")?,
                manga_count: row.try_get("manga_count")?,
            })
        })
        .collect()
}

/// Phase 08 Home overview — one aggregate pass, all from persisted truth.
pub async fn stats(pool: &SqlitePool) -> Result<LibraryStatsDto, AppError> {
    let row = sqlx::query(
        "SELECT (SELECT COUNT(*) FROM mangas) AS saved_mangas, \
                (SELECT COUNT(*) FROM chapters) AS total_chapters, \
                (SELECT COUNT(*) FROM chapters WHERE local_path IS NOT NULL) AS downloaded_chapters, \
                (SELECT IFNULL(SUM(local_size_bytes), 0) FROM chapters WHERE local_path IS NOT NULL) AS local_size_bytes, \
                (SELECT COUNT(*) FROM favorites) AS favorites, \
                (SELECT COUNT(DISTINCT c.manga_id) FROM chapters c WHERE c.local_path IS NOT NULL) AS downloaded_mangas, \
                (SELECT COUNT(*) FROM mangas m WHERE EXISTS (SELECT 1 FROM reading_progress rp WHERE rp.manga_id = m.id)) AS reading, \
                (SELECT COUNT(*) FROM mangas WHERE status = 'completed') AS completed, \
                (SELECT COUNT(*) FROM download_tasks WHERE status = 'paused') AS paused, \
                (SELECT COUNT(DISTINCT c.manga_id) FROM download_tasks dt JOIN chapters c ON c.id = dt.chapter_id \
                  WHERE dt.status = 'completed' AND dt.output_relpath IS NOT NULL AND c.local_path IS NULL) AS missing",
    )
    .fetch_one(pool)
    .await?;
    Ok(LibraryStatsDto {
        saved_mangas: row.try_get("saved_mangas")?,
        total_chapters: row.try_get("total_chapters")?,
        downloaded_chapters: row.try_get("downloaded_chapters")?,
        local_size_bytes: row.try_get("local_size_bytes")?,
        favorites: row.try_get("favorites")?,
        downloaded_mangas: row.try_get("downloaded_mangas")?,
        reading: row.try_get("reading")?,
        completed: row.try_get("completed")?,
        paused: row.try_get("paused")?,
        missing: row.try_get("missing")?,
    })
}

/// A chapter row with a presence-checked artifact flag (bounded fs per call).
pub async fn downloaded_chapters(
    pool: &SqlitePool,
    app_data_dir: &Path,
    manga_id: &str,
) -> Result<Vec<serde_json::Value>, AppError> {
    crate::domain::validate_id(manga_id, "mangaId")?;
    let rows = sqlx::query(
        "SELECT id, title, number, page_count, local_path, output_format, local_size_bytes, updated_at \
         FROM chapters WHERE manga_id = ?1 \
         ORDER BY number IS NULL ASC, number ASC, created_at ASC, id ASC",
    )
    .bind(manga_id)
    .fetch_all(pool)
    .await?;
    let mut items = Vec::new();
    for row in rows {
        let local_path: Option<String> = row.try_get("local_path")?;
        let Some(path) = local_path else { continue };
        // Same relpath discipline as every other DB-path consumer: a tampered
        // row must not even probe arbitrary paths through artifactOk.
        let artifact_ok = storage::validate_relpath(&path).is_ok()
            && storage::library_root(app_data_dir).join(&path).exists();
        items.push(serde_json::json!({
            "id": row.try_get::<String, _>("id")?,
            "title": row.try_get::<String, _>("title")?,
            "number": row.try_get::<Option<f64>, _>("number")?,
            "pageCount": row.try_get::<Option<i64>, _>("page_count")?,
            "storageFormat": row.try_get::<Option<String>, _>("output_format")?,
            "sizeBytes": row.try_get::<Option<i64>, _>("local_size_bytes")?,
            "artifactOk": artifact_ok,
            "updatedAt": row.try_get::<String, _>("updated_at")?,
        }));
    }
    Ok(items)
}

/// Prove recorded artifacts against the filesystem. Non-destructive: missing
/// or unreadable outputs clear the offline flag only — files are never deleted
/// and history (tasks/favorites/progress) is untouched.
pub async fn reconcile(
    pool: &SqlitePool,
    app_data_dir: &Path,
) -> Result<ReconcileReportDto, AppError> {
    let rows = sqlx::query(
        "SELECT id, local_path, output_format FROM chapters \
         WHERE local_path IS NOT NULL ORDER BY id LIMIT ?1",
    )
    .bind(RECONCILE_SCAN_LIMIT)
    .fetch_all(pool)
    .await?;
    let mut report = ReconcileReportDto {
        checked: rows.len() as i64,
        healthy: 0,
        missing: 0,
        corrupted: 0,
    };
    for row in rows {
        let chapter_id: String = row.try_get("id")?;
        let path: String = row.try_get("local_path")?;
        let format: String = row
            .try_get::<Option<String>, _>("output_format")?
            .unwrap_or_else(|| "cbz".to_string());
        let outcome = storage::validate_relpath(&path)
            .map(|()| {
                let artifact = storage::library_root(app_data_dir).join(&path);
                if !artifact.exists() {
                    Err(false)
                } else {
                    match prove_artifact(&artifact, &format) {
                        Prove::Ok(size) => Ok(size),
                        Prove::Corrupt => Err(true),
                    }
                }
            })
            // An unparseable stored path counts as corrupted, not missing.
            .unwrap_or(Err(true));
        match outcome {
            Ok(size) => {
                report.healthy += 1;
                if let Some(size) = size {
                    sqlx::query("UPDATE chapters SET local_size_bytes = ?2 WHERE id = ?1")
                        .bind(&chapter_id)
                        .bind(size)
                        .execute(pool)
                        .await?;
                }
            }
            Err(corrupted) => {
                if corrupted {
                    report.corrupted += 1;
                } else {
                    report.missing += 1;
                }
                sqlx::query(
                    "UPDATE chapters SET local_path = NULL, output_format = NULL, \
                     local_size_bytes = NULL, updated_at = ?2 WHERE id = ?1",
                )
                .bind(&chapter_id)
                .bind(now_iso())
                .execute(pool)
                .await?;
            }
        }
    }
    Ok(report)
}

enum Prove {
    /// None = size unknown (folder mode sums below).
    Ok(Option<i64>),
    Corrupt,
}

fn prove_artifact(artifact: &Path, format: &str) -> Prove {
    if format == "folder" {
        match image_folder::list_pages(artifact) {
            Ok(pages) => {
                let total: i64 = pages.iter().map(|p| p.size_bytes as i64).sum();
                Prove::Ok(Some(total))
            }
            Err(_) => Prove::Corrupt,
        }
    } else {
        match cbz::open(artifact) {
            Ok(archive) => Prove::Ok(
                artifact
                    .metadata()
                    .ok()
                    .map(|m| m.len() as i64)
                    .or(Some(archive.entries.iter().map(|e| e.size as i64).sum())),
            ),
            Err(_) => Prove::Corrupt,
        }
    }
}

/// Remove one chapter's local artifact + offline flags, disk-first so a failed
/// delete can never falsely mark content as removed.
pub async fn remove_chapter_files(
    pool: &SqlitePool,
    app_data_dir: &Path,
    chapter_id: &str,
) -> Result<(), AppError> {
    crate::domain::validate_id(chapter_id, "chapterId")?;
    let row = sqlx::query(
        "SELECT local_path, output_format FROM chapters WHERE id = ?1 AND local_path IS NOT NULL",
    )
    .bind(chapter_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::from(LibraryError::NotFound))?;
    let path: String = row.try_get("local_path")?;
    let format: Option<String> = row.try_get("output_format")?;
    storage::validate_relpath(&path)?;
    let library_root = storage::library_root(app_data_dir);
    let artifact = storage::ensure_under_root(&library_root, &library_root.join(&path))?;

    let delete_result = if format.as_deref() == Some("folder") || artifact.is_dir() {
        std::fs::remove_dir_all(&artifact)
    } else {
        std::fs::remove_file(&artifact)
    };
    if let Err(error) = delete_result {
        // Filesystem still authoritative: keep DB flags intact. Log only the
        // error KIND — io::Error Display can embed the absolute path.
        eprintln!(
            "chapter artifact delete failed for {chapter_id}: {:?}",
            error.kind()
        );
        return Err(LibraryError::DeleteFailed.into());
    }
    sqlx::query(
        "UPDATE chapters SET local_path = NULL, output_format = NULL, local_size_bytes = NULL, \
         updated_at = ?2 WHERE id = ?1",
    )
    .bind(chapter_id)
    .bind(now_iso())
    .execute(pool)
    .await?;
    // An INTENTIONAL delete must not keep the manga flagged "missing": the
    // completed-task proof of an artifact points at a path that is now gone
    // by user request. (Externally deleted files keep the flag — startup
    // recovery only clears local_path, never the task's output_relpath.)
    sqlx::query("UPDATE download_tasks SET output_relpath = NULL WHERE chapter_id = ?1 AND status = 'completed'")
        .bind(chapter_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Remove every local artifact of a manga (metadata rows kept: favorites,
/// progress and catalog remain — this is "delete downloads", not "erase").
pub async fn remove_manga_files(
    pool: &SqlitePool,
    app_data_dir: &Path,
    manga_id: &str,
) -> Result<RemoveMangaFilesDto, AppError> {
    crate::domain::validate_id(manga_id, "mangaId")?;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM chapters WHERE manga_id = ?1 AND local_path IS NOT NULL ORDER BY id",
    )
    .bind(manga_id)
    .fetch_all(pool)
    .await?;
    let mut removed = Vec::new();
    let mut failed = Vec::new();
    for (chapter_id,) in rows {
        match remove_chapter_files(pool, app_data_dir, &chapter_id).await {
            Ok(()) => removed.push(chapter_id),
            Err(_) => failed.push(chapter_id),
        }
    }
    Ok(RemoveMangaFilesDto { removed, failed })
}

/// Erase a manga from the Library entirely. `delete_files` also removes the
/// downloaded artifacts; in keep-files mode nothing on disk is touched.
/// Everything that still references the manga or its chapters goes in ONE
/// transaction: the RESTRICT children first (progress, favorites, tasks —
/// checkpoint pages cascade with them), then chapters (alerts, history
/// cascade), then the manga row (genres cascade). A queued/in-flight/failed
/// download blocks the erase: the dispatcher must never lose the row a claim
/// points at, and a "failed" row is still a retry the user owns.
pub async fn remove_manga(
    pool: &SqlitePool,
    app_data_dir: &Path,
    manga_id: &str,
    delete_files: bool,
) -> Result<RemoveMangaDto, AppError> {
    crate::domain::validate_id(manga_id, "mangaId")?;
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM mangas WHERE id = ?1")
        .bind(manga_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::not_found("truyện trong thư viện"));
    }
    let unsettled: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM download_tasks t JOIN chapters c ON c.id = t.chapter_id \
         WHERE c.manga_id = ?1 AND t.status NOT IN ('completed', 'cancelled')",
    )
    .bind(manga_id)
    .fetch_one(pool)
    .await?;
    if unsettled > 0 {
        return Err(AppError::validation(format!(
            "Còn {unsettled} tác vụ tải chưa kết thúc của truyện — hãy hủy hoặc tải xong rồi xóa."
        )));
    }
    let chapters_removed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM chapters WHERE manga_id = ?1")
            .bind(manga_id)
            .fetch_one(pool)
            .await?;

    let mut files_removed = 0i64;
    // The catalog is what the user asked to erase; a file that refuses to go
    // (locked by another process) must not block the whole operation —
    // mirror remove_manga_files' per-item tolerance and keep counting.
    let mut failed_files = 0i64;
    if delete_files {
        let rows = sqlx::query_as::<_, (String, Option<String>, String)>(
            "SELECT local_path, output_format, id FROM chapters \
             WHERE manga_id = ?1 AND local_path IS NOT NULL ORDER BY id",
        )
        .bind(manga_id)
        .fetch_all(pool)
        .await?;
        let library_root = storage::library_root(app_data_dir);
        for (path, format, chapter_id) in rows {
            if storage::validate_relpath(&path).is_err() {
                continue; // unparseable stored path: nothing safe to remove
            }
            let Ok(artifact) = storage::ensure_under_root(&library_root, &library_root.join(&path))
            else {
                eprintln!("manga erase: path escapes library root for chapter {chapter_id}");
                failed_files += 1;
                continue;
            };
            let is_folder = format.as_deref() == Some("folder") || artifact.is_dir();
            let outcome = if is_folder {
                std::fs::remove_dir_all(&artifact)
            } else {
                std::fs::remove_file(&artifact)
            };
            match outcome {
                Ok(()) => files_removed += 1,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    eprintln!(
                        "manga erase: artifact delete failed for chapter {chapter_id}: {:?}",
                        error.kind()
                    );
                    failed_files += 1;
                }
            }
        }
    }

    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    for sql in [
        "DELETE FROM reading_progress WHERE manga_id = ?1",
        "DELETE FROM favorites WHERE manga_id = ?1",
        "DELETE FROM download_tasks WHERE chapter_id IN (SELECT id FROM chapters WHERE manga_id = ?1)",
        "DELETE FROM chapters WHERE manga_id = ?1",
        "DELETE FROM mangas WHERE id = ?1",
    ] {
        sqlx::query(sql).bind(manga_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    if delete_files && failed_files > 0 {
        eprintln!("manga erase: {failed_files} file(s) could not be deleted for manga {manga_id}");
    }
    Ok(RemoveMangaDto {
        chapters_removed,
        files_removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::error::LibraryError;
    use crate::reader::testkit;

    struct Lib {
        pool: SqlitePool,
        data: tempfile::TempDir,
        _db: tempfile::TempDir,
    }

    impl Lib {
        fn library(&self) -> std::path::PathBuf {
            storage::library_root(self.data.path())
        }
    }

    async fn completed_task(pool: &SqlitePool, chapter_id: &str) {
        let ts = now_iso();
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority,              completed_pages, total_pages, attempts, revision, created_at, updated_at)              VALUES (?1, ?2, 'completed', 'cbz', 0, 3, 3, 0, 1, ?3, ?3)",
        )
        .bind(format!("tk-{chapter_id}"))
        .bind(chapter_id)
        .bind(&ts)
        .execute(pool)
        .await
        .unwrap();
    }

    /// manga A: 4 chapters, c1 cbz present, c2 recorded but file absent
    /// (+completed task), favorite, has progress. manga B: no downloads.
    async fn seeded() -> (Lib, String, Vec<String>) {
        let (_db, pool) = test_pool().await;
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(storage::library_root(data.path())).unwrap();
        let (manga_a, chapters_a) = testkit::seed_manga(&pool, "ma", 4).await;
        testkit::seed_manga(&pool, "mb", 2).await;
        testkit::build_cbz(
            &data.path().join("stg"),
            &storage::library_root(data.path()),
            "ma/c1.cbz",
            3,
        );
        testkit::mark_downloaded(&pool, &chapters_a[0], "ma/c1.cbz", "cbz").await;
        testkit::mark_downloaded(&pool, &chapters_a[1], "ma/c2.cbz", "cbz").await;
        completed_task(&pool, &chapters_a[1]).await;
        let ts = now_iso();
        sqlx::query("INSERT INTO favorites (manga_id, created_at) VALUES (?1, ?2)")
            .bind(&manga_a)
            .bind(&ts)
            .execute(&pool)
            .await
            .unwrap();
        crate::db::repositories::progress_repo::save_progress(
            &pool,
            &manga_a,
            &chapters_a[0],
            2,
            None,
            Some(3),
        )
        .await
        .unwrap();
        (Lib { pool, data, _db }, manga_a, chapters_a)
    }

    async fn list_all(lib: &Lib, filter: &str) -> Vec<LibraryMangaDto> {
        list_mangas(&lib.pool, None, Some("title"), Some(filter), None, 50, 0)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn t11_genre_filter_and_counts_come_from_the_genre_table() {
        let (lib, manga_a, _) = seeded().await;
        // `seeded()` returns generated pks, so the second story is seeded here
        // to get an id to tag.
        let (manga_b, _) = testkit::seed_manga(&lib.pool, "mc", 1).await;
        for (id, genre) in [
            (manga_a.as_str(), "Hành động"),
            (manga_a.as_str(), "Viễn tưởng"),
            (manga_b.as_str(), "Hành động"),
        ] {
            sqlx::query("INSERT INTO manga_genres (manga_id, genre) VALUES (?1, ?2)")
                .bind(id)
                .bind(genre)
                .execute(&lib.pool)
                .await
                .unwrap();
        }
        let action = list_mangas(
            &lib.pool,
            None,
            Some("title"),
            Some("all"),
            Some("Hành động"),
            50,
            0,
        )
        .await
        .unwrap();
        assert_eq!(action.0.len(), 2, "both tagged stories come back");
        let first = action
            .0
            .iter()
            .find(|row| row.id == manga_a)
            .expect("manga_a carries the genre");
        assert_eq!(
            first.genres,
            vec!["Hành động".to_string(), "Viễn tưởng".to_string()],
            "a story keeps all of its genres, in the source's order"
        );

        let scifi = list_mangas(
            &lib.pool,
            None,
            Some("title"),
            Some("all"),
            Some("Viễn tưởng"),
            50,
            0,
        )
        .await
        .unwrap();
        assert_eq!(scifi.0.len(), 1);
        assert_eq!(scifi.0[0].id, manga_a);

        let tags = genres(&lib.pool).await.unwrap();
        assert_eq!(tags[0].genre, "Hành động");
        assert_eq!(tags[0].manga_count, 2, "counted from our own rows");
        assert_eq!(tags[1].genre, "Viễn tưởng");
        assert_eq!(tags[1].manga_count, 1);
    }

    #[tokio::test]
    async fn t11_quick_filters_match_the_counts_they_display() {
        let (lib, manga_a, _) = seeded().await;
        sqlx::query("INSERT INTO manga_genres (manga_id, genre) VALUES (?1, ?2)")
            .bind(&manga_a)
            .bind("Hành động")
            .execute(&lib.pool)
            .await
            .unwrap();
        let stats = stats(&lib.pool).await.unwrap();
        // Every stat must be reproducible by the filter of the same name,
        // otherwise a quick-filter tile would show a number it cannot reach.
        for (filter, count) in [
            ("downloaded", stats.downloaded_mangas),
            ("reading", stats.reading),
            ("favorite", stats.favorites),
            ("completed", stats.completed),
            ("missing", stats.missing),
        ] {
            let (_, total) =
                list_mangas(&lib.pool, None, Some("title"), Some(filter), None, 100, 0)
                    .await
                    .unwrap();
            assert_eq!(total, count, "filter {filter} disagrees with its stat");
        }
        assert_eq!(stats.saved_mangas, 2);
        assert!(stats.downloaded_mangas >= 1);
        assert_eq!(stats.reading, 1);
    }

    #[tokio::test]
    async fn t07_list_aggregates_counts_favorite_progress_and_filters() {
        let (lib, manga_a, _) = seeded().await;
        let all = list_all(&lib, "all").await;
        assert_eq!(all.len(), 2);
        let a = all.iter().find(|m| m.id == manga_a).expect("manga A");
        assert_eq!(a.downloaded_chapters, 2);
        assert_eq!(a.total_chapters, 4);
        assert!(a.favorite);
        let progress = a.progress.clone().expect("reading progress surfaced");
        assert_eq!(progress.page_index, 2);
        assert_eq!(progress.page_total, Some(3));
        assert!(a.last_read_at.is_some());
        assert_eq!(list_all(&lib, "downloaded").await.len(), 1);
        assert_eq!(list_all(&lib, "favorite").await.len(), 1);
        assert_eq!(list_all(&lib, "partial").await.len(), 1);
        let (none, _) = list_mangas(&lib.pool, Some("%"), None, None, None, 50, 0)
            .await
            .unwrap();
        assert!(
            none.is_empty(),
            "LIKE wildcard must be escaped, not match all"
        );
    }

    #[tokio::test]
    async fn t07_reconcile_reports_missing_without_deleting_and_fixes_size() {
        let (lib, _, _) = seeded().await;
        let report = reconcile(&lib.pool, lib.data.path()).await.unwrap();
        assert_eq!(report.checked, 2);
        assert_eq!(report.healthy, 1, "c1 present and parses");
        assert_eq!(report.missing, 1, "c2 recorded but file absent");
        assert_eq!(report.corrupted, 0);
        let c1: (Option<i64>,) =
            sqlx::query_as("SELECT local_size_bytes FROM chapters WHERE id = ?1")
                .bind({
                    // c1 keeps its local_path after reconcile (healthy)
                    let (id,): (String,) =
                        sqlx::query_as("SELECT id FROM chapters WHERE local_path = 'ma/c1.cbz'")
                            .fetch_one(&lib.pool)
                            .await
                            .unwrap();
                    id
                })
                .fetch_one(&lib.pool)
                .await
                .unwrap();
        assert!(c1.0.unwrap() > 0, "healthy chapter got size back-filled");
        let still_flagged: Option<(String,)> =
            sqlx::query_as("SELECT id FROM chapters WHERE local_path = 'ma/c2.cbz'")
                .fetch_optional(&lib.pool)
                .await
                .unwrap();
        assert!(still_flagged.is_none(), "missing artifact flag cleared");
        assert!(lib.library().join("ma/c1.cbz").exists(), "non-destructive");
    }

    #[tokio::test]
    async fn t07_remove_chapter_is_disk_first_and_path_safe() {
        let (lib, _, chapters) = seeded().await;
        remove_chapter_files(&lib.pool, lib.data.path(), &chapters[0])
            .await
            .unwrap();
        assert!(!lib.library().join("ma/c1.cbz").exists(), "file removed");
        let left: (Option<String>,) =
            sqlx::query_as("SELECT local_path FROM chapters WHERE id = ?1")
                .bind(&chapters[0])
                .fetch_one(&lib.pool)
                .await
                .unwrap();
        assert!(left.0.is_none(), "DB flag cleared");
        assert!(matches!(
            remove_chapter_files(&lib.pool, lib.data.path(), &chapters[0]).await,
            Err(AppError::Library(LibraryError::NotFound))
        ));
    }

    #[tokio::test]
    async fn t07_remove_rejects_traversal_without_touching_disk() {
        let (lib, _, _) = seeded().await;
        let (manga_id, ids) = testkit::seed_manga(&lib.pool, "evil", 1).await;
        let _ = manga_id;
        testkit::mark_downloaded(&lib.pool, &ids[0], "../outside.cbz", "cbz").await;
        std::fs::write(lib.data.path().join("outside.cbz"), b"victim").unwrap();
        let error = remove_chapter_files(&lib.pool, lib.data.path(), &ids[0])
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
        assert!(
            lib.data.path().join("outside.cbz").exists(),
            "rejected delete must not touch the file"
        );
    }

    #[tokio::test]
    async fn t10_downloaded_chapters_never_probes_unvalidated_paths() {
        let (lib, _, _) = seeded().await;
        let (manga_id, ids) = testkit::seed_manga(&lib.pool, "probe", 1).await;
        // Tampered DB row with a traversal local_path: listing must succeed
        // and report artifactOk=false WITHOUT evaluating the raw path.
        testkit::mark_downloaded(&lib.pool, &ids[0], "../outside.cbz", "cbz").await;
        std::fs::write(lib.data.path().join("outside.cbz"), b"exists-bait").unwrap();
        let items = downloaded_chapters(&lib.pool, lib.data.path(), &manga_id)
            .await
            .unwrap();
        let row = items
            .iter()
            .find(|item| item["id"] == serde_json::json!(ids[0]))
            .expect("tampered chapter listed");
        assert_eq!(row["artifactOk"], serde_json::json!(false));
    }

    #[tokio::test]
    async fn t12_alerts_filter_and_new_alerts_count() {
        let (lib, manga_a, chapters) = seeded().await;
        let ts = now_iso();
        sqlx::query(
            "INSERT INTO chapter_alerts (manga_id, chapter_id, detected_at) VALUES (?1, ?2, ?3)",
        )
        .bind(&manga_a)
        .bind(&chapters[0])
        .bind(&ts)
        .execute(&lib.pool)
        .await
        .unwrap();
        let with_alerts = list_all(&lib, "alerts").await;
        assert_eq!(with_alerts.len(), 1);
        assert_eq!(with_alerts[0].new_alerts, 1);
        let all = list_all(&lib, "all").await;
        let a = all.iter().find(|m| m.id == manga_a).unwrap();
        assert_eq!(a.new_alerts, 1);
        // Ack empties the filter again (alerts are deleted, not flagged).
        super::super::watch_service::ack_alerts(&lib.pool, &manga_a)
            .await
            .unwrap();
        assert!(list_all(&lib, "alerts").await.is_empty());
    }

    #[tokio::test]
    async fn t11_intentional_delete_clears_missing_flag_external_loss_keeps_it() {
        // Regression (whole-project review round 2): remove_chapter_files
        // left the completed task untouched, so the manga was flagged
        // "missing chapters" forever after an INTENTIONAL delete. External
        // loss must keep the flag — that is the anomaly it advertises.
        let (lib, _, _) = seeded().await;
        let (manga_id, ids) = testkit::seed_manga(&lib.pool, "missing", 1).await;
        testkit::build_cbz(
            &lib.data.path().join("stg2"),
            &storage::library_root(lib.data.path()),
            "missing/cb1.cbz",
            3,
        );
        testkit::mark_downloaded(&lib.pool, &ids[0], "missing/cb1.cbz", "cbz").await;
        sqlx::query("UPDATE chapters SET page_count = 3 WHERE id = ?1")
            .bind(&ids[0])
            .execute(&lib.pool)
            .await
            .unwrap();
        let ts = now_iso();
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, \
             completed_pages, total_pages, attempts, revision, output_relpath, created_at, updated_at) \
             VALUES ('tk-missing', ?1, 'completed', 'cbz', 0, 3, 3, 0, 1, 'missing/cb1.cbz', ?2, ?2)",
        )
        .bind(&ids[0])
        .bind(&ts)
        .execute(&lib.pool)
        .await
        .unwrap();
        let list_one = |l: &Lib| {
            let pool = l.pool.clone();
            let manga = manga_id.clone();
            async move {
                list_mangas(&pool, None, Some("title"), Some("all"), None, 50, 0)
                    .await
                    .unwrap()
                    .0
                    .into_iter()
                    .find(|m| m.id == manga)
                    .unwrap()
                    .has_missing
            }
        };
        assert!(!list_one(&lib).await, "downloaded and present: not missing");

        remove_chapter_files(&lib.pool, lib.data.path(), &ids[0])
            .await
            .unwrap();
        assert!(
            !list_one(&lib).await,
            "intentional delete must not flag missing"
        );
        // Catalog metadata (page_count) survives the delete of files only.
        let pages: (Option<i64>,) = sqlx::query_as("SELECT page_count FROM chapters WHERE id = ?1")
            .bind(&ids[0])
            .fetch_one(&lib.pool)
            .await
            .unwrap();
        assert_eq!(
            pages.0,
            Some(3),
            "page_count is catalog data, not download state"
        );

        // Simulate the external-loss path: startup/reconcile clears the
        // offline flag but keeps the task's output_relpath proof.
        sqlx::query("UPDATE chapters SET local_path = NULL, output_format = NULL WHERE id = ?1")
            .bind(&ids[0])
            .execute(&lib.pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE download_tasks SET output_relpath = 'missing/cb1.cbz' WHERE id = 'tk-missing'",
        )
        .execute(&lib.pool)
        .await
        .unwrap();
        assert!(list_one(&lib).await, "lost artifact must flag missing");
    }

    #[tokio::test]
    async fn t07_remove_manga_files_reports_partial_failure_honestly() {
        let (lib, manga_a, _) = seeded().await;
        let result = remove_manga_files(&lib.pool, lib.data.path(), &manga_a)
            .await
            .unwrap();
        // c1 removed (real file); c2 fails (recorded file already missing).
        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.failed.len(), 1);
    }

    #[tokio::test]
    async fn t08_stats_and_progress_fallback_when_chapter_not_downloaded() {
        let (lib, manga_a, chapters) = seeded().await;
        let s = stats(&lib.pool).await.unwrap();
        assert_eq!(s.saved_mangas, 2);
        assert_eq!(s.downloaded_chapters, 2);
        assert!(s.local_size_bytes >= 0);
        assert_eq!(s.favorites, 1);

        // Progress on a chapter WITHOUT a local artifact must not surface
        // (Continue Reading would dead-end into a missing file).
        crate::db::repositories::progress_repo::save_progress(
            &lib.pool,
            &manga_a,
            &chapters[3],
            5,
            None,
            Some(10),
        )
        .await
        .unwrap();
        let all = list_all(&lib, "all").await;
        let a = all.iter().find(|m| m.id == manga_a).unwrap();
        assert!(a.progress.is_none(), "undownloaded progress chapter hidden");
        // And it reappears once the chapter is downloaded again.
        testkit::build_cbz(
            &lib.data.path().join("stg2"),
            &lib.library(),
            "ma/c4.cbz",
            2,
        );
        testkit::mark_downloaded(&lib.pool, &chapters[3], "ma/c4.cbz", "cbz").await;
        let all = list_all(&lib, "all").await;
        let a = all.iter().find(|m| m.id == manga_a).unwrap();
        assert_eq!(a.progress.as_ref().map(|p| p.page_index), Some(5));
    }

    /// Seed every remaining dependent table for one chapter so the erase path
    /// is proven against the FULL FK graph (RESTRICT rows must go in order).
    async fn seed_dependents(lib: &Lib, manga_id: &str, chapter_id: &str) {
        let ts = now_iso();
        sqlx::query(
            "INSERT INTO chapter_alerts (manga_id, chapter_id, detected_at) VALUES (?1, ?2, ?3)",
        )
        .bind(manga_id)
        .bind(chapter_id)
        .bind(&ts)
        .execute(&lib.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO read_history (manga_id, chapter_id, read_at) VALUES (?1, ?2, ?3)")
            .bind(manga_id)
            .bind(chapter_id)
            .bind(&ts)
            .execute(&lib.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO manga_genres (manga_id, genre) VALUES (?1, ?2)")
            .bind(manga_id)
            .bind("Hành động")
            .execute(&lib.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO download_pages (task_id, page_index, status, created_at, updated_at) VALUES (?1, 0, 'pending', ?2, ?2)")
            .bind(format!("tk-{chapter_id}")).bind(&ts).execute(&lib.pool).await.unwrap();
    }

    async fn table_count(pool: &SqlitePool, table: &str, manga_id: &str) -> i64 {
        let column = if table == "download_tasks" {
            "chapter_id"
        } else {
            "manga_id"
        };
        let sql = if table == "download_tasks" {
            format!("SELECT COUNT(*) FROM {table} WHERE {column} IN (SELECT id FROM chapters WHERE manga_id = ?1)")
        } else {
            format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1")
        };
        sqlx::query_scalar::<_, i64>(&sql)
            .bind(manga_id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn t12_erase_manga_removes_whole_record_tree_and_keeps_files_when_asked() {
        let (lib, manga_a, chapters_a) = seeded().await;
        seed_dependents(&lib, &manga_a, &chapters_a[1]).await;

        let outcome = remove_manga(&lib.pool, lib.data.path(), &manga_a, false)
            .await
            .unwrap();
        assert_eq!(outcome.chapters_removed, 4);
        assert_eq!(
            outcome.files_removed, 0,
            "keep-files mode touches nothing on disk"
        );
        for table in [
            "chapters",
            "download_tasks",
            "reading_progress",
            "favorites",
            "chapter_alerts",
            "read_history",
            "manga_genres",
        ] {
            assert_eq!(
                table_count(&lib.pool, table, &manga_a).await,
                0,
                "{table} must be gone with the manga"
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mangas WHERE id = ?1")
                .bind(&manga_a)
                .fetch_one(&lib.pool)
                .await
                .unwrap(),
            0
        );
        assert!(
            lib.library().join("ma/c1.cbz").exists(),
            "keep-files mode leaves downloads on disk"
        );
        let cascade: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM download_pages WHERE task_id = ?1")
                .bind(format!("tk-{}", chapters_a[1]))
                .fetch_one(&lib.pool)
                .await
                .unwrap();
        assert_eq!(cascade, 0, "checkpoint pages cascade with their task");
        // The other manga is untouched by the erase.
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM chapters c JOIN mangas m ON m.id = c.manga_id WHERE m.source_manga_id = 'mb'"
            )
            .fetch_one(&lib.pool)
            .await
            .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn t12_erase_manga_with_files_deletes_artifacts_too() {
        let (lib, manga_a, _chapters_a) = seeded().await;
        let outcome = remove_manga(&lib.pool, lib.data.path(), &manga_a, true)
            .await
            .unwrap();
        assert_eq!(outcome.files_removed, 1, "only the one artifact on disk");
        assert!(
            !lib.library().join("ma/c1.cbz").exists(),
            "delete-files mode removes the artifact"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mangas WHERE id = ?1")
                .bind(&manga_a)
                .fetch_one(&lib.pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn t12_erase_manga_refuses_active_tasks_and_unknown_ids() {
        let (lib, manga_a, chapters_a) = seeded().await;
        let ts = now_iso();
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, completed_pages, total_pages, attempts, revision, created_at, updated_at) VALUES ('tk-live', ?1, 'queued', 'cbz', 0, 0, 3, 0, 1, ?2, ?2)",
        )
        .bind(&chapters_a[3])
        .bind(&ts)
        .execute(&lib.pool)
        .await
        .unwrap();
        let error = remove_manga(&lib.pool, lib.data.path(), &manga_a, true)
            .await
            .expect_err("an active download blocks the erase");
        assert!(
            matches!(error, AppError::Validation(_)),
            "must ask the user to stop the queue first: {error:?}"
        );
        assert!(
            lib.library().join("ma/c1.cbz").exists(),
            "refused erase must not have touched files"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mangas WHERE id = ?1")
                .bind(&manga_a)
                .fetch_one(&lib.pool)
                .await
                .unwrap(),
            1
        );

        let unknown = remove_manga(&lib.pool, lib.data.path(), "no-such-manga", false)
            .await
            .expect_err("unknown id is not success");
        assert!(matches!(unknown, AppError::NotFound(_)));
    }
}
