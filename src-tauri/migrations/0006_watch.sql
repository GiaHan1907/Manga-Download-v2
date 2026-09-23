-- Feature plan F1: new-chapter watch. Forward-only ALTERs + one table.
-- Semantics:
--   mangas.watch_enabled  NULL = follow the global settings watchEnabled;
--                         0/1  = explicit per-manga opt-out/in (overrides global).
--   mangas.watched_at     last SUCCESSFUL detail poll (failures leave it stale
--                         and bump watch_fail_count, which backs the poll off
--                         geometrically — see watch_service).
--   mangas.watch_baseline_key  source_chapter_id of the newest chapter at the
--                         previous poll. NULL on the first poll: chapters
--                         discovered then are recorded but NOT alerted
--                         (enrolling in an existing series is not "new").
-- chapter_alerts rows are created only for chapters that appear AFTER a
-- baseline exists; the UI acknowledges (deletes) them per manga.

ALTER TABLE mangas ADD COLUMN watch_enabled INTEGER CHECK (watch_enabled IS NULL OR watch_enabled IN (0, 1));
ALTER TABLE mangas ADD COLUMN watched_at TEXT;
ALTER TABLE mangas ADD COLUMN watch_fail_count INTEGER NOT NULL DEFAULT 0 CHECK (watch_fail_count >= 0);
ALTER TABLE mangas ADD COLUMN watch_baseline_key TEXT;

CREATE TABLE chapter_alerts (
    manga_id TEXT NOT NULL REFERENCES mangas(id) ON DELETE CASCADE,
    chapter_id TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    detected_at TEXT NOT NULL,
    PRIMARY KEY (manga_id, chapter_id)
);

CREATE INDEX idx_alerts_manga ON chapter_alerts(manga_id);
