-- cpn_panel: panel state mirrored from /var/lib/cpn.
-- Idempotent (IF NOT EXISTS). Applied by `cpn panel-db migrate` and on upgrade.
-- DML only for the panel account; DDL runs as the administrative account.

CREATE TABLE IF NOT EXISTS panel_documents (
  namespace VARCHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  doc_key VARCHAR(255) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL,
  -- Plain bytes, or a `cpn1:` envelope (application-level AES-256-GCM) when encrypted = 1.
  body LONGBLOB NOT NULL,
  encrypted TINYINT(1) NOT NULL DEFAULT 0,
  -- SHA-256 of the plain bytes (plain rows) or a keyed HMAC (encrypted rows).
  content_sha256 CHAR(64) CHARACTER SET ascii NOT NULL,
  size_bytes INT UNSIGNED NOT NULL,
  file_mode SMALLINT UNSIGNED NOT NULL DEFAULT 384,
  phase TINYINT UNSIGNED NOT NULL DEFAULT 3,
  revision BIGINT UNSIGNED NOT NULL DEFAULT 1,
  imported_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (namespace, doc_key),
  KEY idx_panel_documents_phase (phase)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS panel_import_runs (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  context VARCHAR(64) CHARACTER SET ascii NOT NULL,
  started_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  finished_at TIMESTAMP NULL DEFAULT NULL,
  scanned INT UNSIGNED NOT NULL DEFAULT 0,
  imported INT UNSIGNED NOT NULL DEFAULT 0,
  unchanged INT UNSIGNED NOT NULL DEFAULT 0,
  skipped INT UNSIGNED NOT NULL DEFAULT 0,
  failed INT UNSIGNED NOT NULL DEFAULT 0,
  ok TINYINT(1) NOT NULL DEFAULT 0
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS panel_meta (
  k VARCHAR(64) CHARACTER SET ascii NOT NULL PRIMARY KEY,
  v VARCHAR(512) NOT NULL,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
