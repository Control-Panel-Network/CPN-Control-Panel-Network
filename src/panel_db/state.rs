//! Orchestration of the MariaDB panel-state migration.
//!
//! [`ensure`] is the single entry point used on panel start (which is how a
//! package upgrade, a tip upgrade, and a fresh install all reach it) and by
//! `cpn panel-db migrate`. A full run happens once per installed version:
//! back up, provision, import, verify, mail import, privilege audit. Between
//! upgrades the same entry point only performs a cheap incremental sync.

use super::backup;
use super::client::{AdminSql, PanelSql, SqlExec};
use super::config::{PanelDbConfig, disabled_by_operator, write_private_atomic};
use super::crypto::PanelKey;
use super::docs_store::{DocStore, MariaDocStore};
use super::import::{ImportReport, VerifyReport, import_all, verify};
use super::mail::{self, CutoverPaths, DOVECOT_SQL_CONF, RealDovecot};
use super::provision;
use crate::paths::{default_data_dir, join_data};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const STATE_FILE: &str = "panel-db-state.json";
pub const LOCK_FILE: &str = "panel-db-auto.lock";
const LOCK_STALE: Duration = Duration::from_secs(30 * 60);
const KEEP_BACKUPS: usize = 5;
const VERIFY_ATTEMPTS: u32 = 4;
const VERIFY_PAUSE: Duration = Duration::from_secs(2);
/// Identifies this build: package version plus the commit it was built from, so a tip
/// upgrade (same version number, new commit and possibly new migrations) re-runs migrate.
fn build_key() -> String {
    let sha = env!("CPN_GIT_SHA");
    let short: String = sha.chars().take(12).collect();
    if short.is_empty() {
        env!("CARGO_PKG_VERSION").to_string()
    } else {
        format!("{}+{short}", env!("CARGO_PKG_VERSION"))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PanelDbState {
    #[serde(default)]
    pub last_version: String,
    #[serde(default)]
    pub last_run_unix: u64,
    #[serde(default)]
    pub last_ok: bool,
    #[serde(default)]
    pub last_summary: String,
    #[serde(default)]
    pub last_backup: Option<String>,
}

impl PanelDbState {
    pub fn load() -> Self {
        fs::read_to_string(join_data(STATE_FILE))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Could not serialize panel database state: {e}"))?;
        write_private_atomic(&join_data(STATE_FILE), json.as_bytes())
    }

    /// True when this version has not yet completed a clean full migration.
    pub fn needs_full_run(&self, config_present: bool, version: &str, force: bool) -> bool {
        force || !config_present || !self.last_ok || self.last_version != version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Disabled,
    Busy,
    /// MariaDB is not installed or not reachable yet; retried on the next start.
    Skipped(String),
    Synced(String),
    Migrated(String),
}

impl Outcome {
    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "Panel database migration is disabled by the operator".into(),
            Self::Busy => "Another panel database run is in progress".into(),
            Self::Skipped(why) => format!("Panel database step skipped: {why}"),
            Self::Synced(s) => format!("Panel database synced: {s}"),
            Self::Migrated(s) => format!("Panel database migrated: {s}"),
        }
    }
}

struct RunLock(PathBuf);

impl RunLock {
    fn acquire() -> Option<Self> {
        let path = join_data(LOCK_FILE);
        for _ in 0..2 {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if options.open(&path).is_ok() {
                return Some(Self(path));
            }
            let stale = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|age| age > LOCK_STALE);
            if !stale {
                return None;
            }
            let _ = fs::remove_file(&path);
        }
        None
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn log(level: &str, message: &str) {
    crate::upgrade_tip_log::log_tagged_main_only("panel-db", level, message);
}

fn reachable(admin: &dyn SqlExec) -> Result<(), String> {
    if crate::panel_ops_db::mariadb_client_bin().is_none() {
        return Err("MariaDB client is not installed".into());
    }
    admin
        .run(None, "SELECT 1")
        .map(|_| ())
        .map_err(|e| format!("MariaDB is not reachable ({e})"))
}

fn existing_databases(admin: &dyn SqlExec, cfg: &PanelDbConfig) -> Result<Vec<String>, String> {
    let rows = admin.query(None, "SELECT schema_name FROM information_schema.schemata")?;
    let names: Vec<String> = rows
        .into_iter()
        .filter_map(|r| r.into_iter().next())
        .collect();
    Ok([&cfg.panel_db, &cfg.mail_db]
        .into_iter()
        .filter(|db| names.iter().any(|n| n == *db))
        .cloned()
        .collect())
}

fn import_summary(import: &ImportReport, check: &VerifyReport) -> String {
    format!("{}; verify: {}", import.summary(), check.summary())
}

/// Import mailbox rows from a classic passdb (inside MariaDB) and optionally cut over.
fn mail_step(admin: &dyn SqlExec, cfg: &mut PanelDbConfig, now: u64) -> Result<String, String> {
    let Some(legacy) = mail::detect_legacy_db(admin) else {
        return Ok("no classic mailbox table found".into());
    };
    cfg.legacy_mail_db = Some(legacy.clone());
    let refresh = !mail::cutover_done(admin, cfg)?;
    let counts = mail::import_legacy(admin, cfg, &legacy, refresh)?;
    let mut line = counts.summary();
    if cfg.auto_mail_cutover && refresh && counts.ready_for_cutover() {
        let paths = CutoverPaths {
            conf: PathBuf::from(DOVECOT_SQL_CONF),
            backup_dir: PathBuf::from(backup::BACKUP_ROOT).join("dovecot"),
        };
        line.push_str("; ");
        line.push_str(&mail::cutover(
            admin,
            cfg,
            &legacy,
            &paths,
            &RealDovecot,
            now,
        )?);
    }
    Ok(line)
}

fn open_store<'a>(panel: &'a PanelSql, cfg: &PanelDbConfig) -> Result<MariaDocStore<'a>, String> {
    Ok(MariaDocStore::new(
        panel,
        &cfg.panel_db,
        PanelKey::load(false)?,
    ))
}

