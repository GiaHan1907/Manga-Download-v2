-- Phase 05: sources become user-manageable registry rows.
-- `adapter` selects the generic implementation shipped with the app
-- ('mangadex' built-in REST, 'madara' generic WordPress/Madara sites).
-- base_url is required for 'madara' rows (validated by the Rust service, never by SQL).

ALTER TABLE sources ADD COLUMN adapter TEXT NOT NULL DEFAULT 'mangadex';
ALTER TABLE sources ADD COLUMN base_url TEXT;
