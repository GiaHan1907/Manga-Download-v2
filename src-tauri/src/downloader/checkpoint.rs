//! Page checkpoint reconciliation: after any interruption the database rows
//! and the staged files must agree before work resumes. A page counts as
//! done only when its row says `stored` AND the final staged file exists and
//! matches the recorded size/CRC — a filename or DB flag alone is never
//! enough (design §2.4).

use std::path::Path;

use sqlx::SqlitePool;

use crate::db::repositories::download_repo::{self, PageRow};
use crate::downloader::image_validate;
use crate::error::AppError;

#[derive(Debug, Default)]
pub struct ReconcileReport {
    pub valid: usize,
    pub reset_to_pending: usize,
}

/// Verify every `stored` page row against its staged file; invalid rows are
/// reset to `pending` so they are re-fetched. Returns counts for logging and
/// the list of still-valid (index, name) pairs.
pub async fn reconcile(
    pool: &SqlitePool,
    task_id: &str,
    staging_dir: &Path,
) -> Result<(ReconcileReport, Vec<(usize, String)>), AppError> {
    let pages = download_repo::list_pages(pool, task_id).await?;
    let mut report = ReconcileReport::default();
    let mut valid_pages: Vec<(usize, String)> = Vec::new();
    for page in pages {
        let intact = match &page {
            PageRow {
                status,
                storage_key: Some(name),
                size_bytes,
                checksum,
                ..
            } if status == "stored" => {
                staged_file_intact(staging_dir, name, *size_bytes, checksum.as_deref())
            }
            _ => false,
        };
        if intact {
            report.valid += 1;
            if let PageRow {
                storage_key: Some(name),
                ..
            } = &page
            {
                valid_pages.push((page.page_index as usize, name.clone()));
            }
        } else if page.status == "stored" {
            report.reset_to_pending += 1;
            sqlx::query(
                "UPDATE download_pages SET status = 'pending', storage_key = NULL, \
                 size_bytes = NULL, checksum = NULL, updated_at = ?3 \
                 WHERE task_id = ?1 AND page_index = ?2",
            )
            .bind(task_id)
            .bind(page.page_index)
            .bind(crate::domain::now_iso())
            .execute(pool)
            .await?;
        }
    }
    if report.reset_to_pending > 0 {
        let (done,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM download_pages WHERE task_id = ?1 AND status = 'stored'",
        )
        .bind(task_id)
        .fetch_one(pool)
        .await?;
        sqlx::query("UPDATE download_tasks SET completed_pages = ?2 WHERE id = ?1")
            .bind(task_id)
            .bind(done)
            .execute(pool)
            .await?;
    }
    Ok((report, valid_pages))
}

fn staged_file_intact(
    staging_dir: &Path,
    name: &str,
    size: Option<i64>,
    checksum: Option<&str>,
) -> bool {
    let Some(size) = size else { return false };
    let Ok(bytes) = std::fs::read(staging_dir.join(name)) else {
        return false;
    };
    if bytes.len() as i64 != size {
        return false;
    }
    match checksum {
        Some(expected) => image_validate::crc32_hex(&bytes) == expected,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::download_repo;
    use crate::db::test_pool;

    #[tokio::test]
    async fn t06_12_stored_page_with_missing_file_resets_to_pending() {
        let (_dir, pool) = test_pool().await;
        let staging = tempfile::tempdir().unwrap();

        let (_source_setup, chapter_id) = seed(&pool).await;
        let task = download_repo::queue_batch(&pool, &[chapter_id], "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        download_repo::ensure_page_rows(&pool, &task.id, 2)
            .await
            .unwrap();

        let png = test_png();
        download_repo::mark_page_stored(
            &pool,
            &task.id,
            0,
            "000000.png",
            png.len() as i64,
            &image_validate::crc32_hex(&png),
        )
        .await
        .unwrap();
        // File intentionally NOT written → checkpoint must invalidate it.
        let (report, valid) = reconcile(&pool, &task.id, staging.path()).await.unwrap();
        assert_eq!(report.reset_to_pending, 1);
        assert!(valid.is_empty());
        let task_after = download_repo::get_task(&pool, &task.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task_after.completed_pages, 0);

        // Now write the real file: reconcile accepts the stored row.
        sqlx::query(
            "UPDATE download_pages SET status='stored', storage_key='000000.png', size_bytes=?2, checksum=?3 \
             WHERE task_id=?1 AND page_index=0",
        )
        .bind(&task.id)
        .bind(png.len() as i64)
        .bind(image_validate::crc32_hex(&png))
        .execute(&pool)
        .await
        .unwrap();
        std::fs::write(staging.path().join("000000.png"), &png).unwrap();
        let (report, valid) = reconcile(&pool, &task.id, staging.path()).await.unwrap();
        assert_eq!(report.valid, 1);
        assert_eq!(valid, vec![(0, "000000.png".to_string())]);
    }

    fn test_png() -> Vec<u8> {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x1a, 0x0a, 0x00];
        png.extend(std::iter::repeat_n(0x22u8, 32));
        png
    }

    async fn seed(pool: &SqlitePool) -> (String, String) {
        use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};
        ensure_source(pool, "src-a", "A").await.unwrap();
        let (manga_id, _) = upsert_manga(pool, "src-a", "m", "T", None, None, "ongoing")
            .await
            .unwrap();
        let (chapter_id, _) = upsert_chapter(pool, &manga_id, "c1", "C1", None, None)
            .await
            .unwrap();
        (manga_id, chapter_id)
    }
}
