-- Discover Phase 2 follow-up (Library redesign mockup): the genres a story's
-- detail page published, persisted so the Library can filter and label without
-- re-fetching every source. Written in the SAME transaction that saves the
-- manga, and rewritten wholesale on every save so a refresh keeps the set in
-- sync. Pure metadata: a leaf table with no files behind it, so it cascades
-- with the manga.

CREATE TABLE manga_genres (
    manga_id TEXT NOT NULL REFERENCES mangas(id) ON DELETE CASCADE,
    genre TEXT NOT NULL,
    PRIMARY KEY (manga_id, genre)
);

-- The Library's genre filter and the per-genre counts both look up by genre.
CREATE INDEX idx_manga_genres_genre ON manga_genres(genre, manga_id);
