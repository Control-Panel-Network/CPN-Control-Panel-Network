-- Mail onboarding + SSL defaults schema bump (0006_mail_onboarding_ssl_defaults)
-- JSON stores: mail-onboarding.json, ssl-defaults.json (default Let's Encrypt).

CREATE TABLE IF NOT EXISTS mail_onboarding_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version INTEGER NOT NULL DEFAULT 1,
    mail_mode TEXT NOT NULL DEFAULT 'local',
    skip_rdns INTEGER NOT NULL DEFAULT 0,
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS ssl_defaults_meta (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version INTEGER NOT NULL DEFAULT 1,
    default_provider TEXT NOT NULL DEFAULT 'letsencrypt',
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);
