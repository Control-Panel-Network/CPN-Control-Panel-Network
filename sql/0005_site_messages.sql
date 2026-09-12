-- CPN site message defaults (0005_site_messages)
-- Primary store remains JSON at $CPN_DATA_DIR/site-messages.json (mode 600).
-- Optional sqlite mirror for operators who use panel.db.

CREATE TABLE IF NOT EXISTS schema_migrations (
    id TEXT PRIMARY KEY NOT NULL,
    applied_at_unix INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS site_message_defaults (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    suspend_message_html TEXT NOT NULL DEFAULT '',
    site_ready_html TEXT NOT NULL DEFAULT '',
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO site_message_defaults (id, suspend_message_html, site_ready_html, updated_at_unix)
VALUES (1, '', '', 0);
