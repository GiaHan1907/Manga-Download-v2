-- Phase 04 domain schema. Forward-only; existing app_settings untouched.
-- Conventions (Phase 01): IDs are TEXT UUID v4 assigned by Rust; timestamps are
-- UTC ISO-8601 TEXT; foreign keys enforced per-connection via pragma.

CREATE TABLE sources (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE mangas (
    id TEXT PRIMARY KEY NOT NULL,
    source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE RESTRICT,
    source_manga_id TEXT NOT NULL,
    title TEXT NOT NULL,
    author TEXT,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'unknown' CHECK (status IN ('ongoing', 'completed', 'unknown')),
    cover_key TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (source_id, source_manga_id)
);

CREATE TABLE chapters (
    id TEXT PRIMARY KEY NOT NULL,
    manga_id TEXT NOT NULL REFERENCES mangas(id) ON DELETE RESTRICT,
    source_chapter_id TEXT NOT NULL,
    title TEXT NOT NULL,
    number REAL,
    page_count INTEGER CHECK (page_count IS NULL OR page_count >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (manga_id, source_chapter_id)
);

-- Schema only in Phase 04 (checkpoint design for Phase 06). Phase 04 never
-- writes rows that claim a real transfer happened.
CREATE TABLE download_tasks (
    id TEXT PRIMARY KEY NOT NULL,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN ('queued', 'paused', 'failed', 'completed', 'cancelled')),
    target_format TEXT NOT NULL DEFAULT 'cbz' CHECK (target_format IN ('cbz', 'folder', 'pdf')),
    priority INTEGER NOT NULL DEFAULT 0,
    completed_pages INTEGER NOT NULL DEFAULT 0 CHECK (completed_pages >= 0),
    total_pages INTEGER NOT NULL DEFAULT 0 CHECK (total_pages >= 0),
    error_code TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE download_pages (
    task_id TEXT NOT NULL REFERENCES download_tasks(id) ON DELETE CASCADE,
    page_index INTEGER NOT NULL CHECK (page_index >= 0),
    storage_key TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'stored', 'failed')),
    size_bytes INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
    checksum TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (task_id, page_index)
);

-- Ownership (chapter belongs to manga) is enforced in the reading service
-- transaction, because a plain FK cannot express the pair constraint.
CREATE TABLE reading_progress (
    manga_id TEXT PRIMARY KEY NOT NULL REFERENCES mangas(id) ON DELETE RESTRICT,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE RESTRICT,
    page_index INTEGER NOT NULL CHECK (page_index >= 0),
    updated_at TEXT NOT NULL
);

CREATE TABLE favorites (
    manga_id TEXT PRIMARY KEY NOT NULL REFERENCES mangas(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_mangas_updated_at ON mangas(updated_at);
CREATE INDEX idx_mangas_title ON mangas(title COLLATE NOCASE);
CREATE INDEX idx_chapters_manga ON chapters(manga_id);
CREATE INDEX idx_tasks_status_created ON download_tasks(status, created_at);
CREATE INDEX idx_tasks_chapter ON download_tasks(chapter_id);
CREATE INDEX idx_progress_updated ON reading_progress(updated_at);
CREATE INDEX idx_favorites_created ON favorites(created_at);
