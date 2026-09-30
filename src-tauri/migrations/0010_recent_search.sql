-- Discover Phase 2: the user's own recent search queries. Only the query text
-- and a timestamp are stored — no results, no source ids, nothing that would
-- let a search be reconstructed beyond what the user typed. Deduplicated by
-- the query itself so re-running a search refreshes its position instead of
-- adding a row; the repo prunes to the newest few entries on every write.

CREATE TABLE recent_searches (
    query TEXT NOT NULL PRIMARY KEY,
    searched_at TEXT NOT NULL
);

CREATE INDEX idx_recent_searches_at ON recent_searches(searched_at DESC);
