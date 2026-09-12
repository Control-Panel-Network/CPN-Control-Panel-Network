-- CPN WordPress sites registry (0004_wordpress_sites)
-- Applied via panel_migrate (JSON store + optional panel.db)

CREATE TABLE IF NOT EXISTS schema_migrations (
    id TEXT PRIMARY KEY NOT NULL,
    applied_at_unix INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS wordpress_sites (
    id TEXT PRIMARY KEY NOT NULL,
    domain TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL DEFAULT '',
    docroot TEXT NOT NULL,
    site_url TEXT NOT NULL DEFAULT '',
    admin_user TEXT NOT NULL DEFAULT '',
    db_name TEXT NOT NULL DEFAULT '',
    db_user TEXT NOT NULL DEFAULT '',
    owner TEXT NOT NULL DEFAULT '',
    wp_version TEXT NOT NULL DEFAULT '',
    php_version TEXT NOT NULL DEFAULT '',
    theme TEXT NOT NULL DEFAULT '',
    plugin_count INTEGER NOT NULL DEFAULT 0,
    search_indexing INTEGER NOT NULL DEFAULT 1,
    debugging INTEGER NOT NULL DEFAULT 0,
    maintenance INTEGER NOT NULL DEFAULT 0,
    password_protection INTEGER NOT NULL DEFAULT 0,
    created_at_unix INTEGER NOT NULL DEFAULT 0,
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_wordpress_sites_domain ON wordpress_sites (domain);
CREATE INDEX IF NOT EXISTS idx_wordpress_sites_owner ON wordpress_sites (owner);
