//! Library backup: the metadata that cannot be re-derived once the app's
//! database is gone — which stories are saved, their favorites, watch flags and
//! reading position. The artifacts on disk are NOT part of this file: they live
//! in the library folder, which the user can copy with any file manager.
//!
//! Restore is additive on purpose. It never deletes a favorite, never clears a
//! watch flag and never moves the reading position backwards — a restored backup
//! must not silently undo work done after it was written.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};

use crate::db::repositories::{favorite_repo, progress_repo};
use crate::domain::now_iso;
use crate::error::AppError;
use crate::services::watch_service;

const FORMAT: &str = "manga-downloader-1989/library-backup";
const VERSION: u32 = 1;
/// Guards a hand-edited or corrupt file from filling memory.
const MAX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummaryDto {
    /// Entries in the file (export) or matched against the library (import).
    pub mangas: usize,
    pub favorites: usize,
    pub watched: usize,
    pub progress: usize,
    /// Entries whose story is not in this library — reported, never invented.
    pub skipped: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupFile {
    format: String,
    version: u32,
    exported_at: String,
    mangas: Vec<BackupManga>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManga {
    source_id: String,
    source_manga_id: String,
    title: String,
    favorite: bool,
    watch: bool,
    progress: Option<BackupProgress>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupProgress {
    chapter_source_id: String,
    page_index: i64,
    scroll_offset: Option<f64>,
}

/// Write the backup next to the path the user picked, via a temp file: a
/// half-written backup that looks complete is worse than none.
pub async fn export_to(pool: &SqlitePool, path: &Path) -> Result<BackupSummaryDto, AppError> {
    let rows = sqlx::query(
        "SELECT m.source_id, m.source_manga_id, m.title, m.watch_enabled, \
                EXISTS(SELECT 1 FROM favorites f WHERE f.manga_id = m.id) AS favorite, \
                p.page_index, p.scroll_offset, c.source_chapter_id \
         FROM mangas m \
         LEFT JOIN reading_progress p ON p.manga_id = m.id \
         LEFT JOIN chapters c ON c.id = p.chapter_id \
         ORDER BY m.title COLLATE NOCASE ASC, m.id ASC",
    )
    .fetch_all(pool)
    .await?;

    let mut mangas = Vec::with_capacity(rows.len());
    for row in rows {
        let chapter_source_id: Option<String> = row.try_get("source_chapter_id")?;
        let page_index: Option<i64> = row.try_get("page_index")?;
        let scroll_offset: Option<f64> = row.try_get("scroll_offset")?;
        mangas.push(BackupManga {
            source_id: row.try_get("source_id")?,
            source_manga_id: row.try_get("source_manga_id")?,
            title: row.try_get("title")?,
            favorite: row.try_get::<i64, _>("favorite")? == 1,
            watch: row.try_get::<Option<i64>, _>("watch_enabled")?.unwrap_or(0) == 1,
            progress: match (chapter_source_id, page_index) {
                (Some(chapter_source_id), Some(page_index)) => Some(BackupProgress {
                    chapter_source_id,
                    page_index,
                    scroll_offset,
                }),
                _ => None,
            },
        });
    }

    let file = BackupFile {
        format: FORMAT.to_string(),
        version: VERSION,
        exported_at: now_iso(),
        mangas,
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|_| AppError::validation("Không dựng được nội dung sao lưu."))?;

    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, &bytes).map_err(|error| {
        AppError::validation(format!(
            "Không ghi được file sao lưu tạm (`{}`): {error}.",
            temp.display()
        ))
    })?;
    std::fs::rename(&temp, path).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        AppError::validation(format!(
            "Không tạo được file sao lưu (`{}`): {error}.",
            path.display()
        ))
    })?;

    let summary = &file.mangas;
    Ok(BackupSummaryDto {
        mangas: summary.len(),
        favorites: summary.iter().filter(|entry| entry.favorite).count(),
        watched: summary.iter().filter(|entry| entry.watch).count(),
        progress: summary
            .iter()
            .filter(|entry| entry.progress.is_some())
            .count(),
        skipped: 0,
    })
}

