-- cpn_mail: virtual mailbox passdb/userdb for Dovecot (replaces a leftover classic `e_users`).
-- Dovecot reads only the `mailbox_auth` view through a SELECT-only account.

CREATE TABLE IF NOT EXISTS mail_domains (
  domain VARCHAR(253) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL PRIMARY KEY,
  enabled TINYINT(1) NOT NULL DEFAULT 1,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  deleted_at TIMESTAMP NULL DEFAULT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS mailboxes (
  email VARCHAR(254) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL PRIMARY KEY,
  local_part VARCHAR(64) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  domain VARCHAR(253) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  -- Dovecot scheme-prefixed hash (`{BLF-CRYPT}...` or `{CRYPT}...`); never plaintext.
  password_hash VARCHAR(255) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
  mail_location VARCHAR(512) CHARACTER SET utf8mb4 NOT NULL,
  home VARCHAR(512) CHARACTER SET utf8mb4 NOT NULL,
  uid INT UNSIGNED NOT NULL DEFAULT 5000,
  gid INT UNSIGNED NOT NULL DEFAULT 5000,
  quota_mb INT UNSIGNED NULL DEFAULT NULL,
  enabled TINYINT(1) NOT NULL DEFAULT 1,
  source VARCHAR(32) CHARACTER SET ascii NOT NULL DEFAULT 'panel',
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  deleted_at TIMESTAMP NULL DEFAULT NULL,
  KEY idx_mailboxes_domain (domain)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS mail_aliases (
  source VARCHAR(254) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  destination VARCHAR(254) CHARACTER SET ascii COLLATE ascii_general_ci NOT NULL,
  enabled TINYINT(1) NOT NULL DEFAULT 1,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (source, destination)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS mail_meta (
  k VARCHAR(64) CHARACTER SET ascii NOT NULL PRIMARY KEY,
  v VARCHAR(512) NOT NULL,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE OR REPLACE SQL SECURITY DEFINER VIEW mailbox_auth AS
  SELECT email, password_hash, mail_location, home, uid, gid
  FROM mailboxes
  WHERE enabled = 1 AND deleted_at IS NULL;
