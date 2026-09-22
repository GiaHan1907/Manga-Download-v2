-- Phase 07: library & offline reader. Forward-only, plain ALTERs (no rebuild,
-- no new tables — chapters/mangas/reading_progress are the single store of
-- truth; nothing here duplicates Phase 04/05/06 concepts).

ALTER TABLE mangas ADD COLUMN last_read_at TEXT;
ALTER TABLE chapters ADD COLUMN local_size_bytes INTEGER;
ALTER TABLE reading_progress ADD COLUMN scroll_offset REAL;
ALTER TABLE reading_progress ADD COLUMN page_total INTEGER;

-- Sort by recently read (library "Continue reading").
CREATE INDEX idx_mangas_last_read ON mangas(last_read_at);
