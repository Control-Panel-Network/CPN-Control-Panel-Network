//! Preflight, change journal, and rollback for install stages (issue #13).

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub const JOURNAL_FILE: &str = "install-journal.jsonl";
pub const BACKUP_DIR: &str = "install-backups";

static CURRENT_RUN_ID: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalAction {
    CreatedFile,
    ChangedFile,
    CreatedDir,
    EnabledService,
    InstalledPackage,
    WroteRepo,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub ts_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub stage: String,
    pub action: JournalAction,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    FailedRolledBack,
    FailedPartial,
}

#[derive(Debug, Clone)]
pub struct PreflightReport {
    pub os_ok: bool,
    pub root_ok: bool,
    pub disk_ok: bool,
    pub notes: Vec<String>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0)
}

fn data_root() -> PathBuf {
    crate::manifest::data_dir()
}

pub fn journal_path() -> PathBuf {
    data_root().join(JOURNAL_FILE)
}

pub fn backup_root() -> PathBuf {
    data_root().join(BACKUP_DIR)
}

/// Start a scoped install transaction; rollback only undoes this run (issue #13).
pub fn begin_install_run(label: &str) -> Result<String, String> {
    ensure_journal_dirs()?;
    let stamp = now_unix();
    let run_id = format!("run-{stamp}-{label}");
    {
        let mut guard = CURRENT_RUN_ID
            .lock()
            .map_err(|_| "install journal run lock poisoned".to_string())?;
        *guard = Some(run_id.clone());
    }
    record(
        "run",
        JournalAction::Note,
        "begin",
        None,
        Some(format!("begin install run {run_id}")),
    )?;
    Ok(run_id)
}

pub fn end_install_run() {
    if let Ok(mut guard) = CURRENT_RUN_ID.lock() {
        *guard = None;
    }
}

fn current_run_id() -> Option<String> {
    CURRENT_RUN_ID.lock().ok().and_then(|guard| guard.clone())
}

/// Run cheap checks before mutating the host.
pub fn run_preflight(min_free_mb: u64) -> Result<PreflightReport, String> {
    let mut notes = Vec::new();
    let mut os_ok = true;
    let root_ok;
    let mut disk_ok = true;

    match crate::os_support::require_installable_guest() {
        Ok(guest) => notes.push(format!("guest ok: {} ({})", guest.label, guest.pretty_name)),
        Err(error) => {
            os_ok = false;
            notes.push(format!("guest check failed: {error}"));
        }
    }

    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            root_ok = false;
            notes.push("installer is not running as root".into());
        } else {
            root_ok = true;
            notes.push("running as root".into());
        }
    }
    #[cfg(not(unix))]
    {
        root_ok = true;
        notes.push("root check skipped on non-unix host".into());
    }

    match free_disk_mb(Path::new("/")) {
        Ok(free) if free < min_free_mb => {
            disk_ok = false;
            notes.push(format!(
                "low disk: {free} MiB free (need {min_free_mb} MiB)"
            ));
        }
        Ok(free) => notes.push(format!("disk ok: {free} MiB free")),
        Err(error) => {
            disk_ok = false;
            notes.push(format!("disk check failed: {error}"));
        }
    }

    if !(os_ok && root_ok && disk_ok) {
        return Err(format!("Preflight failed: {}", notes.join("; ")));
    }
    Ok(PreflightReport {
        os_ok,
        root_ok,
        disk_ok,
        notes,
    })
}

fn free_disk_mb(path: &Path) -> Result<u64, String> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let c_path =
            CString::new(path.to_string_lossy().as_bytes()).map_err(|error| error.to_string())?;
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
        if rc != 0 {
            return Err(format!("statvfs failed for {}", path.display()));
        }
        let free = (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64) / (1024 * 1024);
        Ok(free)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(64 * 1024)
    }
}

