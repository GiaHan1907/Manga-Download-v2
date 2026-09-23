-- Feature: third-party image hosts for user-registered sources.
-- Some WordPress manga sites serve chapter images from a separate CDN
-- domain. The transport allowlist stays fail-closed: an adapter may ONLY
-- contact its base host plus the exact hosts recorded here (user-explicit,
-- validated, capped, no IPs/wildcards). NULL = base host only (old rows).

ALTER TABLE sources ADD COLUMN image_hosts TEXT;
