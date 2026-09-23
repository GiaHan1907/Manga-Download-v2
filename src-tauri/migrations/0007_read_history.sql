-- Feature plan F4: per-chapter read history. Append-style table maintained
-- inside the SAME save_progress transaction — a chapter is marked read when
-- the reader reaches its last pages (pageTotal known). UPSERT keeps one row
-- per (manga, chapter) with the most recent read timestamp (no spam rows).
-- User-deletable per manga (metadata only — never touches files/progress).

CREATE TABLE read_history (
    manga_id TEXT NOT NULL REFERENCES mangas(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    read_at TEXT NOT NULL,
    PRIMARY KEY (manga_id, chapter_id)
);

CREATE INDEX idx_history_read_at ON read_history(read_at DESC);
