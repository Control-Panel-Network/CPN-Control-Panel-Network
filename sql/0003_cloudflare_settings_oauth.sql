-- Cloudflare settings OAuth schema bump (0003_cloudflare_settings_oauth)
-- Documents auth_type oauth in cloudflare.json (schema_version 2).

CREATE TABLE IF NOT EXISTS cloudflare_settings_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version INTEGER NOT NULL DEFAULT 2,
    auth_type TEXT NOT NULL DEFAULT 'api_token',
    oauth_enabled INTEGER NOT NULL DEFAULT 0,
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);
