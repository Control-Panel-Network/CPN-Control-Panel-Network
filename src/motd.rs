//! Install and print CPN SSH/login MOTD and short panel-ready banners.
//!
//! English-only MOTD for interactive SSH logins. Never prints passwords or
//! bootstrap tokens. Product branding is CPN / Control Panel Network only.
//! Login URLs are resolved live each login (see `packaging/cpn-motd.sh` and
//! `cpn panel url`), not baked as a static `/etc/motd` at install time.

use crate::listen_port::{DEFAULT_PORT, load_preferred_listen_port};
use crate::panel_access_mode::{
    domain_only_motd_note, ip_backup_http_login_url, load_panel_access_mode, PanelAccessMode,
};
use crate::panel_login_facts::load_display_server_public_ipv4;
use crate::panel_network::load_panel_hostname;
use crate::panel_public_url::{is_loopback_login_base, load_panel_public_url, local_listen_base_url};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Embedded profile.d script (source of truth: `packaging/cpn-motd.sh`).
const MOTD_SCRIPT: &str = include_str!("../packaging/cpn-motd.sh");

/// Primary install path on Unix hosts.
pub const PROFILE_D_MOTD_PATH: &str = "/etc/profile.d/cpn-motd.sh";

/// Optional library copy for packaging and operators.
pub const LIB_MOTD_PATH: &str = "/usr/lib/cpn/cpn-motd.sh";

/// Legacy CyberPanel login banner (`/etc/profile.d/cyberpanel.sh`).
const CYBERPANEL_PROFILE_D_MOTD: &str = "/etc/profile.d/cyberpanel.sh";
const CYBERPANEL_MOTD_DISABLED_SUFFIX: &str = ".disabled-by-cpn";

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
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create directory {}: {error}", parent.display()))?;
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

/// Rename CyberPanel's profile.d MOTD so only CPN's banner runs on SSH login.
///
/// Best-effort: no error if already disabled or missing. Does not remove CyberPanel itself.
fn disable_legacy_cyberpanel_motd() {
    let active = PathBuf::from(CYBERPANEL_PROFILE_D_MOTD);
    if !active.is_file() {
        return;
    }
    let disabled = PathBuf::from(format!(
        "{CYBERPANEL_PROFILE_D_MOTD}{CYBERPANEL_MOTD_DISABLED_SUFFIX}"
    ));
    if disabled.is_file() {
        let _ = fs::remove_file(&active);
        return;
    }
    if fs::rename(&active, &disabled).is_err() {
        let _ = fs::remove_file(&active);
    }
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
    disable_legacy_cyberpanel_motd();
    Ok(written)
}

/// Ensure MOTD files exist (upgrade / re-start self-heal). Ignores non-root.
pub fn ensure_motd_installed() {
    match install_motd() {
        Ok(paths) if !paths.is_empty() => {
            crate::panel_login_facts::sync_public_login_facts();
        }
        Ok(_) => {
            crate::panel_login_facts::sync_public_login_facts();
        }
        Err(error) => {
            eprintln!("cpn-installer: could not install login MOTD: {error}");
        }
    }
}

