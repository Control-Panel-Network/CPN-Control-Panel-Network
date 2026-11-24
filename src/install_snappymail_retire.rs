//! Retire the prior webmail (SnappyMail) after Tachyon is live.
//!
//! Order: import prior data into Tachyon (idempotent), make sure Tachyon is the active
//! client and healthy, archive everything under `/home/backup`, verify the archives, and
//! only then remove the prior application/data trees and (when proven safe) its MariaDB
//! schema. Mailbox Maildirs and Tachyon data/keys are never touched.

use crate::install_snappymail_retire_db::{self as retire_db, DbInventory};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const OPT_ROOT: &str = "/opt/cpn-webmail";
const PRIOR_APP: &str = "/opt/cpn-webmail/snappymail";
const PRIOR_DATA: &str = "/var/lib/cpn-webmail/snappymail";
const CLASSIC_TREES: &[&str] = &[
    "/usr/local/lscp/cyberpanel/snappymail",
    "/usr/local/CyberCP/public/snappymail",
    "/usr/local/CyberCPBak/public/snappymail",
];
const BACKUP_ROOT: &str = "/home/backup";
const DONE_MARKER: &str = "/var/lib/cpn/webmail-snappy-retire-v1";
const ATTEMPT_STAMP: &str = "/var/lib/cpn/webmail-snappy-retire-attempt";
const MIGRATE_MARKER: &str = "/var/lib/cpn-webmail/tachyon/_data_/_default_/.cpn-snappy-migrate-v1";
const AUTO_RETRY_SECS: u64 = 6 * 3600;

#[derive(Debug, Clone, Copy, Default)]
pub struct RetireOptions {
    pub dry_run: bool,
    pub keep_db: bool,
    /// Operator-initiated: may switch the active webmail from SnappyMail to Tachyon.
    /// Automatic heal never switches an operator's explicit SnappyMail choice.
    pub allow_switch: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RetireReport {
    pub backup_dir: Option<PathBuf>,
    pub removed: Vec<String>,
    pub kept: Vec<String>,
    pub notes: Vec<String>,
}

impl RetireReport {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(d) = &self.backup_dir {
            parts.push(format!("backup: {}", d.display()));
        }
        parts.push(format!("removed: {}", join_or_none(&self.removed)));
        parts.push(format!("kept: {}", join_or_none(&self.kept)));
        parts.extend(self.notes.iter().cloned());
        parts.join("; ")
    }
}

fn join_or_none(v: &[String]) -> String {
    if v.is_empty() {
        "none".into()
    } else {
        v.join(", ")
    }
}

/// Only these exact trees (or obsolete `snappymail-*` extract dirs) may ever be removed.
pub(crate) fn is_removable(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s == PRIOR_APP || s == PRIOR_DATA || CLASSIC_TREES.iter().any(|c| *c == s) {
        return true;
    }
    path.parent() == Some(Path::new(OPT_ROOT))
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("snappymail-") && n.len() > "snappymail-".len())
}

fn classic_panel_gone() -> bool {
    !Path::new("/usr/local/CyberCP/manage.py").exists()
        && !crate::service_detect::systemd_unit_active("lscpd")
}