fn finish_state(version: &str, ok: bool, summary: &str, backup: Option<String>) {
    let state = PanelDbState {
        last_version: version.into(),
        last_run_unix: crate::account::now_unix(),
        last_ok: ok,
        last_summary: summary.chars().take(500).collect(),
        last_backup: backup,
    };
    if let Err(e) = state.save() {
        log(
            "error",
            &format!("Could not record panel database state: {e}"),
        );
    }
}

fn sync_only(cfg: &PanelDbConfig) -> Result<Outcome, String> {
    let panel = PanelSql::new(cfg);
    let store = open_store(&panel, cfg)?;
    let root = default_data_dir();
    let import = import_all(&root, &store, false)?;
    if !import.ok() {
        return Err(format!(
            "Panel database sync failed for {} file(s)",
            import.failed
        ));
    }
    Ok(Outcome::Synced(import.summary()))
}

/// Import, then verify; files that change while the run is in flight (maintenance
/// pages, notifications during an upgrade) are re-synced and re-verified a few times.
fn import_until_stable(
    root: &Path,
    store: &dyn DocStore,
    attempts: u32,
    pause: Duration,
) -> Result<(ImportReport, VerifyReport), String> {
    let mut import = import_all(root, store, false)?;
    let mut check = verify(root, store)?;
    for _ in 1..attempts.max(1) {
        if check.ok() {
            break;
        }
        std::thread::sleep(pause);
        let again = import_all(root, store, false)?;
        import.inserted += again.inserted;
        import.updated += again.updated;
        import.failed = again.failed;
        import.failures = again.failures;
        check = verify(root, store)?;
    }
    Ok((import, check))
}

