use sqlx::SqlitePool;

use crate::db::repositories::settings_repo;
use crate::domain::settings::AppSettings;
use crate::domain::{now_iso, SettingsPatchInput};
use crate::error::AppError;

pub const SETTINGS_KEY: &str = "settings.v1";

pub async fn load(pool: &SqlitePool) -> Result<AppSettings, AppError> {
    let raw = settings_repo::get_json(pool, SETTINGS_KEY).await?;
    let stored = match raw {
        None => return Ok(AppSettings::default()),
        Some(value) => value,
    };

    // A corrupt stored value must not brick the app: fall back to defaults.
    Ok(serde_json::from_str(&stored).unwrap_or_default())
}

pub async fn apply_patch(
    pool: &SqlitePool,
    patch: SettingsPatchInput,
) -> Result<AppSettings, AppError> {
    let current = load(pool).await?;
    let next = current.merged_with_patch(patch);
    next.validate()?;
    persist(pool, &next).await?;
    Ok(next)
}

pub async fn reset(pool: &SqlitePool) -> Result<AppSettings, AppError> {
    let defaults = AppSettings::default();
    persist(pool, &defaults).await?;
    Ok(defaults)
}

async fn persist(pool: &SqlitePool, settings: &AppSettings) -> Result<(), AppError> {
    let json = serde_json::to_string(settings)
        .map_err(|_| AppError::validation("Không thể serialize cài đặt."))?;
    settings_repo::upsert_json(pool, SETTINGS_KEY, &json, &now_iso()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SettingsPatchInput;

    fn patch(chapter: Option<i64>) -> SettingsPatchInput {
        SettingsPatchInput {
            default_reader_mode: None,
            default_reading_direction: None,
            chapter_concurrency: chapter,
            image_concurrency: None,
            output_format: None,
        }
    }

    #[tokio::test]
    async fn t04_08_settings_survive_reopen_and_reject_invalid_patches() {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::db::init_pool(dir.path()).await.unwrap();

        assert_eq!(load(&pool).await.unwrap(), AppSettings::default());

        let updated = apply_patch(&pool, patch(Some(4))).await.unwrap();
        assert_eq!(updated.chapter_concurrency, 4);
        pool.close().await;

        let reopened = crate::db::init_pool(dir.path()).await.unwrap();
        assert_eq!(load(&reopened).await.unwrap().chapter_concurrency, 4);

        assert!(apply_patch(&reopened, patch(Some(99))).await.is_err());
        assert!(apply_patch(&reopened, patch(Some(0))).await.is_err());
        // Rejected patch must not have changed stored value.
        assert_eq!(load(&reopened).await.unwrap().chapter_concurrency, 4);

        let reset = reset(&reopened).await.unwrap();
        assert_eq!(reset, AppSettings::default());
    }
}
