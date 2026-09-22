use sqlx::{FromRow, SqlitePool};

use crate::domain::{new_id, now_iso, MAX_ID_LEN};
use crate::error::AppError;

/// Persisted lifecycle states (migration 0004 CHECK).
pub const STATUS_STATES: &[&str] = &[
    "queued",
    "resolving",
    "downloading",
    "pausing",
    "paused",
    "verifying",
    "exporting",
    "completed",
    "failed",
    "cancelled",
];
/// States a live worker may own. A crash always resets these — the UI must
/// never display an ACTIVE state without a running worker behind it.
pub const ACTIVE_STATES: &[&str] = &["resolving", "downloading", "verifying", "exporting"];
/// A chapter may own at most one of these at a time (queue dedup key).
pub const NON_TERMINAL_STATES: &[&str] = &[
    "queued",
    "resolving",
    "downloading",
    "pausing",
    "paused",
    "verifying",
    "exporting",
    "failed",
];

pub const MAX_BATCH_SIZE: usize = 100;
pub const MAX_PRIORITY: i64 = 10;

#[derive(Debug, Clone, FromRow)]
pub struct TaskRow {
    pub id: String,
    pub chapter_id: String,
    pub status: String,
    pub target_format: String,
    pub priority: i64,
    pub completed_pages: i64,
    pub total_pages: i64,
    pub attempts: i64,
    pub next_retry_at: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub revision: i64,
    pub finished_at: Option<String>,
    pub output_relpath: Option<String>,
    #[sqlx(default)]
    pub chapter_title: Option<String>,
    #[sqlx(default)]
    pub manga_title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct PageRow {
    pub page_index: i64,
    pub storage_key: Option<String>,
    pub status: String,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
}

/// Everything a worker needs to resolve a chapter back to its owning adapter,
/// joined server-side so the renderer never handles source identity.
#[derive(Debug, Clone, FromRow)]
pub struct ChapterContext {
    pub chapter_id: String,
    pub chapter_title: String,
    pub manga_id: String,
    pub manga_title: String,
    pub source_id: String,
    pub source_manga_id: String,
    pub source_chapter_id: String,
}

#[derive(Debug, Clone)]
pub struct QueueError {
    pub chapter_id: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct QueueOutcome {
    pub created: Vec<TaskRow>,
    pub reused: Vec<TaskRow>,
    pub errors: Vec<QueueError>,
}

const TASK_SELECT: &str =
    "SELECT t.id, t.chapter_id, t.status, t.target_format, t.priority, t.completed_pages, \
     t.total_pages, t.attempts, t.next_retry_at, t.error_code, t.error_message, t.revision, \
     t.finished_at, t.output_relpath, c.title AS chapter_title, m.title AS manga_title, \
     t.created_at, t.updated_at \
     FROM download_tasks t \
     JOIN chapters c ON c.id = t.chapter_id \
     JOIN mangas m ON m.id = c.manga_id";

/// `?N` placeholders starting at `start` for each value (bind order matters).
fn placeholders(start: usize, count: usize) -> String {
    (start..start + count)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_LEN
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

async fn task_by_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
) -> Result<TaskRow, AppError> {
    let task = sqlx::query_as::<_, TaskRow>(&format!("{TASK_SELECT} WHERE t.id = ?1"))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
    task.ok_or_else(|| AppError::not_found("task"))
}

/// One transaction for the whole batch. A chapter that already owns a
/// non-terminal task is reused, never duplicated; completed/cancelled tasks
/// do not block a fresh submission.
pub async fn queue_batch(
    pool: &SqlitePool,
    chapter_ids: &[String],
    target_format: &str,
    priority: i64,
) -> Result<QueueOutcome, AppError> {
    if chapter_ids.is_empty() || chapter_ids.len() > MAX_BATCH_SIZE {
        return Err(AppError::validation(
            "Danh sách chương phải có từ 1 đến 100 mục.",
        ));
    }
    if !matches!(target_format, "cbz" | "folder") {
        return Err(AppError::validation(
            "Định dạng xuất không hợp lệ (chỉ cbz hoặc folder).",
        ));
    }
    if !(-MAX_PRIORITY..=MAX_PRIORITY).contains(&priority) {
        return Err(AppError::validation("priority phải trong khoảng -10..=10."));
    }

    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let mut outcome = QueueOutcome::default();
    let ts = now_iso();
    for chapter_id in chapter_ids {
        if !valid_id(chapter_id) {
            outcome.errors.push(QueueError {
                chapter_id: chapter_id.clone(),
                code: "VALIDATION_ERROR",
                message: "chapterId không hợp lệ.".to_string(),
            });
            continue;
        }
        let chapter_exists: bool = sqlx::query("SELECT 1 FROM chapters WHERE id = ?1 LIMIT 1")
            .bind(chapter_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
        if !chapter_exists {
            outcome.errors.push(QueueError {
                chapter_id: chapter_id.clone(),
                code: "NOT_FOUND",
                message: "Chương không tồn tại trong thư viện local — hãy lưu truyện trước."
                    .to_string(),
            });
            continue;
        }
        let sql = format!(
            "{TASK_SELECT} WHERE t.chapter_id = ?1 AND t.status IN ({}) \
             ORDER BY t.created_at DESC LIMIT 1",
            placeholders(2, NON_TERMINAL_STATES.len())
        );
        let mut query = sqlx::query_as::<_, TaskRow>(&sql).bind(chapter_id);
        for state in NON_TERMINAL_STATES {
            query = query.bind(state);
        }
        if let Some(mut task) = query.fetch_optional(&mut *tx).await? {
            // Reuse wins over duplication, but a submission with a DIFFERENT
            // format must not silently keep the old one. Only safe while no
            // worker owns the row (queued/paused/failed): an active task's
            // format is fixed until it settles.
            if matches!(task.status.as_str(), "queued" | "paused" | "failed")
                && task.target_format != target_format
            {
                let ts2 = now_iso();
                sqlx::query(
                    "UPDATE download_tasks SET target_format = ?2, revision = revision + 1, \
                     updated_at = ?3 WHERE id = ?1",
                )
                .bind(&task.id)
                .bind(target_format)
                .bind(&ts2)
                .execute(&mut *tx)
                .await?;
                task.target_format = target_format.to_string();
            }
            outcome.reused.push(task);
            continue;
        }
        let id = new_id();
        sqlx::query(
            "INSERT INTO download_tasks (id, chapter_id, status, target_format, priority, \
             completed_pages, total_pages, attempts, revision, created_at, updated_at) \
             VALUES (?1, ?2, 'queued', ?3, ?4, 0, 0, 0, 0, ?5, ?5)",
        )
        .bind(&id)
        .bind(chapter_id)
        .bind(target_format)
        .bind(priority)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
        outcome.created.push(task_by_id(&mut tx, &id).await?);
    }
    tx.commit().await?;
    Ok(outcome)
}

pub async fn get_task(pool: &SqlitePool, task_id: &str) -> Result<Option<TaskRow>, AppError> {
    crate::domain::validate_id(task_id, "taskId")?;
    let task = sqlx::query_as::<_, TaskRow>(&format!("{TASK_SELECT} WHERE t.id = ?1"))
        .bind(task_id)
        .fetch_optional(pool)
        .await?;
    Ok(task)
}

/// Guarded lifecycle transition: succeeds only from an allowed current state
/// and bumps `revision` (event ordering token). Returns None when the task
/// already moved elsewhere (someone else won the race). Entering any state
/// other than `failed` clears persisted error fields.
pub async fn transition(
    pool: &SqlitePool,
    task_id: &str,
    from: &[&str],
    to: &str,
) -> Result<Option<TaskRow>, AppError> {
    crate::domain::validate_id(task_id, "taskId")?;
    if !STATUS_STATES.contains(&to)
        || from.is_empty()
        || !from.iter().all(|state| STATUS_STATES.contains(state))
    {
        return Err(AppError::validation("Chuyển đổi trạng thái không hợp lệ."));
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let ts = now_iso();
    let clear_error = if to == "failed" {
        ""
    } else {
        ", error_code = NULL, error_message = NULL, next_retry_at = NULL"
    };
    let set_finished = if matches!(to, "completed" | "cancelled") {
        ", finished_at = ?3"
    } else {
        ""
    };
    let sql = format!(
        "UPDATE download_tasks SET status = ?2, revision = revision + 1, updated_at = ?3\
         {set_finished}{clear_error} \
         WHERE id = ?1 AND status IN ({})",
        placeholders(4, from.len())
    );
    let mut query = sqlx::query(&sql).bind(task_id).bind(to).bind(&ts);
    for state in from {
        query = query.bind(state);
    }
    let changed = query.execute(&mut *tx).await?.rows_affected();
    let result = if changed == 1 {
        Some(task_by_id(&mut tx, task_id).await?)
    } else {
        None
    };
    tx.commit().await?;
    Ok(result)
}

pub async fn set_priority(
    pool: &SqlitePool,
    task_id: &str,
    priority: i64,
) -> Result<Option<TaskRow>, AppError> {
    crate::domain::validate_id(task_id, "taskId")?;
    if !(-MAX_PRIORITY..=MAX_PRIORITY).contains(&priority) {
        return Err(AppError::validation("priority phải trong khoảng -10..=10."));
    }
    let movable = ["queued", "paused", "failed"];
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let sql = format!(
        "UPDATE download_tasks SET priority = ?2, revision = revision + 1, updated_at = ?3 \
         WHERE id = ?1 AND status IN ({})",
        placeholders(4, movable.len())
    );
    let mut query = sqlx::query(&sql)
        .bind(task_id)
        .bind(priority)
        .bind(now_iso());
    for state in &movable {
        query = query.bind(state);
    }
    let changed = query.execute(&mut *tx).await?.rows_affected();
    let result = if changed == 1 {
        Some(task_by_id(&mut tx, task_id).await?)
    } else {
        None
    };
    tx.commit().await?;
    Ok(result)
}

/// Dispatcher claim: the highest-priority, oldest eligible queued task whose
/// retry window (if any) has elapsed, atomically moved to `resolving`.
pub async fn claim_next(pool: &SqlitePool) -> Result<Option<TaskRow>, AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let ts = now_iso();
    let candidate: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM download_tasks \
         WHERE status = 'queued' AND (next_retry_at IS NULL OR next_retry_at <= ?1) \
         ORDER BY priority DESC, created_at ASC, id ASC LIMIT 1",
    )
    .bind(&ts)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((task_id,)) = candidate else {
        tx.rollback().await?;
        return Ok(None);
    };
    let changed = sqlx::query(
        "UPDATE download_tasks SET status = 'resolving', revision = revision + 1, updated_at = ?2 \
         WHERE id = ?1 AND status = 'queued'",
    )
    .bind(&task_id)
    .bind(&ts)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed != 1 {
        tx.rollback().await?;
        return Ok(None);
    }
    let task = task_by_id(&mut tx, &task_id).await?;
    tx.commit().await?;
    Ok(Some(task))
}

pub async fn chapter_context(
    pool: &SqlitePool,
    chapter_id: &str,
) -> Result<Option<ChapterContext>, AppError> {
    let ctx = sqlx::query_as::<_, ChapterContext>(
        "SELECT c.id AS chapter_id, c.title AS chapter_title, c.source_chapter_id, \
                m.id AS manga_id, m.title AS manga_title, m.source_id, m.source_manga_id \
         FROM chapters c JOIN mangas m ON m.id = c.manga_id WHERE c.id = ?1",
    )
    .bind(chapter_id)
    .fetch_optional(pool)
    .await?;
    Ok(ctx)
}

/// Idempotently materialize `total_pages` pending checkpoints for a task.
pub async fn ensure_page_rows(
    pool: &SqlitePool,
    task_id: &str,
    total_pages: usize,
) -> Result<(), AppError> {
    if total_pages == 0 || total_pages > crate::sources::http::MAX_PAGE_COUNT {
        return Err(AppError::validation(
            "Số trang nằm ngoài giới hạn cho phép.",
        ));
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let ts = now_iso();
    for index in 0..total_pages {
        sqlx::query(
            "INSERT INTO download_pages (task_id, page_index, status, created_at, updated_at) \
             VALUES (?1, ?2, 'pending', ?3, ?3) ON CONFLICT(task_id, page_index) DO NOTHING",
        )
        .bind(task_id)
        .bind(index as i64)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE download_tasks SET total_pages = ?2, updated_at = ?3 WHERE id = ?1")
        .bind(task_id)
        .bind(total_pages as i64)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    // Manifest shrank between attempts: drop ALL surplus rows so checkpoints
    // mirror exactly the current page set. Staged files of dropped rows are
    // task-owned scratch (removed with the staging dir at completion/cancel);
    // keeping them would make verification permanently fail on every retry.
    sqlx::query("DELETE FROM download_pages WHERE task_id = ?1 AND page_index >= ?2")
        .bind(task_id)
        .bind(total_pages as i64)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Record one validated staged page, then recompute the task counter from
/// page rows (never trust a stale worker/UI count).
pub async fn mark_page_stored(
    pool: &SqlitePool,
    task_id: &str,
    page_index: usize,
    storage_key: &str,
    size_bytes: i64,
    checksum: &str,
) -> Result<i64, AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let ts = now_iso();
    sqlx::query(
        "UPDATE download_pages SET status = 'stored', storage_key = ?3, size_bytes = ?4, \
         checksum = ?5, updated_at = ?6 WHERE task_id = ?1 AND page_index = ?2",
    )
    .bind(task_id)
    .bind(page_index as i64)
    .bind(storage_key)
    .bind(size_bytes)
    .bind(checksum)
    .bind(&ts)
    .execute(&mut *tx)
    .await?;
    let (done,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM download_pages WHERE task_id = ?1 AND status = 'stored'",
    )
    .bind(task_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("UPDATE download_tasks SET completed_pages = ?2, updated_at = ?3 WHERE id = ?1")
        .bind(task_id)
        .bind(done)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(done)
}

pub async fn list_pages(pool: &SqlitePool, task_id: &str) -> Result<Vec<PageRow>, AppError> {
    let rows = sqlx::query_as::<_, PageRow>(
        "SELECT page_index, storage_key, status, size_bytes, checksum FROM download_pages \
         WHERE task_id = ?1 ORDER BY page_index ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Requeue a failed task after an explicit retry (attempts are kept so the
/// backoff schedule survives; the transition clears the error fields).
pub async fn retry_failed(pool: &SqlitePool, task_id: &str) -> Result<Option<TaskRow>, AppError> {
    transition(pool, task_id, &["failed"], "queued").await
}

/// Automatic-retry requeue: back to `queued` from any active state with a
/// new attempt counted and the retry window persisted. The informative
/// error stays visible on the queued card until the next success clears it.
pub async fn requeue_with_backoff(
    pool: &SqlitePool,
    task_id: &str,
    next_retry_at: Option<&str>,
    error_code: &str,
    error_message: &str,
) -> Result<Option<TaskRow>, AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let ts = now_iso();
    let from = ACTIVE_STATES;
    let sql = format!(
        "UPDATE download_tasks SET status = 'queued', attempts = attempts + 1, \
         next_retry_at = ?2, error_code = ?3, error_message = ?4, \
         revision = revision + 1, updated_at = ?5 \
         WHERE id = ?1 AND status IN ({})",
        placeholders(6, from.len())
    );
    let mut query = sqlx::query(&sql)
        .bind(task_id)
        .bind(next_retry_at)
        .bind(error_code)
        .bind(error_message)
        .bind(&ts);
    for state in from {
        query = query.bind(state);
    }
    let changed = query.execute(&mut *tx).await?.rows_affected();
    let result = if changed == 1 {
        Some(task_by_id(&mut tx, task_id).await?)
    } else {
        None
    };
    tx.commit().await?;
    Ok(result)
}

/// Persist a safe error pair and move the task to `failed` from any
/// non-terminal state. Terminal states are never rewritten.
pub async fn record_failure(
    pool: &SqlitePool,
    task_id: &str,
    error_code: &str,
    error_message: &str,
) -> Result<Option<TaskRow>, AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let ts = now_iso();
    sqlx::query(
        "UPDATE download_tasks SET error_code = ?2, error_message = ?3 \
         WHERE id = ?1 AND status NOT IN ('completed', 'cancelled')",
    )
    .bind(task_id)
    .bind(error_code)
    .bind(error_message)
    .execute(&mut *tx)
    .await?;
    let sql = format!(
        "UPDATE download_tasks SET status = 'failed', revision = revision + 1, updated_at = ?2 \
         WHERE id = ?1 AND status IN ({})",
        placeholders(3, NON_TERMINAL_STATES.len())
    );
    let mut query = sqlx::query(&sql).bind(task_id).bind(&ts);
    for state in NON_TERMINAL_STATES {
        query = query.bind(state);
    }
    let changed = query.execute(&mut *tx).await?.rows_affected();
    let result = if changed == 1 {
        Some(task_by_id(&mut tx, task_id).await?)
    } else {
        None
    };
    tx.commit().await?;
    Ok(result)
}

/// Commit a verified artifact: task → completed and the chapter's local
/// paths in one transaction (the filesystem artifact was verified before).
pub async fn publish_completed(
    pool: &SqlitePool,
    task_id: &str,
    output_relpath: &str,
    output_format: &str,
    local_size_bytes: Option<i64>,
) -> Result<Option<TaskRow>, AppError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let (chapter_id,): (String,) =
        sqlx::query_as("SELECT chapter_id FROM download_tasks WHERE id = ?1")
            .bind(task_id)
            .fetch_one(&mut *tx)
            .await?;
    let ts = now_iso();
    let changed = sqlx::query(
        "UPDATE download_tasks SET status = 'completed', revision = revision + 1, finished_at = ?3, \
         updated_at = ?3, output_relpath = ?2, error_code = NULL, error_message = NULL, \
         next_retry_at = NULL \
         WHERE id = ?1 AND status = 'exporting'",
    )
    .bind(task_id)
    .bind(output_relpath)
    .bind(&ts)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if changed != 1 {
        tx.rollback().await?;
        return Ok(None);
    }
    sqlx::query(
        "UPDATE chapters SET local_path = ?2, output_format = ?3, local_size_bytes = ?5, updated_at = ?4 WHERE id = ?1",
    )
    .bind(&chapter_id)
    .bind(output_relpath)
    .bind(output_format)
    .bind(&ts)
    .bind(local_size_bytes)
    .execute(&mut *tx)
    .await?;
    let task = task_by_id(&mut tx, task_id).await?;
    tx.commit().await?;
    Ok(Some(task))
}

pub async fn tasks_in_status(pool: &SqlitePool, status: &str) -> Result<Vec<TaskRow>, AppError> {
    let rows = sqlx::query_as::<_, TaskRow>(&format!(
        "{TASK_SELECT} WHERE t.status = ?1 ORDER BY t.created_at ASC"
    ))
    .bind(status)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Startup reconciliation of persisted lifecycle (crash-safe intent):
/// queued/paused survive; `pausing` honors the user's pause intent; active
/// worker states return to `queued` (checkpoints remain — resume is
/// page-level). Returns affected rows for logging.
pub async fn recover_stale_tasks(pool: &SqlitePool) -> Result<u64, AppError> {
    let ts = now_iso();
    let sql = format!(
        "UPDATE download_tasks SET status = 'queued', revision = revision + 1, updated_at = ?1 \
         WHERE status IN ({})",
        placeholders(2, ACTIVE_STATES.len())
    );
    let mut query = sqlx::query(&sql).bind(&ts);
    for state in ACTIVE_STATES {
        query = query.bind(state);
    }
    let mut changed = query.execute(pool).await?.rows_affected();
    changed += sqlx::query(
        "UPDATE download_tasks SET status = 'paused', revision = revision + 1, updated_at = ?1 \
         WHERE status = 'pausing'",
    )
    .bind(&ts)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed)
}

/// Test-only: attach a verified artifact to a chapter without a task.
#[cfg(test)]
pub async fn attach_artifact_for_test(
    pool: &SqlitePool,
    chapter_id: &str,
    relpath: &str,
    format: &str,
) {
    sqlx::query(
        "UPDATE chapters SET local_path = ?2, output_format = ?3, updated_at = ?4 WHERE id = ?1",
    )
    .bind(chapter_id)
    .bind(relpath)
    .bind(format)
    .bind(now_iso())
    .execute(pool)
    .await
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::manga_repo::{ensure_source, upsert_chapter, upsert_manga};

    async fn seed_chapter(pool: &sqlx::SqlitePool, chapter_slug: &str) -> String {
        ensure_source(pool, "src-a", "Source A").await.unwrap();
        let (manga_id, _) = upsert_manga(pool, "src-a", "m1", "T", None, None, "ongoing")
            .await
            .unwrap();
        let (chapter_id, _) = upsert_chapter(pool, &manga_id, chapter_slug, "C", None, None)
            .await
            .unwrap();
        chapter_id
    }

    #[tokio::test]
    async fn t06_04_queue_batch_creates_bounded_tasks_and_rejects_oversize() {
        let (_dir, pool) = crate::db::test_pool().await;
        let one = seed_chapter(&pool, "c1").await;
        let two = seed_chapter(&pool, "c2").await;
        let outcome = queue_batch(&pool, &[one.clone(), two.clone()], "cbz", 0)
            .await
            .unwrap();
        assert_eq!(outcome.created.len(), 2);
        assert_eq!(outcome.reused.len(), 0);

        // Oversized batch is rejected before any write.
        let huge = vec![one.clone(); MAX_BATCH_SIZE + 1];
        assert!(queue_batch(&pool, &huge, "cbz", 0).await.is_err());
    }

    #[tokio::test]
    async fn t06_05_concurrent_identical_submit_dedups_to_one_active_task() {
        let (_dir, pool) = crate::db::test_pool().await;
        let chapter = seed_chapter(&pool, "c1").await;
        let first = queue_batch(&pool, std::slice::from_ref(&chapter), "cbz", 0)
            .await
            .unwrap();
        assert_eq!(first.created.len(), 1);
        let second = queue_batch(&pool, std::slice::from_ref(&chapter), "cbz", 0)
            .await
            .unwrap();
        assert!(
            second.created.is_empty(),
            "must not duplicate an active task"
        );
        assert_eq!(second.reused.len(), 1);
        assert_eq!(second.reused[0].id, first.created[0].id);
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM download_tasks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn t10_reuse_of_idle_task_adopts_new_target_format() {
        // Regression (whole-project review round 2): re-submitting a queued
        // task with a different format silently kept the OLD format.
        let (_dir, pool) = crate::db::test_pool().await;
        let chapter = seed_chapter(&pool, "c1").await;
        let first = queue_batch(&pool, std::slice::from_ref(&chapter), "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        let again = queue_batch(&pool, std::slice::from_ref(&chapter), "folder", 0)
            .await
            .unwrap();
        assert_eq!(again.reused.len(), 1);
        assert_eq!(again.reused[0].id, first.id, "still deduped");
        assert_eq!(
            again.reused[0].target_format, "folder",
            "idle (queued) reuse must honor the newly requested format"
        );

        // An ACTIVE task's format is fixed until it settles (its worker
        // already captured it; rewriting would desync export vs DB record).
        transition(&pool, &first.id, &["queued"], "downloading")
            .await
            .unwrap();
        let busy = queue_batch(&pool, std::slice::from_ref(&chapter), "cbz", 0)
            .await
            .unwrap();
        assert_eq!(busy.reused[0].target_format, "folder");
    }

    #[tokio::test]
    async fn unknown_chapter_and_bad_format_rejected_per_item() {
        let (_dir, pool) = crate::db::test_pool().await;
        let outcome = queue_batch(&pool, &["does-not-exist".to_string()], "cbz", 0)
            .await
            .unwrap();
        assert_eq!(outcome.errors.len(), 1);
        assert_eq!(outcome.errors[0].code, "NOT_FOUND");
        let valid = seed_chapter(&pool, "c1").await;
        assert!(queue_batch(&pool, &[valid], "pdf", 0).await.is_err());
    }

    #[tokio::test]
    async fn illegal_state_transitions_are_no_ops() {
        let (_dir, pool) = crate::db::test_pool().await;
        let chapter = seed_chapter(&pool, "c1").await;
        let task = queue_batch(&pool, &[chapter], "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();

        // Non-status values are rejected outright.
        assert!(transition(&pool, &task.id, &["queued"], "not-a-state")
            .await
            .is_err());
        // A move whose `from` guard does not match the current state is a
        // no-op returning None (the edge table lives in the service layer).
        assert!(transition(&pool, &task.id, &["paused"], "queued")
            .await
            .unwrap()
            .is_none());
        let moved = transition(&pool, &task.id, &["queued"], "completed")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(moved.status, "completed");
        assert_eq!(moved.finished_at, Some(moved.updated_at.clone()));
        // A second attempt from queued fails the guard (state already advanced).
        assert!(transition(&pool, &task.id, &["queued"], "paused")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn completed_output_survives_and_chapter_gets_local_path() {
        let (_dir, pool) = crate::db::test_pool().await;
        let chapter = seed_chapter(&pool, "c1").await;
        let task = queue_batch(&pool, std::slice::from_ref(&chapter), "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        for status in ["resolving", "downloading", "verifying", "exporting"] {
            let from = if status == "resolving" {
                "queued"
            } else {
                previous(status)
            };
            let moved = transition(&pool, &task.id, &[from], status).await.unwrap();
            assert!(moved.is_some(), "advance to {status}");
        }
        ensure_page_rows(&pool, &task.id, 3).await.unwrap();
        mark_page_stored(&pool, &task.id, 0, "000001.png", 72, "abc")
            .await
            .unwrap();
        let done = mark_page_stored(&pool, &task.id, 1, "000002.png", 72, "def")
            .await
            .unwrap();
        assert_eq!(done, 2, "counter recomputed from page rows");

        let published = publish_completed(&pool, &task.id, "manga-x/c1-t.cbz", "cbz", Some(214))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(published.status, "completed");
        assert_eq!(published.completed_pages, 2);
        let local: (Option<String>,) =
            sqlx::query_as("SELECT local_path FROM chapters WHERE id = ?1")
                .bind(&chapter)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(local.0.as_deref(), Some("manga-x/c1-t.cbz"));
    }

    #[tokio::test]
    async fn recovery_resets_active_states_but_keeps_user_pause() {
        let (_dir, pool) = crate::db::test_pool().await;
        let active = seed_chapter(&pool, "c1").await;
        let paused = seed_chapter(&pool, "c2").await;
        let a = queue_batch(&pool, &[active], "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        let b = queue_batch(&pool, &[paused], "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        transition(&pool, &a.id, &["queued"], "downloading")
            .await
            .unwrap();
        transition(&pool, &b.id, &["queued"], "pausing")
            .await
            .unwrap();

        let affected = recover_stale_tasks(&pool).await.unwrap();
        assert_eq!(affected, 2);
        assert_eq!(
            get_task(&pool, &a.id).await.unwrap().unwrap().status,
            "queued"
        );
        assert_eq!(
            get_task(&pool, &b.id).await.unwrap().unwrap().status,
            "paused"
        );
    }

    #[tokio::test]
    async fn shrinking_manifest_prunes_surplus_pages_of_any_state() {
        let (_dir, pool) = crate::db::test_pool().await;
        let chapter = seed_chapter(&pool, "c1").await;
        let task = queue_batch(&pool, &[chapter], "cbz", 0)
            .await
            .unwrap()
            .created[0]
            .clone();
        ensure_page_rows(&pool, &task.id, 5).await.unwrap();
        mark_page_stored(&pool, &task.id, 4, "000004.png", 10, "cs")
            .await
            .unwrap();
        ensure_page_rows(&pool, &task.id, 3).await.unwrap();
        let rows = list_pages(&pool, &task.id).await.unwrap();
        // ALL surplus rows (pending AND stored) are dropped: with positional
        // identity a shrunk manifest no longer includes page 4, and a
        // leftover stored row would fail verification on every retry.
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| r.page_index < 3));
        let total = get_task(&pool, &task.id).await.unwrap().unwrap();
        assert_eq!(total.total_pages, 3);
    }

    fn previous(status: &str) -> &'static str {
        match status {
            "downloading" => "resolving",
            "verifying" => "downloading",
            "exporting" => "verifying",
            _ => "queued",
        }
    }
}