/// Build short English "panel ready" lines (no secrets).
/// Prefer live `panel_public_url`, then hostname, then loopback listen port.
pub fn panel_ready_lines(version: &str, port: u16, hostname: Option<&str>) -> Vec<String> {
    let port = if port == 0 { DEFAULT_PORT } else { port };
    let public = load_panel_public_url();
    let host = hostname
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(load_panel_hostname);
    let server_ip = load_display_server_public_ipv4();
    let mut lines = vec![
        "------------------------------------------------------------".into(),
        format!("CPN panel ready (v{version})"),
        format!("Server IP: {server_ip}"),
        format!("Listen port: {port}"),
    ];
    let primary_base = if let Some(ref url) = public {
        url.trim_end_matches('/').to_string()
    } else if let Some(ref host) = host {
        format!("https://{host}")
    } else {
        local_listen_base_url(port)
    };
    if is_loopback_login_base(&primary_base) {
        lines.push("Login URL: (set hostname HTTPS: cpn network set-hostname)".into());
        lines.push(format!(
            "HTTPS: set a hostname (cpn network set-hostname) for reverse proxy on 443; never https://IP:{port}"
        ));
        lines.push(format!(
            "Admin tunnel: ssh -L {port}:127.0.0.1:{port} user@host (not shown as Login URL)"
        ));
        lines.push(format!(
            "VirtualBox NAT: host may forward 2089->guest {port} for lab HTTP on the host"
        ));
    } else if let Some(ref url) = public {
        lines.push(format!("Login URL: {url}/login"));
        lines.push(format!("Access mode: {}", load_panel_access_mode().label()));
        if let Some(backup) = ip_backup_http_login_url(port) {
            lines.push(format!("IP backup (HTTP only): {backup}"));
        } else if load_panel_access_mode() == PanelAccessMode::DomainOnly {
            lines.push(domain_only_motd_note(port));
        }
        lines.push(
            "Lab tip: Windows host may use the public URL when NAT forwards the guest port".into(),
        );
    } else if let Some(ref host) = host {
        lines.push(format!("Login URL: https://{host}/login"));
        lines.push(format!("Access mode: {}", load_panel_access_mode().label()));
        if let Some(backup) = ip_backup_http_login_url(port) {
            lines.push(format!("IP backup (HTTP only): {backup}"));
        } else if load_panel_access_mode() == PanelAccessMode::DomainOnly {
            lines.push(domain_only_motd_note(port));
        }
    }
    lines.push("Panel service: systemctl status cpn-installer.service".into());
    lines.push("Show URL anytime: cpn panel url".into());
    lines.push("Start again: sudo systemctl start cpn-installer.service".into());
    lines.push("Or foreground: sudo cpn-installer --web".into());
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
    use crate::account::with_test_data_dir;
    use crate::listen_port::save_preferred_listen_port;
    use crate::panel_network::clear_panel_hostname;
    use crate::panel_public_url::{clear_panel_public_url, save_panel_public_url};

    #[test]
    fn panel_ready_omits_loopback_login_and_shows_server_ip() {
        with_test_data_dir(|| {
            clear_panel_public_url().unwrap();
            clear_panel_hostname().unwrap();
            let lines = panel_ready_lines("1.0.0", 2087, None);
            let joined = lines.join("\n");
            assert!(joined.contains("v1.0.0"));
            assert!(joined.contains("Server IP:"));
            assert!(joined.contains("set hostname HTTPS"));
            assert!(!joined.contains("Login URL: http://127.0.0.1"));
            assert!(joined.contains("never https://IP:2087"));
            assert!(joined.contains("cpn-installer.service"));
            assert!(joined.contains("cpn panel url"));
            assert!(joined.contains("2089"));
            assert!(!joined.to_lowercase().contains("cyberpanel"));
            assert!(!joined.contains("password"));
        });
    }

    #[test]
    fn panel_ready_prefers_hostname_https() {
        with_test_data_dir(|| {
            clear_panel_public_url().unwrap();
            let lines = panel_ready_lines("1.0.0", 2089, Some("panel.example.com"));
            let joined = lines.join("\n");
            assert!(joined.contains("https://panel.example.com/login"));
            assert!(!joined.contains("Login URL: http://127.0.0.1"));
            assert!(joined.contains("domain-only"));
        });
    }

    #[test]
    fn panel_ready_hides_loopback_nat_public_url() {
        with_test_data_dir(|| {
            save_preferred_listen_port(2087).unwrap();
            save_panel_public_url("http://127.0.0.1:2089").unwrap();
            let lines = panel_ready_lines("0.2.6", 2087, None);
            let joined = lines.join("\n");
            assert!(!joined.contains("Login URL: http://127.0.0.1"));
            assert!(joined.contains("set hostname HTTPS"));
            clear_panel_public_url().unwrap();
        });
    }

    #[test]
    fn motd_script_is_cpn_branded_and_live() {
        let lower = MOTD_SCRIPT.to_ascii_lowercase();
        assert!(MOTD_SCRIPT.contains("Control Panel Network"));
        assert!(MOTD_SCRIPT.contains("News Targeted"));
        assert!(MOTD_SCRIPT.contains("cpn panel url --motd"));
        assert!(MOTD_SCRIPT.contains("listen_port"));
        assert!(MOTD_SCRIPT.contains("panel_public_url"));
        assert!(MOTD_SCRIPT.contains("/etc/cpn"));
        assert!(MOTD_SCRIPT.contains("This server has installed CPN"));
        assert!(MOTD_SCRIPT.contains("Forum         : https://discord.gg/nx9Kzrk"));
        assert!(MOTD_SCRIPT.contains("Support       : https://discord.gg/nx9Kzrk"));
        assert!(MOTD_SCRIPT.contains("cpn-installer.service"));
        assert!(!lower.contains("cyberpanel"));
        assert!(!lower.contains("enjoy your accelerated"));
        assert!(!MOTD_SCRIPT.contains('\u{2014}'));
        assert!(!MOTD_SCRIPT.contains('\u{2013}'));
        // CPN ASCII: N must keep its top and bottom underscores (match C/P weight).
        assert!(
            MOTD_SCRIPT.contains("____ ____  _   _"),
            "MOTD CPN banner missing N top underscore"
        );
        assert!(
            MOTD_SCRIPT.contains("|_|_\\_|") || MOTD_SCRIPT.contains("|_|_\\\\_|"),
            "MOTD CPN banner missing N bottom underscore connector"
        );
    }
}
