//! Pre-migration backup: the data dir and (when they exist) both databases.
//!
//! Written under `/home/backup/cpn-panel-db/<stamp>/` with mode 0700/0600 and
//! verified before any migration step runs. A backup that cannot be verified
//! aborts the migration, so the host is never changed without a way back.

use super::sqlfmt::safe_name;
use crate::paths::default_data_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const BACKUP_ROOT: &str = "/home/backup/cpn-panel-db";

/// Cache and generated trees that add size but no state.
const TAR_EXCLUDES: &[&str] = &[
    "tmp",
    "cache",
    "site-previews",
    "theme-catalog-assets",
    "snapshots",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReport {
    pub dir: PathBuf,
    pub data_archive_bytes: u64,
    pub dumps: Vec<String>,
}

impl BackupReport {
    pub fn summary(&self) -> String {
        format!(
            "backup {} (data archive {} bytes, database dumps: {})",
            self.dir.display(),
            self.data_archive_bytes,
            if self.dumps.is_empty() {
                "none yet".to_string()
            } else {
                self.dumps.join(", ")
            }
        )
    }
}

fn private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|e| format!("Could not create {}: {}", path.display(), e.kind()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("Could not secure {}: {}", path.display(), e.kind()))?;
    }
    Ok(())
}

fn secure_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Archive the data dir; returns the verified archive size.
fn archive_data(data_dir: &Path, out: &Path) -> Result<u64, String> {
    let parent = data_dir.parent().ok_or("Data dir has no parent")?;
    let name = data_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Data dir has no name")?;
    let mut cmd = Command::new("tar");
    cmd.arg("-czf").arg(out);
    for ex in TAR_EXCLUDES {
        cmd.arg(format!("--exclude={name}/{ex}"));
    }
    cmd.arg("-C").arg(parent).arg(name);
    let status = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not run tar: {}", e.kind()))?;
    if !status.success() {
        return Err("Data directory archive failed".into());
    }
    secure_file(out);
    let listed = Command::new("tar")
        .arg("-tzf")
        .arg(out)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not verify archive: {}", e.kind()))?;
    let size = file_len(out);
    if !listed.success() || size == 0 {
        return Err("Data directory archive failed verification".into());
    }
    Ok(size)
}

fn dump_bin() -> Option<&'static str> {
    ["mariadb-dump", "mysqldump"].into_iter().find(|b| {
        Command::new(b)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Dump one database (administrative socket login, like every other CPN dump).
fn dump_db(db: &str, out: &Path) -> Result<(), String> {
    safe_name(db, 64)?;
    let bin = dump_bin().ok_or("mariadb-dump is not installed")?;
    let file = fs::File::create(out).map_err(|e| format!("Could not create dump: {}", e.kind()))?;
    secure_file(out);
    let status = Command::new(bin)
        .args([
            "--single-transaction",
            "--skip-lock-tables",
            "--routines",
            db,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not run {bin}: {}", e.kind()))?;
    if !status.success() || file_len(out) == 0 {
        let _ = fs::remove_file(out);
        return Err(format!("Dump of {db} failed"));
    }
    Ok(())
}

/// Back up the data dir plus every database in `existing_dbs`; verifies each artifact.
pub fn run(stamp: u64, existing_dbs: &[String]) -> Result<BackupReport, String> {
    run_at(
        Path::new(BACKUP_ROOT),
        &default_data_dir(),
        stamp,
        existing_dbs,
    )
}

pub fn run_at(
    root: &Path,
    data_dir: &Path,
    stamp: u64,
    existing_dbs: &[String],
) -> Result<BackupReport, String> {
    private_dir(root)?;
    let dir = root.join(format!("{stamp}"));
    private_dir(&dir)?;
    let size = if data_dir.is_dir() {
        archive_data(data_dir, &dir.join("var-lib-cpn.tar.gz"))?
    } else {
        0
    };
    let mut dumps = Vec::new();
    for db in existing_dbs {
        dump_db(db, &dir.join(format!("{db}.sql")))?;
        dumps.push(db.clone());
    }
    Ok(BackupReport {
        dir,
        data_archive_bytes: size,
        dumps,
    })
}

/// Keep the newest `keep` numeric backup directories; returns how many were removed.
pub fn prune(root: &Path, keep: usize) -> usize {
    let Ok(read) = fs::read_dir(root) else {
        return 0;
    };
    let mut stamps: Vec<(u64, PathBuf)> = read
        .flatten()
        .filter_map(|e| {
            let stamp = e.file_name().to_str()?.parse::<u64>().ok()?;
            e.path().is_dir().then(|| (stamp, e.path()))
        })
        .collect();
    stamps.sort_by_key(|(s, _)| *s);
    let excess = stamps.len().saturating_sub(keep);
    stamps
        .into_iter()
        .take(excess)
        .filter(|(_, p)| fs::remove_dir_all(p).is_ok())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_keeps_newest_numeric_dirs_only() {
        let root = std::env::temp_dir().join(format!("cpn-pdb-prune-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for s in ["1", "2", "3", "4"] {
            fs::create_dir_all(root.join(s)).unwrap();
        }
        fs::create_dir_all(root.join("manual-keep")).unwrap();
        assert_eq!(prune(&root, 2), 2);
        assert!(root.join("3").exists() && root.join("4").exists());
        assert!(!root.join("1").exists() && root.join("manual-keep").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn dump_refuses_hostile_database_names() {
        let out = std::env::temp_dir().join("cpn-pdb-never.sql");
        assert!(dump_db("x; DROP DATABASE mysql", &out).is_err());
        assert!(!out.exists());
    }

    #[test]
    fn excludes_cover_cache_trees_only() {
        for ex in TAR_EXCLUDES {
            assert!(!ex.contains("mfa") && !ex.contains("accounts") && !ex.contains("dkim"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn archive_is_created_private_and_verified() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("cpn-pdb-bk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let data = base.join("cpn");
        fs::create_dir_all(data.join("cache")).unwrap();
        fs::write(data.join("packages.json"), "{}").unwrap();
        fs::write(data.join("cache/x"), "skip").unwrap();
        let report = run_at(&base.join("backups"), &data, 42, &[]).unwrap();
        assert!(report.data_archive_bytes > 0);
        assert!(report.dumps.is_empty());
        let mode = fs::metadata(&report.dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let _ = fs::remove_dir_all(&base);
    }
}