/// Full migration. The error carries the backup dir (when one was made) so the
/// caller can record it alongside the failure.
fn full_run(
    admin: &dyn SqlExec,
    cfg: &mut PanelDbConfig,
    context: &str,
) -> Result<Outcome, (String, Option<String>)> {
    let now = crate::account::now_unix();
    // Persist credentials first so a half-finished run can be resumed, never orphaned.
    PanelKey::load(true).map_err(|e| (e, None))?;
    cfg.save().map_err(|e| (e, None))?;
    let dbs = existing_databases(admin, cfg).map_err(|e| (e, None))?;
    let backup = backup::run(now, &dbs)
        .map_err(|e| (format!("Backup failed, migration not started: {e}"), None))?;
    log("info", &format!("[{context}] {}", backup.summary()));
    let dir = backup.dir.display().to_string();
    steps_after_backup(admin, cfg, now, &backup).map_err(|e| (e, Some(dir)))
}

fn steps_after_backup(
    admin: &dyn SqlExec,
    cfg: &mut PanelDbConfig,
    now: u64,
    backup: &backup::BackupReport,
) -> Result<Outcome, String> {
    let applied = provision::provision(admin, cfg)?;
    provision::self_test(cfg)?;
    let panel = PanelSql::new(cfg);
    let store = open_store(&panel, cfg)?;
    let root = default_data_dir();
    let (import, check) = import_until_stable(&root, &store, VERIFY_ATTEMPTS, VERIFY_PAUSE)?;
    let mail = mail_step(admin, cfg, now)?;
    cfg.save()?;
    let problems = provision::audit_privileges(admin, cfg)?;
    let mut summary = format!(
        "{}; migrations applied: {}; mail: {mail}",
        import_summary(&import, &check),
        if applied.is_empty() {
            "none".into()
        } else {
            applied.join(",")
        }
    );
    let ok = import.ok() && check.ok() && problems.is_empty();
    if !problems.is_empty() {
        summary.push_str(&format!("; privilege problems: {}", problems.join(" | ")));
    }
    if ok {
        finish_state(
            &build_key(),
            true,
            &summary,
            Some(backup.dir.display().to_string()),
        );
        backup::prune(std::path::Path::new(backup::BACKUP_ROOT), KEEP_BACKUPS);
        Ok(Outcome::Migrated(summary))
    } else {
        Err(summary)
    }
}

/// Idempotent entry point (panel start, upgrade, `cpn panel-db migrate`).
pub fn ensure(context: &str, force: bool) -> Result<Outcome, String> {
    if disabled_by_operator() {
        return Ok(Outcome::Disabled);
    }
    let existing = PanelDbConfig::load()?;
    let state = PanelDbState::load();
    let build = build_key();
    let full = state.needs_full_run(existing.is_some(), &build, force);
    let admin = AdminSql;
    if let Err(why) = reachable(&admin) {
        return Ok(Outcome::Skipped(why));
    }
    let Some(_lock) = RunLock::acquire() else {
        return Ok(Outcome::Busy);
    };
    let result = if full {
        let mut cfg =
            existing.unwrap_or_else(|| PanelDbConfig::generate(crate::account::now_unix()));
        full_run(&admin, &mut cfg, context).map_err(|(e, dir)| {
            finish_state(&build, false, &e, dir.or_else(|| state.last_backup.clone()));
            e
        })
    } else {
        sync_only(&existing.ok_or("Panel database config is missing")?)
    };
    match &result {
        Ok(outcome) => log("info", &format!("[{context}] {}", outcome.message())),
        Err(e) => log("error", &format!("[{context}] {e}")),
    }
    result
}

/// Background-friendly wrapper for the panel start hook (never panics, never blocks boot).
pub fn ensure_logged(context: &str) {
    // `ensure` already logs every outcome; the result is only needed by the CLI.
    let _ = ensure(context, false);
}

