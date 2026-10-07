use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::domain::{new_id, now_iso, ChapterDto, MangaDto};
use crate::error::AppError;

/// Register a source identity if absent (idempotent). Sources carry no web
/// adapter in Phase 04 — they exist so registered metadata has a stable owner.
/// Written metadata flows only through Phase 05 adapters (see docs/IPC_CONTRACT.md).
#[allow(dead_code)]
pub async fn ensure_source(
    pool: &SqlitePool,
    source_id: &str,
    display_name: &str,
) -> Result<(), AppError> {
    let ts = now_iso();
    sqlx::query(
        "INSERT INTO sources (id, display_name, enabled, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, ?3)
         ON CONFLICT(id) DO UPDATE SET display_name = excluded.display_name, updated_at = excluded.updated_at",
    )
    .bind(source_id)
    .bind(display_name)
    .bind(&ts)
    .execute(pool)
    .await?;
    Ok(())
}

/// Idempotent manga upsert keyed by (source_id, source_manga_id). Existing rows
/// keep their internal id and user records; only metadata columns refresh.
/// Repository API reserved for Phase 05 registration — no IPC exposes raw upsert.
#[allow(dead_code)]
pub async fn upsert_manga(
    pool: &SqlitePool,
    source_id: &str,
    source_manga_id: &str,
    title: &str,
    author: Option<&str>,
    description: Option<&str>,
    status: &str,
) -> Result<(String, bool), AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let result = upsert_manga_tx(
        tx.as_mut(),
        source_id,
        source_manga_id,
        title,
        author,
        description,
        status,
    )
    .await;
    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

