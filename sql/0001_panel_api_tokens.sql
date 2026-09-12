-- CPN panel API tokens (0001_panel_api_tokens)
-- Applied via panel_migrate (JSON store + optional panel.db)

CREATE TABLE IF NOT EXISTS schema_migrations (
    id TEXT PRIMARY KEY NOT NULL,
    applied_at_unix INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS panel_api_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    username TEXT NOT NULL,
    scopes TEXT NOT NULL DEFAULT 'read',
    token_hash TEXT NOT NULL,
    created_at_unix INTEGER NOT NULL DEFAULT 0,
    last_used_at_unix INTEGER,
    revoked_at_unix INTEGER
);

CREATE INDEX IF NOT EXISTS idx_panel_api_tokens_username ON panel_api_tokens (username);
CREATE INDEX IF NOT EXISTS idx_panel_api_tokens_hash ON panel_api_tokens (token_hash);
CREATE INDEX IF NOT EXISTS idx_panel_api_tokens_revoked ON panel_api_tokens (revoked_at_unix);
