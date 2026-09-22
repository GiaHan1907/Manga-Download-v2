use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::error::AppError;

const DB_FILE_NAME: &str = "manga-downloader-1989.db";

pub mod repositories;

pub async fn init_pool(app_data_dir: &Path) -> Result<SqlitePool, AppError> {
    std::fs::create_dir_all(app_data_dir)?;

    let options = SqliteConnectOptions::new()
        .filename(app_data_dir.join(DB_FILE_NAME))
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

/// Highest applied SQLx migration version; 0 only if the history table is empty.
pub async fn schema_version(pool: &SqlitePool) -> Result<i64, AppError> {
    let row = sqlx::query("SELECT COALESCE(MAX(version), 0) AS version FROM _sqlx_migrations")
        .fetch_one(pool)
        .await?;
    Ok(row.try_get::<i64, _>("version")?)
}

#[cfg(test)]
pub async fn test_pool() -> (tempfile::TempDir, SqlitePool) {
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = init_pool(dir.path()).await.expect("init pool");
    (dir, pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_init_pool_creates_app_settings() {
        let dir = tempdir().unwrap();
        let pool = init_pool(dir.path()).await.unwrap();
        let row: (String,) = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='app_settings'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, "app_settings");
    }

    #[tokio::test]
    async fn test_init_pool_idempotent() {
        let dir = tempdir().unwrap();
        let _pool1 = init_pool(dir.path()).await.unwrap();
        let _pool2 = init_pool(dir.path()).await.unwrap();
    }

    #[tokio::test]
    async fn phase04_domain_tables_exist_after_migrate() {
        let (_dir, pool) = test_pool().await;
        for table in [
            "sources",
            "mangas",
            "chapters",
            "download_tasks",
            "download_pages",
            "reading_progress",
            "favorites",
        ] {
            let row = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
                .bind(table)
                .fetch_optional(&pool)
                .await
                .unwrap();
            assert!(row.is_some(), "missing table {table}");
        }
    }

    #[tokio::test]
    async fn t04_02_upgrade_preserves_existing_app_settings_row() {
        let dir = tempdir().unwrap();
        let pool = init_pool(dir.path()).await.unwrap();
        sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)")
            .bind("theme")
            .bind("\"dark\"")
            .bind("2026-09-01T00:00:00.000Z")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        // Reopen the same database file: migration 0002 applies on top, the old
        // setting must survive untouched (T04-02) and no data doubles (T04-03).
        let pool2 = init_pool(dir.path()).await.unwrap();
        let row = sqlx::query("SELECT value_json FROM app_settings WHERE key='theme'")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(row.try_get::<String, _>("value_json").unwrap(), "\"dark\"");
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM app_settings")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn t04_03_schema_version_stable_across_reopen() {
        let dir = tempdir().unwrap();
        let pool = init_pool(dir.path()).await.unwrap();
        let first = schema_version(&pool).await.unwrap();
        pool.close().await;
        let pool2 = init_pool(dir.path()).await.unwrap();
        assert_eq!(first, schema_version(&pool2).await.unwrap());
        assert!(first >= 2, "domain migration must be applied");
    }
}
