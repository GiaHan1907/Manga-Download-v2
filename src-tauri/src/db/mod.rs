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

    production_migrator().run(&pool).await?;

    Ok(pool)
}

/// Repaired SQL for migration 0004, applied at runtime in place of the file
/// version (whose checksum stays authoritative so DBs that already applied
/// 0004 keep validating).
///
/// Why: sqlx runs every migration inside one transaction; SQLite IGNORES
/// `PRAGMA foreign_keys = OFF` there, `defer_foreign_keys` does not defer
/// ON DELETE CASCADE actions, and RENAME rewrites child FK references
/// regardless of `legacy_alter_table` (all proven by the t10 tests below).
/// With FKs effectively ON, the original `DROP TABLE download_tasks`
/// cascade-deletes every `download_pages` checkpoint row on a 2→4 upgrade.
/// The repair instead rebuilds the CHILD first (pointing at the new parent's
/// temporary name), drops both old tables, then renames the new pair into
/// place — so no live checkpoint row is ever on the receiving end of a
/// cascade.
const REPAIRED_MIGRATION_4_SQL: &str = include_str!("migrations_repaired/0004_download_engine.sql");

/// Migration runner with the 0004 repair applied (see above). The stored and
/// compared checksum deliberately remains the ORIGINAL file's checksum: the
/// repair changes only how the same schema change executes, so already
/// migrated databases must not see a version/checksum mismatch.
pub(crate) fn production_migrator() -> sqlx::migrate::Migrator {
    let mut migrator = sqlx::migrate!("./migrations");
    if let Some(migration) = migrator
        .migrations
        .to_mut()
        .iter_mut()
        .find(|m| m.version == 4)
    {
        migration.sql = std::borrow::Cow::Borrowed(REPAIRED_MIGRATION_4_SQL);
    }
    migrator
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
    use sqlx::{ConnectOptions, Connection};
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

    /// Build a file DB at migration 0003 with one download task that owns
    /// per-page checkpoint rows — exactly the state a 2→4 upgrade must not
    /// damage. Returns the temp dir and an open FK-enforcing connection.
    async fn db_at_0003_with_checkpoints() -> (tempfile::TempDir, sqlx::sqlite::SqliteConnection) {
        use sqlx::sqlite::SqliteConnection;
        let dir = tempdir().unwrap();
        let opts = SqliteConnectOptions::new()
            .filename(dir.path().join("upgrade.db"))
            .create_if_missing(true)
            .foreign_keys(true);
        let mut conn = SqliteConnection::connect_with(&opts).await.unwrap();
        let mig_dir = tempdir().unwrap();
        for file in ["0001_init.sql", "0002_domain.sql", "0003_source_config.sql"] {
            std::fs::copy(
                format!("{}/migrations/{}", env!("CARGO_MANIFEST_DIR"), file),
                mig_dir.path().join(file),
            )
            .unwrap();
        }
        let early = sqlx::migrate::Migrator::new(mig_dir.path()).await.unwrap();
        early.run(&mut conn).await.unwrap();
        // Seed a valid FK chain + checkpoint rows under the 0003 schema.
        let ts = "2026-01-01T00:00:00Z";
        sqlx::query("INSERT INTO sources (id, display_name, created_at, updated_at) VALUES ('s1','S',?1,?1)")
            .bind(ts).execute(&mut conn).await.unwrap();
        sqlx::query("INSERT INTO mangas (id, source_id, source_manga_id, title, created_at, updated_at) VALUES ('m1','s1','sm1','T',?1,?1)")
            .bind(ts).execute(&mut conn).await.unwrap();
        sqlx::query("INSERT INTO chapters (id, manga_id, source_chapter_id, title, created_at, updated_at) VALUES ('c1','m1','sc1','C',?1,?1)")
            .bind(ts).execute(&mut conn).await.unwrap();
        sqlx::query("INSERT INTO download_tasks (id, chapter_id, status, target_format, completed_pages, total_pages, created_at, updated_at) VALUES ('t1','c1','queued','cbz',2,3,?1,?1)")
            .bind(ts).execute(&mut conn).await.unwrap();
        for index in 0..3i64 {
            sqlx::query("INSERT INTO download_pages (task_id, page_index, storage_key, status, size_bytes, checksum, created_at, updated_at) VALUES ('t1',?1,'p.png','stored',10,'abc',?2,?2)")
                .bind(index).bind(ts).execute(&mut conn).await.unwrap();
        }
        (dir, conn)
    }

    async fn page_count(conn: &mut sqlx::sqlite::SqliteConnection) -> i64 {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM download_pages")
            .fetch_one(conn)
            .await
            .unwrap();
        row.0
    }

    #[tokio::test]
    async fn t10_upgrade_from_0003_preserves_page_checkpoints() {
        let (_guard, mut conn) = db_at_0003_with_checkpoints().await;
        production_migrator().run(&mut conn).await.unwrap();
        assert_eq!(
            page_count(&mut conn).await,
            3,
            "the repaired 0004 must keep download_pages rows across the table swap"
        );
        // New columns are live and the task rows survived with their ids.
        let (relpath,): (Option<String>,) =
            sqlx::query_as("SELECT output_relpath FROM download_tasks WHERE id = 't1'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(relpath, None);
        // The rebuilt child must still be FK-attached to the rebuilt parent.
        let (fk_bad,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM (SELECT * FROM pragma_foreign_key_check('download_pages'))",
        )
        .fetch_one(&mut conn)
        .await
        .unwrap();
        assert_eq!(fk_bad, 0, "no dangling checkpoint FKs after the swap");
        let (pages_for_t1,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM download_pages WHERE task_id = 't1'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(pages_for_t1, 3);
        // Re-running the runner must pass checksum validation (stored
        // checksum is the ORIGINAL file's — repair keeps it).
        production_migrator().run(&mut conn).await.unwrap();
    }

    #[tokio::test]
    async fn t10_unrepaired_0004_would_wipe_checkpoints_test_discriminates() {
        // Negative control proving the hazard is real (and that the test
        // above measures it): the original file's `PRAGMA foreign_keys = OFF`
        // is ignored inside sqlx's migration transaction, so the DROP
        // CASCADES into download_pages.
        let (_guard, mut conn) = db_at_0003_with_checkpoints().await;
        let mig_dir = tempdir().unwrap();
        for file in ["0004_download_engine.sql", "0005_library_reader.sql"] {
            std::fs::copy(
                format!("{}/migrations/{}", env!("CARGO_MANIFEST_DIR"), file),
                mig_dir.path().join(file),
            )
            .unwrap();
        }
        let mut old = sqlx::migrate::Migrator::new(mig_dir.path()).await.unwrap();
        // The DB already carries versions 1-3; this runner only holds 4+5.
        old.ignore_missing = true;
        old.run(&mut conn).await.unwrap();
        assert_eq!(
            page_count(&mut conn).await,
            0,
            "sanity: the ORIGINAL 0004 wipes checkpoints under FK ON — the runtime repair is what prevents this"
        );
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

    /// T06-01: a Phase 05-shaped database (0001→0003 applied, rows written,
    /// then 0004 rebuild applied) keeps catalog, favorites and progress and
    /// only widens the task lifecycle. Built by replaying the migration files
    /// verbatim so the rebuild recipe is tested, not just its intent.
    #[tokio::test]
    async fn t06_01_phase05_upgrade_preserves_rows_and_widens_lifecycle() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("upgrade.db");
        let mut conn = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .connect()
            .await
            .unwrap();
        for file in [
            "migrations/0001_init.sql",
            "migrations/0002_domain.sql",
            "migrations/0003_source_config.sql",
        ] {
            let sql = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file),
            )
            .unwrap();
            sqlx::raw_sql(&sql).execute(&mut conn).await.unwrap();
        }
        // Phase 05-shaped data: source, manga, chapter, favorite, progress.
        let ts = crate::domain::now_iso();
        sqlx::raw_sql(
            "INSERT INTO sources (id, display_name, enabled, adapter, base_url, created_at, updated_at) \
             VALUES ('madara-a.example-com','A',1,'madara','https://a.example.com/','t','t'); \
             INSERT INTO mangas (id, source_id, source_manga_id, title, status, created_at, updated_at) \
             VALUES ('mg-1','madara-a.example-com','slug','Title','ongoing','t','t'); \
             INSERT INTO chapters (id, manga_id, source_chapter_id, title, number, page_count, created_at, updated_at) \
             VALUES ('ch-1','mg-1','tap-1','C1',1,NULL,'t','t'); \
             INSERT INTO favorites (manga_id, created_at) VALUES ('mg-1','t'); \
             INSERT INTO reading_progress (manga_id, chapter_id, page_index, updated_at) VALUES ('mg-1','ch-1',4,'t'); \
             INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, completed_pages, total_pages, created_at, updated_at) \
             VALUES ('tk-1','ch-1','queued','cbz',0,0,0,'t','t');",
        )
        .execute(&mut conn)
        .await
        .unwrap();

        let migration_0004 = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("migrations/0004_download_engine.sql"),
        )
        .unwrap();
        sqlx::raw_sql(&migration_0004)
            .execute(&mut conn)
            .await
            .unwrap();

        // Everything survived the rebuild.
        let (favorites,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM favorites")
            .fetch_one(&mut conn)
            .await
            .unwrap();
        assert_eq!(favorites, 1);
        let (progress,): (i64,) =
            sqlx::query_as("SELECT page_index FROM reading_progress WHERE manga_id='mg-1'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(progress, 4);
        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM download_tasks WHERE id='tk-1'")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        assert_eq!(status, "queued");

        // The widened CHECK now accepts active states, still rejects junk.
        sqlx::raw_sql("UPDATE download_tasks SET status = 'downloading' WHERE id='tk-1';")
            .execute(&mut conn)
            .await
            .unwrap();
        assert!(
            sqlx::raw_sql("UPDATE download_tasks SET status = 'nonsense' WHERE id='tk-1';")
                .execute(&mut conn)
                .await
                .is_err()
        );
        // FK cascade path intact: pages can still attach to the rebuilt table.
        sqlx::raw_sql(&format!(
            "INSERT INTO download_pages (task_id, page_index, status, created_at, updated_at) \
             VALUES ('tk-1', 0, 'pending', '{ts}', '{ts}');"
        ))
        .execute(&mut conn)
        .await
        .unwrap();
        // Deleting the manga must still be RESTRICTed by the surviving task.
        assert!(sqlx::raw_sql("DELETE FROM mangas WHERE id='mg-1';")
            .execute(&mut conn)
            .await
            .is_err());
        conn.close().await.unwrap();
    }

    /// T09-28: concurrent writers on the shared pool must serialize via
    /// busy_timeout — no SQLITE_BUSY escapes to the user, no lost updates.
    #[tokio::test]
    async fn t09_concurrent_writers_serialize_without_busy_errors() {
        let (_dir, pool) = test_pool().await;
        let ts = crate::domain::now_iso();
        sqlx::query(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES ('busy-k', '0', ?1)",
        )
        .bind(&ts)
        .execute(&pool)
        .await
        .unwrap();

        let mut handles = Vec::new();
        for index in 0..8u32 {
            let pool = pool.clone();
            handles.push(tokio::spawn(async move {
                sqlx::query("UPDATE app_settings SET value_json = ?2 WHERE key = 'busy-k'")
                    .bind(index)
                    .bind(format!("\"{index}\""))
                    .execute(&pool)
                    .await
            }));
        }
        for handle in handles {
            handle
                .await
                .unwrap()
                .expect("busy_timeout must absorb contention");
        }
        let value: (String,) =
            sqlx::query_as("SELECT value_json FROM app_settings WHERE key='busy-k'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            value.0.starts_with('"') && value.0.len() == 3,
            "one final value: {}",
            value.0
        );
    }
}
