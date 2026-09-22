-- Phase 06: download engine. Forward-only, no data reset.
-- download_tasks is rebuilt ONLY to widen the status CHECK (persisted active
-- lifecycle states) and add retry/reconciliation columns. target_format keeps
-- accepting legacy 'pdf' rows; the engine rejects new pdf submissions in Rust.
--
-- Repaired execution (applied at runtime from src/db/mod.rs; the ORIGINAL
-- migrations/0004_download_engine.sql checksum stays authoritative):
-- sqlx runs each migration inside one transaction and SQLite IGNORES
-- `PRAGMA foreign_keys = OFF` there; `defer_foreign_keys` defers violation
-- checks but NOT ON DELETE CASCADE actions, and modern SQLite rewrites child
-- FK references on RENAME regardless of `legacy_alter_table` (all three
-- behaviors proven by the t10 tests). So the original `DROP TABLE
-- download_tasks` would cascade-delete every download_pages checkpoint row on
-- a 2->4 upgrade. The repair follows the official SQLite rebuild recipe under
-- FK enforcement: rebuild the child FIRST (pointing at the new parent by its
-- temporary name), drop both old tables, then rename the new pair into place
-- — RENAME repoints the child's FK to the final parent name automatically.
-- No statement ever triggers a cascade against live checkpoint rows.

CREATE TABLE download_tasks__new (
    id TEXT PRIMARY KEY NOT NULL,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN (
        'queued', 'resolving', 'downloading', 'pausing', 'paused',
        'verifying', 'exporting', 'completed', 'failed', 'cancelled'
    )),
    target_format TEXT NOT NULL DEFAULT 'cbz' CHECK (target_format IN ('cbz', 'folder', 'pdf')),
    priority INTEGER NOT NULL DEFAULT 0,
    completed_pages INTEGER NOT NULL DEFAULT 0 CHECK (completed_pages >= 0),
    total_pages INTEGER NOT NULL DEFAULT 0 CHECK (total_pages >= 0),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_retry_at TEXT,
    error_code TEXT,
    error_message TEXT,
    revision INTEGER NOT NULL DEFAULT 0,
    finished_at TEXT,
    output_relpath TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO download_tasks__new (
    id, chapter_id, status, target_format, priority, completed_pages,
    total_pages, attempts, revision, created_at, updated_at
)
SELECT
    id, chapter_id, status, target_format, priority, completed_pages,
    total_pages, 0, 0, created_at, updated_at
FROM download_tasks;

-- Rebuild the child against the NEW parent (temporary name) before the old
-- parent is dropped, so the drop can never cascade into checkpoint rows.
CREATE TABLE download_pages__new (
    task_id TEXT NOT NULL REFERENCES download_tasks__new(id) ON DELETE CASCADE,
    page_index INTEGER NOT NULL CHECK (page_index >= 0),
    storage_key TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'stored', 'failed')),
    size_bytes INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
    checksum TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (task_id, page_index)
);

INSERT INTO download_pages__new (
    task_id, page_index, storage_key, status, size_bytes, checksum, created_at, updated_at
)
SELECT
    task_id, page_index, storage_key, status, size_bytes, checksum, created_at, updated_at
FROM download_pages;

DROP TABLE download_pages;
DROP TABLE download_tasks;

-- Modern RENAME repoints download_pages__new's FK to the final parent name.
ALTER TABLE download_tasks__new RENAME TO download_tasks;
ALTER TABLE download_pages__new RENAME TO download_pages;

CREATE INDEX idx_tasks_status_created ON download_tasks(status, created_at);
CREATE INDEX idx_tasks_chapter ON download_tasks(chapter_id);
-- Dispatcher ordering: fair FIFO inside a priority band.
CREATE INDEX idx_tasks_dispatch ON download_tasks(status, priority DESC, created_at);

-- Verified final artifact of a chapter (relative to the app library root).
-- Populated only after export verification; NULL means "not available offline".
ALTER TABLE chapters ADD COLUMN local_path TEXT;
ALTER TABLE chapters ADD COLUMN output_format TEXT;
