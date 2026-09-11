//! Install and print CPN SSH/login MOTD and short panel-ready banners.
//!
//! English-only MOTD for interactive SSH logins. Never prints passwords or
//! bootstrap tokens. Product branding is CPN / Control Panel Network only.

use crate::listen_port::{DEFAULT_PORT, load_preferred_listen_port};
use crate::panel_network::load_panel_hostname;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Embedded profile.d script (source of truth: `packaging/cpn-motd.sh`).
const MOTD_SCRIPT: &str = include_str!("../packaging/cpn-motd.sh");

/// Primary install path on Unix hosts.
pub const PROFILE_D_MOTD_PATH: &str = "/etc/profile.d/cpn-motd.sh";

/// Optional library copy for packaging and operators.
pub const LIB_MOTD_PATH: &str = "/usr/lib/cpn/cpn-motd.sh";

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

fn write_executable(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o755);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

/// Install the dynamic MOTD into `/etc/profile.d` (and `/usr/lib/cpn` when possible).
///
/// No-op on Windows. Best-effort when not root (returns Ok with skipped note path).
pub fn install_motd() -> Result<Vec<PathBuf>, String> {
    if cfg!(windows) {
        return Ok(Vec::new());
    }
    if !is_root() {
        return Ok(Vec::new());
    }
    let mut written = Vec::new();
    let lib = PathBuf::from(LIB_MOTD_PATH);
    write_executable(&lib, MOTD_SCRIPT)?;
    written.push(lib);
    let profile = PathBuf::from(PROFILE_D_MOTD_PATH);
    write_executable(&profile, MOTD_SCRIPT)?;
    written.push(profile);
    Ok(written)
}

/// Ensure MOTD files exist (upgrade / re-start self-heal). Ignores non-root.
pub fn ensure_motd_installed() {
    match install_motd() {
        Ok(paths) if !paths.is_empty() => {
            // Quiet success: operators see the banner on next SSH login.
        }
        Ok(_) => {}
        Err(error) => {
            eprintln!("cpn-installer: could not install login MOTD: {error}");
        }
    }
}

/// Build short English "panel ready" lines (no secrets).
pub fn panel_ready_lines(version: &str, port: u16, hostname: Option<&str>) -> Vec<String> {
    let mut lines = vec![
        "------------------------------------------------------------".into(),
        format!("CPN panel ready (v{version})"),
        format!("Listen port: {port}"),
        format!("Local login: http://127.0.0.1:{port}/login"),
    ];
    if let Some(host) = hostname.map(str::trim).filter(|value| !value.is_empty()) {
        lines.push(format!("Hostname login: https://{host}/login"));
    } else {
        lines.push(format!(
            "Lab tip: ssh -L {port}:127.0.0.1:{port} user@host"
        ));
    }
    lines.push("Start again: sudo cpn-installer --web".into());
    lines.push("SSH/CLI mode: sudo cpn-installer --cli".into());
    lines.push("------------------------------------------------------------".into());
    lines
}

/// Print the short panel-ready summary to stdout.
pub fn print_panel_ready_banner(version: &str, port: u16, hostname: Option<&str>) {
    for line in panel_ready_lines(version, port, hostname) {
        println!("{line}");
    }
}

/// Resolve port/hostname from disk prefs and print the ready banner.
pub fn print_panel_ready_from_disk(version: &str, listen_port: u16) {
    let port = load_preferred_listen_port().unwrap_or(listen_port);
    let port = if port == 0 { DEFAULT_PORT } else { port };
    let hostname = load_panel_hostname();
    print_panel_ready_banner(version, port, hostname.as_deref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_ready_includes_local_url_and_version() {
        let lines = panel_ready_lines("1.0.0", 2087, None);
        let joined = lines.join("\n");
        assert!(joined.contains("v1.0.0"));
        assert!(joined.contains("http://127.0.0.1:2087/login"));
        assert!(joined.contains("cpn-installer --web"));
        assert!(!joined.to_lowercase().contains("cyberpanel"));
        assert!(!joined.contains("password"));
    }

    #[test]
    fn panel_ready_prefers_hostname_https() {
        let lines = panel_ready_lines("1.0.0", 2089, Some("panel.example.com"));
        let joined = lines.join("\n");
        assert!(joined.contains("https://panel.example.com/login"));
        assert!(joined.contains("http://127.0.0.1:2089/login"));
    }

    #[test]
    fn motd_script_is_cpn_branded() {
        let lower = MOTD_SCRIPT.to_ascii_lowercase();
        assert!(MOTD_SCRIPT.contains("CPN / Control Panel Network"));
        assert!(MOTD_SCRIPT.contains("News Targeted"));
        assert!(!lower.contains("cyberpanel"));
        assert!(!lower.contains("enjoy your accelerated"));
        assert!(!MOTD_SCRIPT.contains('\u{2014}'));
        assert!(!MOTD_SCRIPT.contains('\u{2013}'));
    }
}