/// Prior-webmail trees that exist on disk (classic trees only when that panel is gone).
pub fn find_leftover_trees() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in [PRIOR_APP, PRIOR_DATA] {
        if fs::symlink_metadata(p).is_ok() {
            out.push(PathBuf::from(p));
        }
    }
    if let Ok(rd) = fs::read_dir(OPT_ROOT) {
        for ent in rd.flatten() {
            let p = ent.path();
            if is_removable(&p) && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    if classic_panel_gone() {
        for c in CLASSIC_TREES {
            if fs::symlink_metadata(c).is_ok() {
                out.push(PathBuf::from(c));
            }
        }
    }
    out
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn tachyon_ready() -> bool {
    crate::active_webmail::client_files_present(crate::model::MailSystem::Tachyon)
        && Path::new("/var/lib/cpn-webmail/tachyon").is_dir()
}

/// Cheap gate for automatic heal: Tachyon present, not done, throttled, something to retire.
pub fn auto_retire_pending() -> bool {
    if Path::new(DONE_MARKER).is_file() || !tachyon_ready() {
        return false;
    }
    if let Ok(age) = fs::metadata(ATTEMPT_STAMP)
        .and_then(|m| m.modified())
        .and_then(|t| t.elapsed().map_err(std::io::Error::other))
        && age.as_secs() < AUTO_RETRY_SECS
    {
        return false;
    }
    !find_leftover_trees().is_empty()
        || retire_db::inventory()
            .map(|v| !v.is_empty())
            .unwrap_or(false)
}

/// Heal hook: run at most once per throttle window, never fails the caller.
pub fn auto_retire_fail_soft() -> Option<String> {
    if !auto_retire_pending() {
        return None;
    }
    let _ = fs::create_dir_all("/var/lib/cpn");
    let _ = fs::write(ATTEMPT_STAMP, now_secs().to_string());
    Some(match retire_snappymail(RetireOptions::default()) {
        Ok(r) => format!("prior webmail retire: {}", r.summary()),
        Err(e) => format!("prior webmail retire skipped: {e}"),
    })
}

fn ensure_tachyon_active_and_healthy(
    report: &mut RetireReport,
    allow_switch: bool,
) -> Result<(), String> {
    use crate::model::MailSystem;
    if crate::panel_webmail::detect_webmail_client() == Some(MailSystem::Snappymail) {
        if !allow_switch {
            return Err(
                "SnappyMail is still the active webmail; run `cpn app retire-snappymail` to switch to Tachyon and clean up"
                    .into(),
            );
        }
        let msg = crate::apps_webmail::activate_webmail_app(crate::apps::AppId::Tachyon)?;
        report
            .notes
            .push(format!("switched active webmail to Tachyon ({msg})"));
    }
    if crate::panel_webmail::detect_webmail_client() == Some(MailSystem::Tachyon) {
        crate::install_webmail_health::verify_webmail_http_surface("Tachyon")
            .map_err(|e| format!("Tachyon health check failed, nothing removed: {e}"))?;
    } else {
        report
            .notes
            .push("active webmail is not Tachyon; HTTP health check skipped".into());
    }
    Ok(())
}

fn run_tar(args: &[&str]) -> Result<(), String> {
    let st = Command::new("tar")
        .args(args)
        .status()
        .map_err(|e| format!("tar failed to start: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err("tar failed".into())
    }
}

fn backup_tree(tree: &Path, dir: &Path, idx: usize) -> Result<(), String> {
    let name = tree.file_name().and_then(|n| n.to_str()).unwrap_or("tree");
    let parent = tree.parent().ok_or("tree has no parent")?;
    let out = dir.join(format!("{idx:02}-{name}.tar.gz"));
    let out_s = out.to_string_lossy().to_string();
    run_tar(&[
        "-czf",
        &out_s,
        "--numeric-owner",
        "-C",
        &parent.to_string_lossy(),
        name,
    ])
    .map_err(|e| format!("backup of {} failed: {e}", tree.display()))?;
    run_tar(&["-tzf", &out_s]).map_err(|_| format!("backup of {} not readable", tree.display()))?;
    if fs::metadata(&out).map(|m| m.len()).unwrap_or(0) < 32 {
        return Err(format!("backup of {} is empty", tree.display()));
    }
    Ok(())
}

fn make_backup_dir() -> Result<PathBuf, String> {
    let dir = Path::new(BACKUP_ROOT).join(format!("cpn-snappymail-retire-{}", now_secs()));
    fs::create_dir_all(&dir).map_err(|e| format!("backup dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

fn remove_tree(path: &Path) -> Result<(), String> {
    if !is_removable(path) {
        return Err(format!("refusing to remove {}", path.display()));
    }
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path).map_err(|e| e.to_string())
    } else {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    }
}

#[cfg(unix)]
fn repoint_current_if_dangling() {
    let cur = Path::new("/opt/cpn-webmail/current");
    let Ok(target) = fs::read_link(cur) else {
        return;
    };
    if target.to_string_lossy().contains("snappymail") {
        let _ = fs::remove_file(cur);
        let _ = std::os::unix::fs::symlink("/opt/cpn-webmail/tachyon", cur);
    }
}

#[cfg(not(unix))]
fn repoint_current_if_dangling() {}

fn handle_databases(
    opts: RetireOptions,
    backup: &Path,
    report: &mut RetireReport,
) -> Result<(), String> {
    let inv: Vec<DbInventory> = retire_db::inventory()?;
    for db in inv {
        let dump = retire_db::dump_verified(&db.name, backup)?;
        report.notes.push(format!(
            "database `{}` dumped to {}",
            db.name,
            dump.display()
        ));
        if opts.keep_db {
            report
                .kept
                .push(format!("database {} (--keep-db)", db.name));
            continue;
        }
        match retire_db::drop_allowed(&db) {
            Ok(()) => {
                match crate::panel_ops_db::drop_database_with_optional_users(&db.name, true) {
                    Ok(_) => report.removed.push(format!("database {}", db.name)),
                    Err(e) => report
                        .kept
                        .push(format!("database {} (drop failed: {e})", db.name)),
                }
            }
            Err(why) => report.kept.push(format!("database {} ({why})", db.name)),
        }
    }
    Ok(())
}

/// Migrate, verify Tachyon, back up, then remove prior-webmail trees and DB.
pub fn retire_snappymail(opts: RetireOptions) -> Result<RetireReport, String> {
    let mut report = RetireReport::default();
    let trees = find_leftover_trees();
    let dbs = retire_db::inventory().unwrap_or_default();
    if trees.is_empty() && dbs.is_empty() {
        report.notes.push("no prior webmail leftovers found".into());
        if !opts.dry_run {
            let _ = fs::create_dir_all("/var/lib/cpn");
            let _ = fs::write(DONE_MARKER, now_secs().to_string());
        }
        return Ok(report);
    }
    if !tachyon_ready() {
        return Err("Tachyon is not installed/ready; keeping prior webmail".into());
    }
    if opts.dry_run {
        report.removed = trees.iter().map(|p| p.display().to_string()).collect();
        report.removed.extend(
            dbs.iter()
                .map(|d| format!("database {} (if contacts imported)", d.name)),
        );
        report.notes.push("dry run: nothing changed".into());
        return Ok(report);
    }

    let migrated = crate::install_snappymail_migrate::migrate_snappymail_into_tachyon()?;
    report.notes.push(migrated);
    if !Path::new(MIGRATE_MARKER).is_file() {
        return Err("Tachyon import marker missing after migrate; nothing removed".into());
    }
    ensure_tachyon_active_and_healthy(&mut report, opts.allow_switch)?;

    let backup = make_backup_dir()?;
    for (i, tree) in trees.iter().enumerate() {
        backup_tree(tree, &backup, i)?; // abort before any removal if an archive is bad
    }
    handle_databases(opts, &backup, &mut report)?;
    report.backup_dir = Some(backup);

    for tree in &trees {
        match remove_tree(tree) {
            Ok(()) => report.removed.push(tree.display().to_string()),
            Err(e) => report.kept.push(format!("{} ({e})", tree.display())),
        }
    }
    repoint_current_if_dangling();
    if let Err(e) = crate::install_webmail_health::verify_webmail_http_surface("Tachyon") {
        report
            .notes
            .push(format!("post-cleanup health warning: {e}"));
    } else if report.kept.is_empty() {
        let _ = fs::create_dir_all("/var/lib/cpn");
        let _ = fs::write(DONE_MARKER, now_secs().to_string());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removable_allowlist_is_strict() {
        assert!(is_removable(Path::new("/opt/cpn-webmail/snappymail")));
        assert!(is_removable(Path::new("/var/lib/cpn-webmail/snappymail")));
        assert!(is_removable(Path::new(
            "/opt/cpn-webmail/snappymail-2.38.2"
        )));
        assert!(is_removable(Path::new(
            "/usr/local/CyberCP/public/snappymail"
        )));
        assert!(!is_removable(Path::new("/opt/cpn-webmail/tachyon")));
        assert!(!is_removable(Path::new("/var/lib/cpn-webmail/tachyon")));
        assert!(!is_removable(Path::new("/var/lib/cpn-webmail/snappy-repo")));
        assert!(!is_removable(Path::new("/opt/cpn-webmail/snappymail-")));
        assert!(!is_removable(Path::new("/opt/cpn-webmail/x/snappymail-1")));
        assert!(!is_removable(Path::new("/home/example.com/e-mails")));
    }

    #[test]
    fn report_summary_lists_parts() {
        let r = RetireReport {
            removed: vec!["a".into()],
            ..RetireReport::default()
        };
        let s = r.summary();
        assert!(s.contains("removed: a") && s.contains("kept: none"));
    }
}
