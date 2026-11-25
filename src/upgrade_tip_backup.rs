//! Snapshot `/usr/bin/cpn-installer` and `cpn` before tip apply; restore on failure.

use crate::manifest::{cli_bin, installer_bin};
use std::path::{Path, PathBuf};

const INSTALLER_BACKUP: &str = "cpn-installer.tip-rollback";
const CLI_BACKUP: &str = "cpn.tip-rollback";

#[derive(Debug, Clone, Default)]
pub struct TipBinaryBackup {
    pub dir: PathBuf,
    pub installer: Option<PathBuf>,
    pub cli: Option<PathBuf>,
    pub committed: bool,
}

impl TipBinaryBackup {
    pub fn rollback_dir() -> PathBuf {
        crate::paths::default_data_dir().join("rollback")
    }

    pub fn snapshot() -> Result<Self, String> {
        let dir = Self::rollback_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Could not create rollback dir {}: {e}", dir.display()))?;
        let installer_dst = dir.join(INSTALLER_BACKUP);
        let cli_dst = dir.join(CLI_BACKUP);
        let mut backup = TipBinaryBackup {
            dir,
            installer: None,
            cli: None,
            committed: false,
        };
        backup.installer = copy_if_exists(Path::new(installer_bin()), &installer_dst)?;
        backup.cli = copy_if_exists(Path::new(cli_bin()), &cli_dst)?;
        Ok(backup)
    }

    pub fn restore(&self) -> Result<(), String> {
        if self.committed {
            return Ok(());
        }
        let mut notes: Vec<String> = Vec::new();
        if let Some(ref path) = self.installer
            && restore_one(path, Path::new(installer_bin())).is_ok()
        {
            notes.push("restored cpn-installer".into());
        }
        if let Some(ref path) = self.cli
            && restore_one(path, Path::new(cli_bin())).is_ok()
        {
            notes.push("restored cpn CLI".into());
        }
        if !notes.is_empty() {
            crate::upgrade_tip_log::log_info(format!("Tip upgrade rollback: {}", notes.join(", ")));
        }
        Ok(())
    }

    pub fn commit_success(&mut self) {
        self.committed = true;
        let _ = std::fs::remove_file(self.dir.join(INSTALLER_BACKUP));
        let _ = std::fs::remove_file(self.dir.join(CLI_BACKUP));
    }
}

impl Drop for TipBinaryBackup {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn copy_if_exists(src: &Path, dst: &Path) -> Result<Option<PathBuf>, String> {
    if !src.is_file() {
        return Ok(None);
    }
    std::fs::copy(src, dst).map_err(|e| format!("Could not back up {}: {e}", src.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dst, std::fs::Permissions::from_mode(0o755));
    }
    Ok(Some(dst.to_path_buf()))
}

fn restore_one(backup: &Path, target: &Path) -> Result<(), String> {
    if !backup.is_file() {
        return Err("backup missing".into());
    }
    std::fs::copy(backup, target)
        .map_err(|e| format!("Could not restore {}: {e}", target.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_types_exist() {
        let _ = TipBinaryBackup::rollback_dir();
        assert!(!INSTALLER_BACKUP.contains('\u{2014}'));
    }
}
