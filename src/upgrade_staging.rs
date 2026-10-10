//! Per-job staging directories for upgrade / repair plus orphan sweeps.
//!
//! Every maintenance job (release package, commit tip, repair) downloads into
//! one `/var/tmp/cpn-<kind>-<random>` directory owned by a [`StagingDir`] guard.
//! The guard removes the directory when the job ends, on success and on every
//! error path (RAII), so a failed cargo build or a lost download does not leave
//! a source tree behind. Jobs that die mid-flight (panel restart, OOM kill)
//! cannot run their destructor; those directories are picked up by
//! [`sweep_orphaned_staging`] at panel start and at the end of the next job,
//! once they are older than the grace period and no process still uses them.
//!
//! Never touches `/var/lib/cpn` (MFA, accounts, sites) or `/etc/cpn`.

use rand::{Rng, distr::Alphanumeric};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Root for CPN staging directories.
pub const STAGING_ROOT: &str = "/var/tmp";

/// Directory name prefixes CPN creates under [`STAGING_ROOT`] for maintenance.
pub const STAGING_DIR_PREFIXES: &[&str] = &[
    "cpn-tip-",
    "cpn-upgrade-",
    "cpn-gpg-",
    "cpn-install-",
    "cpn-release-",
];

/// Single files CPN drops under [`STAGING_ROOT`] during maintenance.
pub const STAGING_FILE_NAMES: &[&str] = &["cpn-rustup-init.sh"];

/// Default age before an orphaned staging directory may be swept (hours).
pub const DEFAULT_GRACE_HOURS: u64 = 2;

/// Grace period from `CPN_STAGING_GRACE_HOURS` (hours, 0 allowed) or the default.
pub fn grace_period() -> Duration {
    let hours = std::env::var("CPN_STAGING_GRACE_HOURS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_GRACE_HOURS);
    Duration::from_secs(hours.saturating_mul(3600))
}

/// RAII staging directory: removed on drop unless [`StagingDir::keep`] was called.
pub struct StagingDir {
    path: PathBuf,
    keep: bool,
}

impl StagingDir {
    /// Create `/var/tmp/<prefix>-<random>` with mode 0700.
    pub fn create(prefix: &str) -> Result<Self, String> {
        let suffix: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        let path = PathBuf::from(format!("{STAGING_ROOT}/{prefix}-{suffix}"));
        fs::create_dir_all(&path)
            .map_err(|error| format!("Could not create temp dir {}: {error}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o700));
        }
        Ok(Self { path, keep: false })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Path of a file inside this staging dir.
    pub fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    /// Same as [`StagingDir::file`] but as a `String` for command arguments.
    pub fn file_string(&self, name: &str) -> String {
        self.file(name).to_string_lossy().into_owned()
    }

    /// Keep the directory on drop (debugging only; the next sweep removes it).
    #[allow(dead_code)]
    pub fn keep(&mut self) {
        self.keep = true;
    }

    /// Remove now and report what happened (idempotent).
    pub fn remove_now(&mut self) -> Option<String> {
        self.keep = true;
        if !self.path.exists() {
            return None;
        }
        match fs::remove_dir_all(&self.path) {
            Ok(()) => Some(format!("removed job staging dir {}", self.path.display())),
            Err(error) => Some(format!(
                "could not remove job staging dir {}: {error}",
                self.path.display()
            )),
        }
    }
}

impl Drop for StagingDir {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        if let Some(note) = self.remove_now() {
            crate::upgrade_tip_log::log_info(note);
        }
    }
}

fn is_staging_dir_name(name: &str) -> bool {
    STAGING_DIR_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

fn is_staging_file_name(name: &str) -> bool {
    STAGING_FILE_NAMES.contains(&name)
}

/// True when the path (or anything below it) is still referenced by a live
/// process: cwd, exe, or an argument on its command line. Linux `/proc` only;
/// elsewhere returns false.
pub fn path_in_use_by_process(path: &Path) -> bool {
    let needle = path.to_string_lossy().into_owned();
    if needle.is_empty() {
        return false;
    }
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let pid = name.to_string_lossy();
        if !pid.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let base = entry.path();
        for link in ["cwd", "exe"] {
            if let Ok(target) = fs::read_link(base.join(link))
                && target.to_string_lossy().starts_with(&needle)
            {
                return true;
            }
        }
        if let Ok(cmdline) = fs::read(base.join("cmdline"))
            && String::from_utf8_lossy(&cmdline).contains(&needle)
        {
            return true;
        }
    }
    false
}

fn age_of(path: &Path) -> Option<Duration> {
    let meta = fs::symlink_metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}

/// Outcome of one sweep (paths are logged by the caller).
#[derive(Debug, Default, Clone)]
pub struct SweepReport {
    pub removed: Vec<String>,
    pub kept_in_use: Vec<String>,
    pub kept_young: Vec<String>,
    pub errors: Vec<String>,
}

impl SweepReport {
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for path in &self.removed {
            lines.push(format!("staging sweep removed: {path}"));
        }
        for path in &self.kept_in_use {
            lines.push(format!("staging sweep kept (in use by a process): {path}"));
        }
        for path in &self.errors {
            lines.push(format!("staging sweep: {path}"));
        }
        if self.removed.is_empty() && self.kept_in_use.is_empty() {
            lines.push("staging sweep: no orphaned CPN staging dirs under /var/tmp".into());
        }
        lines
    }
}

fn remove_entry(path: &Path, report: &mut SweepReport) {
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => report.removed.push(path.display().to_string()),
        Err(error) => report
            .errors
            .push(format!("could not remove {}: {error}", path.display())),
    }
}