/// Human-readable status without secrets.
pub fn status_lines() -> Vec<String> {
    let mut out = Vec::new();
    let state = PanelDbState::load();
    match PanelDbConfig::load() {
        Ok(Some(cfg)) => {
            out.push(format!("Config: present (mode {})", cfg.mode.label()));
            out.push(format!("Databases: {} and {}", cfg.panel_db, cfg.mail_db));
            let panel = PanelSql::new(&cfg);
            let docs = panel
                .query(
                    Some(&cfg.panel_db),
                    "SELECT COUNT(*), SUM(encrypted) FROM panel_documents",
                )
                .ok()
                .and_then(|r| r.into_iter().next());
            match docs {
                Some(row) => out.push(format!(
                    "Documents in {}: {} ({} encrypted)",
                    cfg.panel_db,
                    row.first().map(String::as_str).unwrap_or("0"),
                    row.get(1)
                        .map(String::as_str)
                        .filter(|v| *v != "NULL")
                        .unwrap_or("0")
                )),
                None => out.push("Documents: database not reachable with the panel account".into()),
            }
            if let Ok(rows) = panel.query(Some(&cfg.mail_db), "SELECT COUNT(*) FROM mailboxes") {
                let n = rows
                    .first()
                    .and_then(|r| r.first())
                    .map(String::as_str)
                    .unwrap_or("0");
                out.push(format!("Mailboxes in {}: {n}", cfg.mail_db));
            }
            out.push(format!(
                "Dovecot passdb: {}",
                fs::read_to_string(DOVECOT_SQL_CONF)
                    .map(|c| format!("{:?}", mail::backend_of(&c)))
                    .unwrap_or_else(|_| "unknown".into())
            ));
        }
        Ok(None) => out.push("Config: not provisioned yet (runs on the next panel start)".into()),
        Err(e) => out.push(format!("Config: {e}")),
    }
    out.push(format!(
        "Last run: version {} ok={} backup={} :: {}",
        if state.last_version.is_empty() {
            "never"
        } else {
            &state.last_version
        },
        state.last_ok,
        state.last_backup.as_deref().unwrap_or("none"),
        if state.last_summary.is_empty() {
            "no summary"
        } else {
            &state.last_summary
        }
    ));
    if disabled_by_operator() {
        out.push("Operator opt-out is active".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_run_needed_until_this_version_succeeds() {
        let mut s = PanelDbState::default();
        assert!(s.needs_full_run(true, "1.0.0", false));
        s.last_ok = true;
        s.last_version = "1.0.0".into();
        assert!(!s.needs_full_run(true, "1.0.0", false));
        assert!(
            s.needs_full_run(true, "1.0.1", false),
            "upgrade re-runs migrate"
        );
        assert!(
            s.needs_full_run(false, "1.0.0", false),
            "missing config re-provisions"
        );
        assert!(
            s.needs_full_run(true, "1.0.0", true),
            "force always re-runs"
        );
        s.last_ok = false;
        assert!(
            s.needs_full_run(true, "1.0.0", false),
            "failed run retries on next start"
        );
    }

    #[test]
    fn outcome_messages_never_empty() {
        for o in [
            Outcome::Disabled,
            Outcome::Busy,
            Outcome::Skipped("x".into()),
            Outcome::Synced("y".into()),
            Outcome::Migrated("z".into()),
        ] {
            assert!(!o.message().is_empty());
        }
    }

    #[test]
    fn state_defaults_are_not_ok() {
        let s = PanelDbState::default();
        assert!(!s.last_ok && s.last_version.is_empty());
    }

    #[test]
    fn import_until_stable_converges_and_reports_clean() {
        use super::super::docs_store::mem::MemDocStore;
        let root = std::env::temp_dir().join(format!("cpn-pdb-stable-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("packages.json"), "{}").unwrap();
        fs::write(root.join("maintenance.json"), "{\"on\":false}").unwrap();
        let store = MemDocStore::new(super::super::crypto::panel_key_test_fixture("state-stable"));
        let (import, check) =
            import_until_stable(&root, &store, 3, Duration::from_millis(1)).unwrap();
        assert!(import.ok() && check.ok());
        assert_eq!(import.inserted, 2);
        // A file that drifted since the last pass is picked up on the next run.
        fs::write(root.join("maintenance.json"), "{\"on\":true}").unwrap();
        let (again, check2) =
            import_until_stable(&root, &store, 3, Duration::from_millis(1)).unwrap();
        assert!(check2.ok());
        assert_eq!((again.inserted, again.updated), (0, 1));
        let _ = fs::remove_dir_all(&root);
    }
}
