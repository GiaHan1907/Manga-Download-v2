use sqlx::{Row, SqlitePool};

use crate::error::AppError;

pub async fn get_json(pool: &SqlitePool, key: &str) -> Result<Option<String>, AppError> {
    let row = sqlx::query("SELECT value_json FROM app_settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.try_get::<String, _>("value_json").unwrap()))
}

pub async fn upsert_json(
    pool: &SqlitePool,
    key: &str,
    value_json: &str,
    now: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value_json)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}
