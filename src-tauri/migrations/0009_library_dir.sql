-- Clean download paths: library/<title-slug>/chapter-<number>.cbz. The slug
-- is CLAIMED once per manga so a later title edit never orphans the folder,
-- and two different manga whose titles slug identically cannot share (and
-- overwrite/adopt) one directory. Existing rows keep their stored relpaths —
-- every consumer reads paths from the DB, never re-derives them.

ALTER TABLE mangas ADD COLUMN library_dir TEXT;
