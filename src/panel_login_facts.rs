//! World-readable login URL facts for SSH MOTD and non-root `cpn panel url`.
//!
//! `$CPN_DATA_DIR` stays mode 700 with secrets at 600. Non-secret listen port,
//! public URL, and hostname are mirrored under `/etc/cpn/` (644) whenever the
//! panel/UI writes those prefs, so interactive SSH as a sudo user still sees
//! live URLs without reading secrets.

use crate::listen_port::{DEFAULT_PORT, load_preferred_listen_port};
use crate::panel_network::load_panel_hostname;
use crate::panel_public_url::load_panel_public_url;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Public (non-secret) mirror directory for MOTD / operator CLI.
pub const ETC_CPN_DIR: &str = "/etc/cpn";

fn etc_path(name: &str) -> PathBuf {
    PathBuf::from(ETC_CPN_DIR).join(name)
}

fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn write_world_readable(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o755));
        }
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

fn clear_world_readable(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("Could not remove {}: {error}", path.display()))?;
    }
    Ok(())
}

fn read_trimmed(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Mirror current live prefs into `/etc/cpn/` (root only; no-op otherwise).
pub fn sync_public_login_facts() {
    if cfg!(windows) || !is_root() {
        return;
    }
    let port = load_preferred_listen_port().unwrap_or(DEFAULT_PORT);
    let _ = write_world_readable(&etc_path("listen_port"), &format!("{port}\n"));
    match load_panel_public_url() {
        Some(url) => {
            let _ = write_world_readable(&etc_path("panel_public_url"), &format!("{url}\n"));
        }
        None => {
            let _ = clear_world_readable(&etc_path("panel_public_url"));
        }
    }
    match load_panel_hostname() {
        Some(host) => {
            let _ = write_world_readable(&etc_path("panel_hostname"), &format!("{host}\n"));
        }
        None => {
            let _ = clear_world_readable(&etc_path("panel_hostname"));
        }
    }
}

/// Listen port for display: data dir when readable, else `/etc/cpn/listen_port`.
pub fn load_display_listen_port() -> u16 {
    if let Some(port) = load_preferred_listen_port() {
        return port;
    }
    if let Some(raw) = read_trimmed(&etc_path("listen_port")) {
        if let Ok(port) = raw.parse::<u16>() {
            if port > 0 {
                return port;
            }
        }
    }
    DEFAULT_PORT
}

/// Public URL for display: data dir when readable, else `/etc/cpn/panel_public_url`.
pub fn load_display_panel_public_url() -> Option<String> {
    if let Some(url) = load_panel_public_url() {
        return Some(url);
    }
    let raw = read_trimmed(&etc_path("panel_public_url"))?;
    crate::panel_public_url::validate_panel_public_url(&raw).ok()
}

/// Hostname for display: data dir when readable, else `/etc/cpn/panel_hostname`.
pub fn load_display_panel_hostname() -> Option<String> {
    if let Some(host) = load_panel_hostname() {
        return Some(host);
    }
    let raw = read_trimmed(&etc_path("panel_hostname"))?;
    crate::panel_network::validate_panel_hostname(&raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::listen_port::save_preferred_listen_port;

    #[test]
    fn display_port_reads_data_dir_when_available() {
        with_test_data_dir(|| {
            save_preferred_listen_port(2111).unwrap();
            assert_eq!(load_display_listen_port(), 2111);
        });
    }
}