pub fn ensure_journal_dirs() -> Result<(), String> {
    fs::create_dir_all(data_root())
        .map_err(|error| format!("Could not create {}: {error}", data_root().display()))?;
    fs::create_dir_all(backup_root())
        .map_err(|error| format!("Could not create {}: {error}", backup_root().display()))?;
    Ok(())
}

pub fn append_journal(entry: JournalEntry) -> Result<(), String> {
    ensure_journal_dirs()?;
    let path = journal_path();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Could not open journal: {error}"))?;
    let line = serde_json::to_string(&entry)
        .map_err(|error| format!("Could not serialize journal entry: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("Could not write journal: {error}"))?;
    Ok(())
}

pub fn record(
    stage: &str,
    action: JournalAction,
    path: &str,
    backup: Option<String>,
    detail: Option<String>,
) -> Result<(), String> {
    append_journal(JournalEntry {
        ts_unix: now_unix(),
        run_id: current_run_id(),
        stage: stage.into(),
        action,
        path: path.into(),
        backup,
        detail,
    })
}

/// Backup existing file (if any) then write new contents. Idempotent when content matches.
pub fn write_file_tracked(stage: &str, path: &Path, contents: &str) -> Result<(), String> {
    ensure_journal_dirs()?;
    if path.exists() {
        let existing = fs::read_to_string(path).unwrap_or_default();
        if existing == contents {
            record(
                stage,
                JournalAction::Note,
                &path.to_string_lossy(),
                None,
                Some("idempotent skip: content unchanged".into()),
            )?;
            return Ok(());
        }
        let backup = backup_path_for(path)?;
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(path, &backup).map_err(|error| {
            format!(
                "Could not backup {} to {}: {error}",
                path.display(),
                backup.display()
            )
        })?;
        fs::write(path, contents).map_err(|error| error.to_string())?;
        record(
            stage,
            JournalAction::ChangedFile,
            &path.to_string_lossy(),
            Some(backup.to_string_lossy().into_owned()),
            None,
        )?;
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            record(
                stage,
                JournalAction::CreatedDir,
                &parent.to_string_lossy(),
                None,
                None,
            )?;
        }
        fs::write(path, contents).map_err(|error| error.to_string())?;
        record(
            stage,
            JournalAction::CreatedFile,
            &path.to_string_lossy(),
            None,
            None,
        )?;
    }
    Ok(())
}

fn backup_path_for(path: &Path) -> Result<PathBuf, String> {
    let stamp = now_unix();
    let safe = path
        .to_string_lossy()
        .trim_start_matches('/')
        .replace(['/', '\\', ':'], "_");
    Ok(backup_root().join(format!("{stamp}_{safe}")))
}

pub fn load_journal() -> Result<Vec<JournalEntry>, String> {
    let path = journal_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut entries = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let line = line.trim().trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let entry: JournalEntry = serde_json::from_str(line)
            .map_err(|error| format!("Invalid journal line {}: {error}", idx + 1))?;
        entries.push(entry);
    }
    Ok(entries)
}

#[derive(Debug, Clone)]
pub struct RollbackReport {
    pub restored: Vec<String>,
    pub removed: Vec<String>,
    pub skipped: Vec<String>,
    pub kind: FailureKind,
}

