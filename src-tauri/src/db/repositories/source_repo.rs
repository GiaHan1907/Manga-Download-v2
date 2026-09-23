use sqlx::{Row, SqlitePool};

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct SourceRow {
    pub id: String,
    pub display_name: String,
    pub adapter: String,
    pub base_url: Option<String>,
    /// Comma-separated exact hostnames additionally allowed for chapter
    /// images (schema 8). None = base host only.
    pub image_hosts: Option<String>,
    pub enabled: bool,
}

pub async fn list_sources(pool: &SqlitePool) -> Result<Vec<SourceRow>, AppError> {
    let rows = sqlx::query(
        "SELECT id, display_name, adapter, base_url, image_hosts, enabled FROM sources ORDER BY display_name COLLATE NOCASE ASC, id ASC",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(SourceRow {
                id: row.try_get("id")?,
                display_name: row.try_get("display_name")?,
                adapter: row.try_get("adapter")?,
                base_url: row.try_get("base_url")?,
                image_hosts: row.try_get("image_hosts")?,
                enabled: row.try_get::<i64, _>("enabled")? != 0,
            })
        })
        .collect()
}

pub async fn upsert_source(
    pool: &SqlitePool,
    id: &str,
    display_name: &str,
    adapter: &str,
    base_url: Option<&str>,
    image_hosts: Option<&str>,
) -> Result<(), AppError> {
    let ts = crate::domain::now_iso();
    sqlx::query(
        "INSERT INTO sources (id, display_name, enabled, adapter, base_url, image_hosts, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(id) DO UPDATE SET display_name = excluded.display_name, adapter = excluded.adapter, base_url = excluded.base_url, image_hosts = excluded.image_hosts, updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(display_name)
    .bind(adapter)
    .bind(base_url)
    .bind(image_hosts)
    .bind(&ts)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> Result<(), AppError> {
    let result = sqlx::query("UPDATE sources SET enabled = ?2, updated_at = ?3 WHERE id = ?1")
        .bind(id)
        .bind(if enabled { 1_i64 } else { 0_i64 })
        .bind(crate::domain::now_iso())
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("source"));
    }
    Ok(())
}

pub async fn delete_source(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM sources WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("source"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rows_survive_reopen_and_idempotent_upsert() {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::db::init_pool(dir.path()).await.unwrap();
        upsert_source(
            &pool,
            "madara-a",
            "Site A",
            "madara",
            Some("https://a.example/"),
            Some("img.a.example"),
        )
        .await
        .unwrap();
        upsert_source(
            &pool,
            "madara-a",
            "Site A rename",
            "madara",
            Some("https://a.example/"),
            None,
        )
        .await
        .unwrap();
        set_enabled(&pool, "madara-a", false).await.unwrap();
        pool.close().await;

        let reopened = crate::db::init_pool(dir.path()).await.unwrap();
        let rows = list_sources(&reopened).await.unwrap();
        assert_eq!(rows.len(), 1, "upsert must not duplicate");
        assert_eq!(rows[0].display_name, "Site A rename");
        assert!(!rows[0].enabled);
        assert!(
            rows[0].image_hosts.is_none(),
            "re-upsert must replace, not merge, image hosts"
        );
        assert!(set_enabled(&reopened, "ghost", true).await.is_err());
    }
}
