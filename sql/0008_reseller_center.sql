-- CPN Reseller Center (0008_reseller_center)
-- Hierarchy, multi-tenant quotas, and branding live in JSON:
--   /var/lib/cpn/resellers.json
-- Account fields (serde defaults when absent on older files):
--   role              TEXT  empty/"user" or "reseller" (bootstrap admin stays owner)
--   parent_reseller   TEXT  username of parent reseller when jailed under one
--
-- Applied via panel_migrate hook `ensure_reseller_store_migrated`.

CREATE TABLE IF NOT EXISTS schema_migrations (
    id TEXT PRIMARY KEY NOT NULL,
    applied_at_unix INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS panel_reseller_meta (
    id TEXT PRIMARY KEY NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO panel_reseller_meta (id, note, updated_at_unix)
VALUES (
    'reseller_v1',
    'resellers.json + account role/parent_reseller hierarchy',
    0
);
