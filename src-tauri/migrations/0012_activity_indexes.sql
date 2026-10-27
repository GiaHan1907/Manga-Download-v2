-- "Hoạt động gần đây" reads two feeds ordered by time: watcher alerts by when
-- they were detected, and download tasks by the moment they settled (finished,
-- or last touched for a failure). Both sorted without an index on that column.
CREATE INDEX IF NOT EXISTS idx_alerts_detected ON chapter_alerts(detected_at DESC);

-- Expression index: it matches the ORDER BY in activity_service exactly.
CREATE INDEX IF NOT EXISTS idx_tasks_activity ON download_tasks(COALESCE(finished_at, updated_at) DESC);