async fn upsert_manga_tx(
    conn: &mut SqliteConnection,
    source_id: &str,
    source_manga_id: &str,
    title: &str,
    author: Option<&str>,
    description: Option<&str>,
    status: &str,
) -> Result<(String, bool), AppError> {
    let ts = now_iso();
    let existing =
        sqlx::query("SELECT id FROM mangas WHERE source_id = ?1 AND source_manga_id = ?2")
            .bind(source_id)
            .bind(source_manga_id)
            .fetch_optional(&mut *conn)
            .await?;

    if let Some(row) = existing {
        let id: String = row.try_get("id")?;
        sqlx::query(
            "UPDATE mangas
             SET title = ?2, author = ?3, description = ?4, status = ?5, updated_at = ?6
             WHERE id = ?1",
        )
        .bind(&id)
        .bind(title)
        .bind(author)
        .bind(description)
        .bind(status)
        .bind(&ts)
        .execute(&mut *conn)
        .await?;
        return Ok((id, false));
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO mangas (id, source_id, source_manga_id, title, author, description, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
    )
    .bind(&id)
    .bind(source_id)
    .bind(source_manga_id)
    .bind(title)
    .bind(author)
    .bind(description)
    .bind(status)
    .bind(&ts)
    .execute(&mut *conn)
    .await?;
    Ok((id, true))
}

#[allow(dead_code)]
pub async fn upsert_chapter(
    pool: &SqlitePool,
    manga_id: &str,
    source_chapter_id: &str,
    title: &str,
    number: Option<f64>,
    page_count: Option<i64>,
) -> Result<(String, bool), AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

    let manga_exists: Option<(String,)> = sqlx::query_as("SELECT id FROM mangas WHERE id = ?1")
        .bind(manga_id)
        .fetch_optional(&mut *tx)
        .await?;
    if manga_exists.is_none() {
        tx.rollback().await?;
        return Err(AppError::not_found("manga của chương"));
    }

    let ts = now_iso();
    let existing =
        sqlx::query("SELECT id FROM chapters WHERE manga_id = ?1 AND source_chapter_id = ?2")
            .bind(manga_id)
            .bind(source_chapter_id)
            .fetch_optional(&mut *tx)
            .await?;

    let result = if let Some(row) = existing {
        let id: String = row.try_get("id")?;
        sqlx::query(
            "UPDATE chapters SET title = ?2, number = ?3, page_count = ?4, updated_at = ?5 WHERE id = ?1",
        )
        .bind(&id)
        .bind(title)
        .bind(number)
        .bind(page_count)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
        (id, false)
    } else {
        let id = new_id();
        sqlx::query(
            "INSERT INTO chapters (id, manga_id, source_chapter_id, title, number, page_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        )
        .bind(&id)
        .bind(manga_id)
        .bind(source_chapter_id)
        .bind(title)
        .bind(number)
        .bind(page_count)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
        (id, true)
    };

    tx.commit().await?;
    Ok(result)
}

pub(crate) const MANGA_SELECT: &str = "SELECT m.id, m.source_id, m.source_manga_id, m.title, m.author, m.description, m.status, m.cover_key, m.updated_at, \
     (SELECT COUNT(*) FROM chapters c WHERE c.manga_id = m.id) AS chapter_count, \
     EXISTS(SELECT 1 FROM favorites f WHERE f.manga_id = m.id) AS favorite \
     FROM mangas m";

pub(crate) fn map_manga(row: &sqlx::sqlite::SqliteRow) -> Result<MangaDto, AppError> {
    Ok(MangaDto {
        id: row.try_get("id")?,
        source_id: row.try_get("source_id")?,
        source_manga_id: row.try_get("source_manga_id")?,
        title: row.try_get("title")?,
        author: row.try_get("author")?,
        description: row.try_get("description")?,
        status: row.try_get("status")?,
        chapter_count: row.try_get("chapter_count")?,
        favorite: row.try_get::<bool, _>("favorite")?,
        updated_at: row.try_get("updated_at")?,
        cover_key: row.try_get("cover_key")?,
    })
}

fn escape_like(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub async fn get_manga(pool: &SqlitePool, manga_id: &str) -> Result<Option<MangaDto>, AppError> {
    let row = sqlx::query(&format!("{MANGA_SELECT} WHERE m.id = ?1"))
        .bind(manga_id)
        .fetch_optional(pool)
        .await?;
    row.map(|r| map_manga(&r)).transpose()
}

pub async fn list_mangas(
    pool: &SqlitePool,
    query: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<MangaDto>, i64), AppError> {
    let total: (i64,) = if let Some(q) = query {
        let pattern = format!("%{}%", escape_like(q));
        sqlx::query_as(
            "SELECT COUNT(*) FROM mangas WHERE title LIKE ?1 ESCAPE '\\' OR IFNULL(author,'') LIKE ?1 ESCAPE '\\'",
        )
        .bind(&pattern)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM mangas")
            .fetch_one(pool)
            .await?
    };

    let rows = if let Some(q) = query {
        let pattern = format!("%{}%", escape_like(q));
        sqlx::query(&format!(
            "{MANGA_SELECT} WHERE m.title LIKE ?1 ESCAPE '\\' OR IFNULL(m.author,'') LIKE ?1 ESCAPE '\\' \
             ORDER BY m.title COLLATE NOCASE ASC, m.id ASC LIMIT ?2 OFFSET ?3"
        ))
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(&format!(
            "{MANGA_SELECT} ORDER BY m.title COLLATE NOCASE ASC, m.id ASC LIMIT ?1 OFFSET ?2"
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    };

    rows.iter()
        .map(map_manga)
        .collect::<Result<Vec<_>, _>>()
        .map(|items| (items, total.0))
}

/// Chapters that still have no number, with the slug a parser can read.
pub async fn chapters_missing_number(pool: &SqlitePool) -> Result<Vec<(String, String)>, AppError> {
    sqlx::query_as(
        "SELECT id, source_chapter_id FROM chapters WHERE number IS NULL ORDER BY manga_id, id",
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

/// Fill a chapter number that was never derived. The `number IS NULL` guard
/// means this can never rewrite a number a catalog refresh already stored, so
/// the repair is safe to run on every start.
pub async fn fill_chapter_number(
    pool: &SqlitePool,
    chapter_id: &str,
    number: f64,
) -> Result<u64, AppError> {
    let result = sqlx::query("UPDATE chapters SET number = ?2 WHERE id = ?1 AND number IS NULL")
        .bind(chapter_id)
        .bind(number)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn list_chapters(pool: &SqlitePool, manga_id: &str) -> Result<Vec<ChapterDto>, AppError> {
    let rows = sqlx::query(
        "SELECT id, manga_id, source_chapter_id, title, number, page_count, local_path, output_format \
         FROM chapters WHERE manga_id = ?1 \
         ORDER BY number IS NULL ASC, number ASC, created_at ASC, id ASC",
    )
    .bind(manga_id)
    .fetch_all(pool)
    .await?;

    rows.iter()
        .map(|row| {
            let local_path: Option<String> = row.try_get("local_path")?;
            Ok(ChapterDto {
                id: row.try_get("id")?,
                manga_id: row.try_get("manga_id")?,
                source_chapter_id: row.try_get("source_chapter_id")?,
                title: row.try_get("title")?,
                number: row.try_get("number")?,
                page_count: row.try_get("page_count")?,
                downloaded: local_path.is_some(),
                output_format: row.try_get("output_format")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn t04_04_duplicate_source_identity_reuses_manga() {
        let (_dir, pool) = crate::db::test_pool().await;
        ensure_source(&pool, "src-a", "Source A").await.unwrap();

        let (first_id, created_first) =
            upsert_manga(&pool, "src-a", "m1", "Title v1", Some("A"), None, "ongoing")
                .await
                .unwrap();
        assert!(created_first);

        let (second_id, created_again) = upsert_manga(
            &pool,
            "src-a",
            "m1",
            "Title v2",
            Some("A"),
            None,
            "completed",
        )
        .await
        .unwrap();
        assert!(!created_again);
        assert_eq!(first_id, second_id);

        let manga = get_manga(&pool, &first_id).await.unwrap().unwrap();
        assert_eq!(manga.title, "Title v2");
        assert_eq!(manga.status, "completed");

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mangas")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn t04_05_duplicate_chapter_identity_reuses_id_unknown_manga_rejected() {
        let (_dir, pool) = crate::db::test_pool().await;
        ensure_source(&pool, "src-a", "Source A").await.unwrap();
        let (manga_id, _) = upsert_manga(&pool, "src-a", "m1", "T", None, None, "ongoing")
            .await
            .unwrap();

        let (c1, created) =
            upsert_chapter(&pool, &manga_id, "c-1", "Chương 1", Some(1.0), Some(10))
                .await
                .unwrap();
        assert!(created);
        let (c2, created_again) =
            upsert_chapter(&pool, &manga_id, "c-1", "Chương 1b", Some(1.0), Some(12))
                .await
                .unwrap();
        assert!(!created_again);
        assert_eq!(c1, c2);

        let error = upsert_chapter(&pool, "does-not-exist", "c-x", "T", None, None)
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::NotFound(_)));

        let chapters = list_chapters(&pool, &manga_id).await.unwrap();
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, "Chương 1b");
    }

    #[tokio::test]
    async fn t04_11_pagination_is_bounded_and_deterministic() {
        let (_dir, pool) = crate::db::test_pool().await;
        ensure_source(&pool, "src-a", "Source A").await.unwrap();
        for index in 0..25 {
            upsert_manga(
                &pool,
                "src-a",
                &format!("m{index:02}"),
                &format!("Manga {index:02}"),
                None,
                None,
                "ongoing",
            )
            .await
            .unwrap();
        }

        let (page1, total) = list_mangas(&pool, None, 10, 0).await.unwrap();
        let (page2, _) = list_mangas(&pool, None, 10, 10).await.unwrap();
        assert_eq!(total, 25);
        assert_eq!(page1.len(), 10);
        assert_eq!(page2.len(), 10);
        assert_eq!(page1[0].title, "Manga 00");
        assert_eq!(page2[0].title, "Manga 10");
        let ids: std::collections::HashSet<_> = page1
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>()
            .iter()
            .cloned()
            .collect();
        assert_eq!(ids.len(), 10);

        let (searched, _) = list_mangas(&pool, Some("manga 1%"), 10, 0).await.unwrap();
        assert!(
            searched.is_empty(),
            "LIKE wildcards in user query must be escaped"
        );
    }
}
