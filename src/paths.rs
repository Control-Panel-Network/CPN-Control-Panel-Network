//! Platform-aware filesystem roots for CPN data and binaries.
//!
//! Override the data root with `CPN_DATA_DIR` (tests and non-standard installs).

use std::{
    env,
    path::{Path, PathBuf},
};

/// Unix default data directory.
pub const UNIX_DATA_DIR: &str = "/var/lib/cpn";

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
}
