-- CPN account security flags (0007_account_security_flags)
-- Panel accounts live in JSON (`panel-bootstrap.json`, `accounts/*.json`).
-- This migration records the flag contract in the SQL ledger and runs a JSON hook.
--
-- Flags (serde defaults false when absent on older files):
--   must_change_password  INTEGER/boolean  require password change on next login
--   totp_required         INTEGER/boolean  require TOTP or passkey before full panel use
--
-- Applied via panel_migrate hook `ensure_account_security_flags_migrated`.

CREATE TABLE IF NOT EXISTS schema_migrations (
    id TEXT PRIMARY KEY NOT NULL,
    applied_at_unix INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS panel_account_security_meta (
    id TEXT PRIMARY KEY NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    updated_at_unix INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO panel_account_security_meta (id, note, updated_at_unix)
VALUES (
    'flags_v1',
    'must_change_password + totp_required on panel-bootstrap / accounts JSON',
    0
);
