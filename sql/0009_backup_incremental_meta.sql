-- Incremental backup job metadata (optional SQL mirror of JSON under $CPN_DATA_DIR).
-- CPN stores jobs in backup-incremental-jobs.json by default; this table is for
-- operators who prefer MariaDB auditing alongside the JSON store.

CREATE TABLE IF NOT EXISTS cpn_backup_incremental_jobs (
  id VARCHAR(64) NOT NULL,
  engine VARCHAR(32) NOT NULL DEFAULT 'restic',
  source_path TEXT NOT NULL,
  destination VARCHAR(32) NOT NULL DEFAULT 'local',
  schedule_cron VARCHAR(64) NOT NULL DEFAULT '0 2 * * *',
  enabled TINYINT(1) NOT NULL DEFAULT 0,
  last_run_unix BIGINT UNSIGNED NOT NULL DEFAULT 0,
  last_status VARCHAR(255) NOT NULL DEFAULT '',
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;