/// Remove orphaned CPN staging dirs/files under `/var/tmp` that are older than
/// `min_age` and not referenced by any live process. `exclude` (the current
/// job's own dir) is always skipped; the job guard removes that one itself.
pub fn sweep_orphaned_staging(min_age: Duration, exclude: Option<&Path>) -> SweepReport {
    let mut report = SweepReport::default();
    if cfg!(windows) {
        return report;
    }
    let Ok(entries) = fs::read_dir(STAGING_ROOT) else {
        return report;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_dir = path.is_dir();
        let matches = if is_dir {
            is_staging_dir_name(&name)
        } else {
            is_staging_file_name(&name)
        };
        if !matches {
            continue;
        }
        if exclude.is_some_and(|ex| ex == path) {
            continue;
        }
        if age_of(&path).is_some_and(|age| age < min_age) {
            report.kept_young.push(path.display().to_string());
            continue;
        }
        if path_in_use_by_process(&path) {
            report.kept_in_use.push(path.display().to_string());
            continue;
        }
        remove_entry(&path, &mut report);
    }
    report
}

/// Roots that hold per-commit cargo target dirs (`tip-<short sha>`).
fn tip_cargo_target_roots() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/home/cpn/cpn-cargo-target"),
        PathBuf::from("/var/tmp/cpn-cargo-target"),
    ]
}

/// Remove per-commit cargo build dirs from earlier commit upgrades. Keeps the
/// dir for `keep_sha` (the commit just installed) and anything a live cargo or
/// rustc still writes to. Shared `debug` / `release` dirs are never touched.
pub fn prune_tip_cargo_targets(keep_sha: Option<&str>, min_age: Duration) -> SweepReport {
    let mut report = SweepReport::default();
    if cfg!(windows) {
        return report;
    }
    let keep_leaf = keep_sha
        .map(crate::build_meta::short_sha)
        .filter(|short| !short.is_empty())
        .map(|short| format!("tip-{short}"));
    for root in tip_cargo_target_roots() {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !path.is_dir() || !name.starts_with("tip-") {
                continue;
            }
            if keep_leaf.as_deref() == Some(name.as_ref()) {
                continue;
            }
            if age_of(&path).is_some_and(|age| age < min_age) {
                report.kept_young.push(path.display().to_string());
                continue;
            }
            if path_in_use_by_process(&path) {
                report.kept_in_use.push(path.display().to_string());
                continue;
            }
            remove_entry(&path, &mut report);
        }
    }
    report
}

/// Panel start: sweep orphans left by a job that died with the previous
/// process (for example a commit build interrupted by the upgrade restart).
/// Keeps the cargo dir of the running binary so a Repair of the same commit
/// stays fast.
pub fn sweep_on_startup() -> Vec<String> {
    let grace = grace_period();
    let mut lines = sweep_orphaned_staging(grace, None).summary_lines();
    let running_sha =
        crate::build_meta::sha_embedded_in_binary(Path::new(crate::manifest::installer_bin()));
    let targets = prune_tip_cargo_targets(running_sha.as_deref(), grace);
    for path in &targets.removed {
        lines.push(format!(
            "staging sweep removed old commit build dir: {path}"
        ));
    }
    for path in &targets.kept_in_use {
        lines.push(format!(
            "staging sweep kept commit build dir (in use): {path}"
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_cover_tip_and_package_staging() {
        assert!(is_staging_dir_name("cpn-tip-tA0gzpiwiCpx"));
        assert!(is_staging_dir_name("cpn-upgrade-abc123"));
        assert!(is_staging_dir_name("cpn-gpg-1"));
        assert!(!is_staging_dir_name("cpn-cargo-target"));
        assert!(!is_staging_dir_name("dnf-cpn-fpde1pmt"));
        assert!(!is_staging_dir_name("systemd-private-x"));
        assert!(is_staging_file_name("cpn-rustup-init.sh"));
    }

    #[test]
    fn grace_defaults_to_two_hours() {
        // Env may be set by an operator shell; only assert the default path.
        if std::env::var("CPN_STAGING_GRACE_HOURS").is_err() {
            assert_eq!(grace_period(), Duration::from_secs(2 * 3600));
        }
    }

    #[test]
    fn sweep_never_lists_cpn_data_paths() {
        let report = sweep_orphaned_staging(Duration::from_secs(0), None);
        for path in report.removed.iter().chain(report.errors.iter()) {
            assert!(!path.contains("/var/lib/cpn"), "{path}");
            assert!(!path.contains("/etc/cpn"), "{path}");
            assert!(!path.contains("/home/"), "{path}");
        }
    }

    #[test]
    fn summary_has_no_dashes_and_mentions_sweep() {
        let report = SweepReport::default();
        for line in report.summary_lines() {
            assert!(line.contains("staging sweep"));
            assert!(!line.contains('\u{2014}'));
            assert!(!line.contains('\u{2013}'));
        }
    }

    #[cfg(unix)]
    #[test]
    fn staging_dir_guard_removes_on_drop() {
        if !Path::new(STAGING_ROOT).is_dir() {
            return;
        }
        let path = {
            let Ok(dir) = StagingDir::create("cpn-release") else {
                return;
            };
            fs::write(dir.file("probe.txt"), "x").expect("write probe");
            assert!(dir.path().is_dir());
            dir.path().to_path_buf()
        };
        assert!(
            !path.exists(),
            "guard must remove {} on drop",
            path.display()
        );
    }
}