/// Restore tracked file changes for the current install run only (newest first).
pub fn rollback_tracked_files() -> Result<RollbackReport, String> {
    let run_id = current_run_id();
    let entries = load_journal()?;
    let scoped: Vec<&JournalEntry> = entries
        .iter()
        .filter(|entry| match (&run_id, &entry.run_id) {
            (Some(expected), Some(actual)) => actual == expected,
            (Some(_), None) => false,
            (None, _) => true,
        })
        .collect();
    let mut restored = Vec::new();
    let mut removed = Vec::new();
    let mut skipped = Vec::new();

    for entry in scoped.iter().rev() {
        match entry.action {
            JournalAction::ChangedFile => {
                if let Some(backup) = entry.backup.as_ref() {
                    let backup_path = Path::new(backup);
                    if backup_path.exists() {
                        if let Some(parent) = Path::new(&entry.path).parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        fs::copy(backup_path, &entry.path).map_err(|error| {
                            format!("Rollback restore failed for {}: {error}", entry.path)
                        })?;
                        restored.push(entry.path.clone());
                    } else {
                        skipped.push(format!("{} (missing backup)", entry.path));
                    }
                } else {
                    skipped.push(format!("{} (no backup)", entry.path));
                }
            }
            JournalAction::CreatedFile | JournalAction::WroteRepo => {
                let path = Path::new(&entry.path);
                if path.is_file() {
                    fs::remove_file(path).map_err(|error| {
                        format!("Rollback remove failed for {}: {error}", entry.path)
                    })?;
                    removed.push(entry.path.clone());
                }
            }
            JournalAction::InstalledPackage | JournalAction::EnabledService => {
                skipped.push(format!(
                    "{} ({:?}): not auto-reverted; see recovery notes",
                    entry.path, entry.action
                ));
            }
            JournalAction::CreatedDir | JournalAction::Note => {}
        }
    }

    let kind = if skipped
        .iter()
        .any(|s| s.contains("InstalledPackage") || s.contains("EnabledService"))
        || skipped.iter().any(|s| s.contains("missing backup"))
    {
        FailureKind::FailedPartial
    } else {
        FailureKind::FailedRolledBack
    };

    record(
        "rollback",
        JournalAction::Note,
        "rollback",
        None,
        Some(format!(
            "kind={kind:?}; run={run_id:?}; restored={}; removed={}",
            restored.len(),
            removed.len()
        )),
    )?;
    end_install_run();

    Ok(RollbackReport {
        restored,
        removed,
        skipped,
        kind,
    })
}

pub fn failure_message(kind: FailureKind) -> String {
    let journal = crate::paths::join_data("install-journal.jsonl");
    let backups = crate::paths::join_data("install-backups");
    match kind {
        FailureKind::FailedRolledBack => format!(
            "Installation failed; tracked file changes were rolled back. Review {}.",
            journal.display()
        ),
        FailureKind::FailedPartial => format!(
            "Installation failed with partial changes remaining (packages/services may still be present). Review {} and {}.",
            journal.display(),
            backups.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn write_and_rollback_restores_prior_content() {
        with_test_data_dir(|| {
            begin_install_run("unit").unwrap();
            let target = data_root().join("sample.conf");
            fs::create_dir_all(data_root()).unwrap();
            fs::write(&target, "old\n").unwrap();
            write_file_tracked("unit", &target, "new\n").unwrap();
            assert_eq!(fs::read_to_string(&target).unwrap(), "new\n");
            let entries = load_journal().unwrap();
            assert!(
                entries
                    .iter()
                    .any(|e| matches!(e.action, JournalAction::ChangedFile)),
                "expected ChangedFile journal entry, got {entries:?}"
            );
            let report = rollback_tracked_files().unwrap();
            assert!(
                !report.restored.is_empty(),
                "expected restored files, report={report:?}"
            );
            assert_eq!(fs::read_to_string(&target).unwrap(), "old\n");
            assert_eq!(report.kind, FailureKind::FailedRolledBack);
        });
    }

    #[test]
    fn idempotent_write_skips_identical_content() {
        with_test_data_dir(|| {
            let target = data_root().join("same.conf");
            write_file_tracked("unit", &target, "same\n").unwrap();
            write_file_tracked("unit", &target, "same\n").unwrap();
            let entries = load_journal().unwrap();
            assert!(
                entries
                    .iter()
                    .any(|e| e.detail.as_deref() == Some("idempotent skip: content unchanged")),
                "expected idempotent skip, got {entries:?}"
            );
        });
    }

    #[test]
    fn failure_messages_are_honest() {
        assert!(failure_message(FailureKind::FailedPartial).contains("partial"));
        assert!(!failure_message(FailureKind::FailedPartial).contains("de forma segura"));
        assert!(failure_message(FailureKind::FailedRolledBack).contains("rolled back"));
    }
}
