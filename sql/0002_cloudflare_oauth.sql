-- Cloudflare OAuth client + link stores (0002_cloudflare_oauth)
-- JSON mirrors live under /var/lib/cpn/; panel.db optional.

CREATE TABLE IF NOT EXISTS cloudflare_oauth_client (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    client_id TEXT NOT NULL DEFAULT '',
    client_secret TEXT NOT NULL DEFAULT '',
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS cloudflare_oauth_link (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    access_token TEXT NOT NULL DEFAULT '',
    refresh_token TEXT NOT NULL DEFAULT '',
    token_type TEXT NOT NULL DEFAULT 'bearer',
    expires_at_unix INTEGER,
    scopes TEXT NOT NULL DEFAULT '',
    connected_at_unix INTEGER NOT NULL DEFAULT 0,
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);
