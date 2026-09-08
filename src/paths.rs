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

fn bootstrap_token_tmp_path(final_path: &Path) -> PathBuf {
    final_path.with_extension("token.tmp")
}

fn reject_symlink_or_existing(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("stat {}: {error}", path.display())),
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "Refusing to write through symlink at {}",
            path.display()
        )),
        Ok(_) => Err(format!(
            "Refusing to clobber existing path {}",
            path.display()
        )),
    }
}

fn remove_stale_regular_file(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("stat {}: {error}", path.display())),
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("Refusing to replace symlink at {}", path.display()))
        }
        Ok(meta) if meta.is_file() => {
            std::fs::remove_file(path).map_err(|error| format!("remove stale temp: {error}"))
        }
        Ok(_) => Err(format!(
            "Refusing to replace non-file path {}",
            path.display()
        )),
    }
}

/// Persist the installer bootstrap token with exclusive create + mode 0600 on Unix.
///
/// `--allow-remote` deliberately omits the full token from stdout. Operators
/// (and NAT VirtualBox labs) read it from this path over SSH instead.
pub fn write_installer_bootstrap_token(token: &str) -> Result<PathBuf, String> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let dir = default_data_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("create data dir: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }

    let path = installer_bootstrap_token_path();
    let tmp = bootstrap_token_tmp_path(&path);
    remove_stale_regular_file(&tmp)?;
    reject_symlink_or_existing(&tmp)?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    {
        let mut file = options
            .open(&tmp)
            .map_err(|error| format!("exclusive create token temp: {error}"))?;
        file.write_all(token.as_bytes())
            .map_err(|error| format!("write token: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync token: {error}"))?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&tmp)
            .map_err(|error| format!("stat token temp: {error}"))?
            .permissions()
            .mode()
            & 0o777;
        if mode != 0o600 {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!(
                "token temp mode was {mode:o}, expected 600 after exclusive create"
            ));
        }
    }

    // Replace any previous token file atomically; refuse if the destination is a symlink.
    if let Ok(meta) = std::fs::symlink_metadata(&path)
        && meta.file_type().is_symlink()
    {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Refusing to replace symlink at {}", path.display()));
    }
    std::fs::rename(&tmp, &path).map_err(|error| {
        let _ = std::fs::remove_file(&tmp);
        format!("rename token: {error}")
    })?;
    Ok(path)
}

/// Persist the bootstrap token. When `require_persisted` is true (`--allow-remote`),
/// failure aborts startup so the operator is never left without a retrievable credential.
pub fn persist_bootstrap_token_for_startup(
    token: &str,
    require_persisted: bool,
) -> Result<Option<PathBuf>, String> {
    match write_installer_bootstrap_token(token) {
        Ok(path) => Ok(Some(path)),
        Err(error) if require_persisted => Err(format!(
            "cannot start with --allow-remote: bootstrap token file was not persisted ({error})"
        )),
        Err(_error) => Ok(None),
    }
}

pub fn clear_installer_bootstrap_token() {
    let _ = std::fs::remove_file(installer_bootstrap_token_path());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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

    #[test]
    fn remote_startup_aborts_when_token_persist_fails() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let probe = std::env::temp_dir().join(format!("cpn-token-fail-{}", std::process::id()));
        let _ = std::fs::remove_file(&probe);
        std::fs::write(&probe, b"not-a-dir").expect("write probe");
        // SAFETY: single-threaded under ENV_LOCK for this process's CPN_DATA_DIR.
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &probe);
        }
        let result = persist_bootstrap_token_for_startup("secret-token", true);
        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
        }
        let _ = std::fs::remove_file(&probe);
        let err = result.expect_err("remote mode must abort when data dir is unusable");
        assert!(
            err.contains("cannot start with --allow-remote"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn local_startup_tolerates_token_persist_failure() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let probe =
            std::env::temp_dir().join(format!("cpn-token-local-fail-{}", std::process::id()));
        let _ = std::fs::remove_file(&probe);
        std::fs::write(&probe, b"not-a-dir").expect("write probe");
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &probe);
        }
        let result = persist_bootstrap_token_for_startup("secret-token", false);
        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
        }
        let _ = std::fs::remove_file(&probe);
        assert_eq!(result.expect("local mode continues"), None);
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_token_is_created_mode_0600_exclusively() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("cpn-token-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &dir);
        }
        let path = write_installer_bootstrap_token("unit-test-token").expect("write token");
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode() & 0o777;
        let body = std::fs::read_to_string(&path).expect("read");
        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(mode, 0o600);
        assert_eq!(body, "unit-test-token");
        assert!(path.ends_with(INSTALLER_BOOTSTRAP_TOKEN_FILE));
    }

    #[cfg(unix)]
    #[test]
    fn bootstrap_token_refuses_symlink_temp_clobber() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("cpn-token-symlink-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let final_path = dir.join(INSTALLER_BOOTSTRAP_TOKEN_FILE);
        let tmp = bootstrap_token_tmp_path(&final_path);
        let sink = dir.join("sink");
        std::fs::write(&sink, b"sink").expect("sink");
        std::os::unix::fs::symlink(&sink, &tmp).expect("symlink tmp");
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &dir);
        }
        let result = write_installer_bootstrap_token("nope");
        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
        }
        let _ = std::fs::remove_dir_all(&dir);
        let err = result.expect_err("must refuse symlink temp");
        assert!(
            err.contains("symlink") || err.contains("Refusing"),
            "unexpected error: {err}"
        );
    }
}