/// Restore from a file the user picked. Stories that are not in this library are
/// counted, not created: a manga row without its chapters would be a ghost.
pub async fn import_from(pool: &SqlitePool, path: &Path) -> Result<BackupSummaryDto, AppError> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        AppError::validation(format!(
            "Không đọc được file sao lưu (`{}`): {error}.",
            path.display()
        ))
    })?;
    if metadata.len() > MAX_BYTES {
        return Err(AppError::validation(
            "File sao lưu lớn bất thường (> 32 MB) — có thể không phải file của app này.",
        ));
    }
    let raw = std::fs::read(path).map_err(|error| {
        AppError::validation(format!(
            "Không đọc được file sao lưu (`{}`): {error}.",
            path.display()
        ))
    })?;
    let file: BackupFile = serde_json::from_slice(&raw)
        .map_err(|_| AppError::validation("File sao lưu không hợp lệ — không đọc được JSON."))?;
    if file.format != FORMAT {
        return Err(AppError::validation(
            "File này không phải bản sao lưu của Manga Downloader 1989.",
        ));
    }
    if file.version > VERSION {
        return Err(AppError::validation(
            "File sao lưu được tạo bởi phiên bản app mới hơn — cập nhật app rồi thử lại.",
        ));
    }
    if file.mangas.len() > MAX_ENTRIES {
        return Err(AppError::validation(
            "File sao lưu có quá nhiều mục — dừng lại để tránh ghi rác vào thư viện.",
        ));
    }

    let mut summary = BackupSummaryDto {
        mangas: 0,
        favorites: 0,
        watched: 0,
        progress: 0,
        skipped: 0,
    };

    for entry in &file.mangas {
        let Some((manga_id,)) = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM mangas WHERE source_id = ?1 AND source_manga_id = ?2",
        )
        .bind(&entry.source_id)
        .bind(&entry.source_manga_id)
        .fetch_optional(pool)
        .await?
        else {
            // Not in this library — say so in the summary instead of guessing
            // which local story the entry meant.
            summary.skipped += 1;
            continue;
        };
        summary.mangas += 1;

        if entry.favorite {
            favorite_repo::set_favorite(pool, &manga_id, true).await?;
            summary.favorites += 1;
        }
        if entry.watch {
            watch_service::set_watch_enabled(pool, &manga_id, Some(true)).await?;
            summary.watched += 1;
        }
        if let Some(progress) = &entry.progress {
            // Never move a reading position that exists: the backup is older
            // than whatever the user read after exporting it.
            if progress_repo::get_progress(pool, &manga_id)
                .await?
                .is_none()
            {
                let chapter: Option<(String,)> = sqlx::query_as(
                    "SELECT id FROM chapters WHERE manga_id = ?1 AND source_chapter_id = ?2",
                )
                .bind(&manga_id)
                .bind(&progress.chapter_source_id)
                .fetch_optional(pool)
                .await?;
                if let Some((chapter_id,)) = chapter {
                    progress_repo::save_progress(
                        pool,
                        &manga_id,
                        &chapter_id,
                        progress.page_index,
                        progress.scroll_offset,
                        None,
                    )
                    .await?;
                    summary.progress += 1;
                }
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};

    async fn seed(pool: &SqlitePool) -> (String, String) {
        ensure_source(pool, "src-a", "Source A").await.unwrap();
        let (manga, _) = upsert_manga(pool, "src-a", "m1", "Truyện A", None, None, "ongoing")
            .await
            .unwrap();
        let (chapter, _) = upsert_chapter(pool, &manga, "chap-7", "Chương 7", Some(7.0), None)
            .await
            .unwrap();
        (manga, chapter)
    }

    #[tokio::test]
    async fn backup_round_trips_favorites_watch_and_progress_without_overwriting() {
        let (_dir, pool) = crate::db::test_pool().await;
        let (manga, chapter) = seed(&pool).await;
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("library-backup.json");

        favorite_repo::set_favorite(&pool, &manga, true)
            .await
            .unwrap();
        watch_service::set_watch_enabled(&pool, &manga, Some(true))
            .await
            .unwrap();
        progress_repo::save_progress(&pool, &manga, &chapter, 5, Some(0.4), None)
            .await
            .unwrap();

        let exported = export_to(&pool, &path).await.unwrap();
        assert_eq!(exported.mangas, 1);
        assert_eq!(exported.favorites, 1);
        assert_eq!(exported.watched, 1);
        assert_eq!(exported.progress, 1);

        // Losing the app's own state is what a restore is for.
        favorite_repo::set_favorite(&pool, &manga, false)
            .await
            .unwrap();
        watch_service::set_watch_enabled(&pool, &manga, Some(false))
            .await
            .unwrap();
        sqlx::query("DELETE FROM reading_progress WHERE manga_id = ?1")
            .bind(&manga)
            .execute(&pool)
            .await
            .unwrap();

        let restored = import_from(&pool, &path).await.unwrap();
        assert_eq!(restored.mangas, 1);
        assert_eq!(restored.favorites, 1);
        assert_eq!(restored.watched, 1);
        assert_eq!(restored.progress, 1);
        assert_eq!(restored.skipped, 0);

        let progress = progress_repo::get_progress(&pool, &manga).await.unwrap();
        assert_eq!(
            progress.map(|p| (p.chapter_id, p.page_index)),
            Some((chapter.clone(), 5))
        );

        // Second import: nothing left to add, and the position must not move.
        progress_repo::save_progress(&pool, &manga, &chapter, 9, None, None)
            .await
            .unwrap();
        let again = import_from(&pool, &path).await.unwrap();
        assert_eq!(again.progress, 0, "a newer position stays untouched");
        assert_eq!(
            progress_repo::get_progress(&pool, &manga)
                .await
                .unwrap()
                .map(|p| p.page_index),
            Some(9)
        );
    }

    #[tokio::test]
    async fn a_foreign_or_damaged_file_is_refused_with_a_reason() {
        let (_dir, pool) = crate::db::test_pool().await;
        seed(&pool).await;
        let out = tempfile::tempdir().unwrap();

        let missing = out.path().join("absent.json");
        assert!(matches!(
            import_from(&pool, &missing).await,
            Err(AppError::Validation(_))
        ));

        let junk = out.path().join("junk.json");
        std::fs::write(&junk, b"not json").unwrap();
        assert!(matches!(
            import_from(&pool, &junk).await,
            Err(AppError::Validation(_))
        ));

        let other = out.path().join("other.json");
        std::fs::write(
            &other,
            br#"{"format":"other-app","version":1,"exportedAt":"x","mangas":[]}"#,
        )
        .unwrap();
        assert!(matches!(
            import_from(&pool, &other).await,
            Err(AppError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn entries_for_stories_absent_here_are_counted_not_created() {
        let (_dir, pool) = crate::db::test_pool().await;
        seed(&pool).await;
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("mixed.json");
        std::fs::write(
            &path,
            format!(
                r#"{{"format":"{FORMAT}","version":1,"exportedAt":"2026-10-27T00:00:00.000Z","mangas":[
                   {{"sourceId":"src-a","sourceMangaId":"m1","title":"Truyện A","favorite":true,"watch":false,"progress":null}},
                   {{"sourceId":"src-a","sourceMangaId":"gone","title":"Truyện đã mất","favorite":true,"watch":true,"progress":null}}]}}"#
            ),
        )
        .unwrap();

        let summary = import_from(&pool, &path).await.unwrap();
        assert_eq!(summary.mangas, 1);
        assert_eq!(summary.favorites, 1);
        assert_eq!(summary.skipped, 1);
        let mangas: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mangas")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            mangas.0, 1,
            "no ghost manga is created for an unmatched entry"
        );
    }
}
