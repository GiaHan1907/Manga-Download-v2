-- Phase 06: download engine. Forward-only, no data reset.
-- download_tasks is rebuilt ONLY to widen the status CHECK (persisted active
-- lifecycle states) and add retry/reconciliation columns. target_format keeps
-- accepting legacy 'pdf' rows; the engine rejects new pdf submissions in Rust.
-- Procedure follows the official SQLite "other schema changes" recipe with
-- foreign_keys off for the swap; download_pages keeps referencing the table
-- by name and resolves to the rebuilt one.

PRAGMA foreign_keys = OFF;

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

DROP TABLE download_tasks;
ALTER TABLE download_tasks__new RENAME TO download_tasks;

CREATE INDEX idx_tasks_status_created ON download_tasks(status, created_at);
CREATE INDEX idx_tasks_chapter ON download_tasks(chapter_id);
-- Dispatcher ordering: fair FIFO inside a priority band.
CREATE INDEX idx_tasks_dispatch ON download_tasks(status, priority DESC, created_at);

-- Verified final artifact of a chapter (relative to the app library root).
-- Populated only after export verification; NULL means "not available offline".
ALTER TABLE chapters ADD COLUMN local_path TEXT;
ALTER TABLE chapters ADD COLUMN output_format TEXT;

PRAGMA foreign_keys = ON;
