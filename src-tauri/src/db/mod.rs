use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::error::AppError;

pub async fn init_pool(app_data_dir: &Path) -> Result<SqlitePool, AppError> {
    std::fs::create_dir_all(app_data_dir)?;

    let db_path = app_data_dir.join("manga-downloader-1989.db");

    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_init_pool_creates_app_settings() {
        let dir = tempdir().unwrap();
        let pool = init_pool(dir.path()).await.unwrap();
        let row: (String,) =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name='app_settings'")
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
}
