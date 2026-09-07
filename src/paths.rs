//! Platform-aware filesystem roots for CPN data and binaries.
//!
//! Override the data root with `CPN_DATA_DIR` (tests and non-standard installs).

use std::{
    env,
    path::{Path, PathBuf},
};

/// Unix default data directory (registry, bootstrap, catalog cache).
pub const UNIX_DATA_DIR: &str = "/var/lib/cpn";

/// Unix default panel home (panel-wide backups under `backups/`).
pub const UNIX_PANEL_HOME: &str = "/home/cpn-panel";

/// Windows default data directory (under ProgramData).
pub const WINDOWS_DATA_DIR: &str = r"C:\ProgramData\CPN";

/// Default installer binary path on Unix packaging.
pub const UNIX_INSTALLER_BIN: &str = "/usr/bin/cpn-installer";

/// Default CLI binary path on Unix packaging.
pub const UNIX_CLI_BIN: &str = "/usr/bin/cpn";

/// Default installer binary path on Windows packaging.
pub const WINDOWS_INSTALLER_BIN: &str = r"C:\Program Files\CPN\cpn-installer.exe";

/// Default CLI binary path on Windows packaging.
pub const WINDOWS_CLI_BIN: &str = r"C:\Program Files\CPN\cpn.exe";

/// Resolve the CPN data directory (`CPN_DATA_DIR` or platform default).
pub fn default_data_dir() -> PathBuf {
    if let Some(override_dir) = env::var_os("CPN_DATA_DIR") {
        return PathBuf::from(override_dir);
    }
    PathBuf::from(platform_data_dir())
}

/// Platform default without env override (useful for docs and help text).
pub fn platform_data_dir() -> &'static str {
    if cfg!(windows) {
        WINDOWS_DATA_DIR
    } else {
        UNIX_DATA_DIR
    }
}

pub fn installer_bin_path() -> &'static str {
    if cfg!(windows) {
        WINDOWS_INSTALLER_BIN
    } else {
        UNIX_INSTALLER_BIN
    }
}

pub fn cli_bin_path() -> &'static str {
    if cfg!(windows) {
        WINDOWS_CLI_BIN
    } else {
        UNIX_CLI_BIN
    }
}

/// Human-readable data dir hint for CLI help (respects `CPN_DATA_DIR` when set).
pub fn data_dir_display() -> String {
    default_data_dir().display().to_string()
}

pub fn join_data(relative: impl AsRef<Path>) -> PathBuf {
    default_data_dir().join(relative)
}

/// Panel operator home (`CPN_PANEL_HOME`, or `<CPN_SITES_HOME>/cpn-panel`, or `/home/cpn-panel`).
pub fn panel_home_dir() -> PathBuf {
    if let Some(override_dir) = env::var_os("CPN_PANEL_HOME") {
        return PathBuf::from(override_dir);
    }
    if let Ok(sites_home) = env::var("CPN_SITES_HOME") {
        let trimmed = sites_home.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("cpn-panel");
        }
    }
    if cfg!(windows) {
        PathBuf::from(r"C:\CPN\SitesHome\cpn-panel")
    } else {
        PathBuf::from(UNIX_PANEL_HOME)
    }
}

/// Panel-wide backup archives (`/home/cpn-panel/backups` by default).
pub fn panel_backups_dir() -> PathBuf {
    panel_home_dir().join("backups")
}

/// Legacy panel backup location under the data dir (pre home-path change).
pub fn legacy_panel_backups_dir() -> PathBuf {
    default_data_dir().join("backups")
}

/// Root-only bootstrap token for `--allow-remote` labs (not printed in logs).
pub const INSTALLER_BOOTSTRAP_TOKEN_FILE: &str = "installer-bootstrap.token";

pub fn installer_bootstrap_token_path() -> PathBuf {
    join_data(INSTALLER_BOOTSTRAP_TOKEN_FILE)
}

/// Persist the installer bootstrap token with mode 0600 on Unix.
///
/// `--allow-remote` deliberately omits the full token from stdout. Operators
/// (and NAT VirtualBox labs) read it from this path over SSH instead.
pub fn write_installer_bootstrap_token(token: &str) -> Result<PathBuf, String> {
    use std::fs;
    let dir = default_data_dir();
    fs::create_dir_all(&dir).map_err(|error| format!("create data dir: {error}"))?;
    let path = installer_bootstrap_token_path();
    let tmp = path.with_extension("token.tmp");
    fs::write(&tmp, token.as_bytes()).map_err(|error| format!("write token: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("chmod token: {error}"))?;
    }
    fs::rename(&tmp, &path).map_err(|error| format!("rename token: {error}"))?;
    Ok(path)
}

pub fn clear_installer_bootstrap_token() {
    let _ = std::fs::remove_file(installer_bootstrap_token_path());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_defaults_are_absolute_style() {
        let dir = platform_data_dir();
        assert!(!dir.is_empty());
        if cfg!(windows) {
            assert!(dir.contains("ProgramData"));
        } else {
            assert_eq!(dir, UNIX_DATA_DIR);
        }
    }

    #[test]
    fn bootstrap_token_path_is_under_data_dir() {
        let path = installer_bootstrap_token_path();
        assert!(path.ends_with(INSTALLER_BOOTSTRAP_TOKEN_FILE));
    }
}
