//! Discover Phase 2 — the user's recent search queries. The table stores only
//! what was typed and when; results, sources and filters are never persisted.
//! Re-running a query refreshes its position instead of adding a row, and every
//! write prunes the list to the newest [`MAX_ENTRIES`] so it stays a memory aid
//! rather than an archive.

use sqlx::Row;
use sqlx::SqlitePool;

use crate::domain::{now_iso, RecentSearchDto};
use crate::error::AppError;

pub const MAX_ENTRIES: i64 = 10;
/// Longer than any query a user would type; guards the table against a blob.
pub const MAX_QUERY_CHARS: usize = 80;

/// Trim + collapse inner whitespace so "naruto   ship" and "naruto ship" are
/// one entry. Errors instead of silently storing noise.
pub fn normalize(query: &str) -> Result<String, AppError> {
    let collapsed = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return Err(AppError::validation("Câu truy vấn trống."));
    }
    if collapsed.chars().count() > MAX_QUERY_CHARS {
        return Err(AppError::validation("Câu truy vấn quá dài."));
    }
    Ok(collapsed)
}

pub async fn record(pool: &SqlitePool, query: &str) -> Result<Vec<RecentSearchDto>, AppError> {
    let query = normalize(query)?;
    sqlx::query(
        "INSERT INTO recent_searches (query, searched_at) VALUES (?1, ?2) \
         ON CONFLICT(query) DO UPDATE SET searched_at = excluded.searched_at",
    )
    .bind(&query)
    .bind(now_iso())
    .execute(pool)
    .await?;
    // Keep the newest N by timestamp; ties are impossible in practice because
    // `searched_at` carries milliseconds.
    sqlx::query(
        "DELETE FROM recent_searches WHERE query NOT IN \
         (SELECT query FROM recent_searches ORDER BY searched_at DESC LIMIT ?1)",
    )
    .bind(MAX_ENTRIES)
    .execute(pool)
    .await?;
    list(pool).await
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<RecentSearchDto>, AppError> {
    let rows =
        sqlx::query("SELECT query, searched_at FROM recent_searches ORDER BY searched_at DESC")
            .fetch_all(pool)
            .await?;
    Ok(rows
        .iter()
        .map(|row| RecentSearchDto {
            query: row.get("query"),
            searched_at: row.get("searched_at"),
        })
        .collect())
}

pub async fn remove(pool: &SqlitePool, query: &str) -> Result<Vec<RecentSearchDto>, AppError> {
    let query = normalize(query)?;
    sqlx::query("DELETE FROM recent_searches WHERE query = ?1")
        .bind(&query)
        .execute(pool)
        .await?;
    list(pool).await
}

pub async fn clear(pool: &SqlitePool) -> Result<i64, AppError> {
    let result = sqlx::query("DELETE FROM recent_searches")
        .execute(pool)
        .await?;
    Ok(i64::try_from(result.rows_affected()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_inner_whitespace() {
        assert_eq!(normalize("  naruto   ship  ").unwrap(), "naruto ship");
    }

    #[test]
    fn normalize_rejects_empty_and_oversize() {
        assert!(normalize("   ").is_err());
        let long = "a".repeat(MAX_QUERY_CHARS + 1);
        assert!(normalize(&long).is_err());
    }

    /// A migrated file DB in a temp dir. `searched_at` has millisecond
    /// precision, so ordering assertions sleep between writes.
    async fn db() -> (tempfile::TempDir, SqlitePool) {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::db::init_pool(dir.path()).await.unwrap();
        (dir, pool)
    }

    #[tokio::test]
    async fn record_dedupes_and_moves_the_refreshed_query_to_the_top() {
        let (_dir, db) = db().await;
        record(&db, "one").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        record(&db, "two").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        let after = record(&db, "  one  ").await.unwrap();
        assert_eq!(
            after.iter().map(|e| e.query.as_str()).collect::<Vec<_>>(),
            vec!["one", "two"],
            "re-running a query refreshes it instead of duplicating it"
        );
    }

    #[tokio::test]
    async fn record_prunes_to_the_newest_entries() {
        let (_dir, db) = db().await;
        for index in 0..(MAX_ENTRIES + 5) {
            record(&db, &format!("q{index}")).await.unwrap();
        }
        let listed = list(&db).await.unwrap();
        assert_eq!(listed.len() as i64, MAX_ENTRIES);
        assert_eq!(
            listed.first().unwrap().query,
            format!("q{}", MAX_ENTRIES + 4)
        );
    }

    #[tokio::test]
    async fn remove_and_clear_only_touch_this_table() {
        let (_dir, db) = db().await;
        record(&db, "keep").await.unwrap();
        record(&db, "drop").await.unwrap();
        let after = remove(&db, "drop").await.unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].query, "keep");
        assert_eq!(clear(&db).await.unwrap(), 1);
        assert!(list(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn empty_query_is_rejected_before_it_reaches_the_table() {
        let (_dir, db) = db().await;
        assert!(record(&db, "   ").await.is_err());
        assert!(list(&db).await.unwrap().is_empty());
    }
}
