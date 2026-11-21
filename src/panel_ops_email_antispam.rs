//! SpamAssassin / Rspamd / MailScanner detect, status, and admin enable/install.

use crate::apps_pkg::{enable_now, install_packages_dnf_or_apt, rpm_or_dpkg_installed};
use crate::panel_ops_security::{cmd_stdout, systemctl_active, which_exists};
use crate::service_detect::systemd_unit_active;

#[derive(Debug, Clone)]
pub struct FilterStatus {
    pub name: &'static str,
    pub installed: bool,
    pub active: bool,
    pub detail: String,
    pub web_ui: Option<String>,
    pub package_hint: &'static str,
}

fn unit_active_any(units: &[&str]) -> bool {
    units
        .iter()
        .any(|u| systemd_unit_active(u) || systemctl_active(u) == "active")
}

pub fn spamassassin_status() -> FilterStatus {
    let bins = which_exists("spamassassin")
        || which_exists("spamd")
        || which_exists("spamc")
        || path_exists("/etc/mail/spamassassin")
        || path_exists("/etc/spamassassin");
    let pkgs = rpm_or_dpkg_installed(&["spamassassin"]);
    let installed = bins || pkgs;
    let active = unit_active_any(&["spamassassin", "spamd"]);
    let version = cmd_stdout("spamassassin", &["--version"]).unwrap_or_default();
    let detail = if !installed {
        "SpamAssassin is not installed on this host.".into()
    } else if active {
        format!(
            "Installed and active. {}",
            if version.is_empty() {
                String::new()
            } else {
                version.lines().next().unwrap_or("").to_string()
            }
        )
    } else {
        "Installed but service is not active. Use Enable to start spamassassin/spamd.".into()
    };
    FilterStatus {
        name: "SpamAssassin",
        installed,
        active,
        detail,
        web_ui: None,
        package_hint: "spamassassin",
    }
}

pub fn rspamd_status() -> FilterStatus {
    let installed =
        which_exists("rspamd") || rpm_or_dpkg_installed(&["rspamd"]) || path_exists("/etc/rspamd");
    let active = unit_active_any(&["rspamd"]);
    let web = if path_exists("/etc/rspamd") || installed {
        Some("http://127.0.0.1:11334/".into())
    } else {
        None
    };
    let detail = if !installed {
        "Rspamd is not installed on this host.".into()
    } else if active {
        "Installed and active. Worker web UI is typically on port 11334 (localhost).".into()
    } else {
        "Installed but not active. Use Enable to start rspamd.".into()
    };
    FilterStatus {
        name: "Rspamd",
        installed,
        active,
        detail,
        web_ui: web,
        package_hint: "rspamd",
    }
}

pub fn mailscanner_status() -> FilterStatus {
    let installed = which_exists("MailScanner")
        || which_exists("mailscanner")
        || rpm_or_dpkg_installed(&["MailScanner", "mailscanner"])
        || path_exists("/etc/MailScanner");
    let active = unit_active_any(&["mailscanner", "MailScanner"]);
    let detail = if !installed {
        "MailScanner is not available on this host (common on AlmaLinux 9). This page stays LIVE for detection and enable when a package exists."
            .into()
    } else if active {
        "MailScanner is installed and active.".into()
    } else {
        "MailScanner package detected but not active.".into()
    };
    FilterStatus {
        name: "MailScanner",
        installed,
        active,
        detail,
        web_ui: None,
        package_hint: "MailScanner",
    }
}

fn path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

pub fn enable_spamassassin() -> Result<String, String> {
    let st = spamassassin_status();
    if !st.installed {
        install_packages_dnf_or_apt(&["spamassassin"], &["spamassassin"])?;
    }
    if let Err(e) = enable_now(&["spamassassin"]) {
        enable_now(&["spamd"]).map_err(|_| e)?;
    }
    Ok("SpamAssassin enable requested.".into())
}

pub fn enable_rspamd() -> Result<String, String> {
    let st = rspamd_status();
    if !st.installed {
        install_packages_dnf_or_apt(&["rspamd"], &["rspamd"])?;
    }
    let heal = heal_rspamd_for_auth_results().unwrap_or_default();
    enable_now(&["rspamd"])?;
    // Re-wire Postfix milters so smtpd includes rspamd after start.
    let milters = crate::panel_ops_opendkim::ensure_postfix_opendkim_milters()
        .unwrap_or_else(|e| format!("milter wire skipped: {e}"));
    Ok(format!("Rspamd enable requested. {heal} {milters}"))
}

/// Ensure rspamd can finish milter EOM and emit Authentication-Results (Tachyon Auth UI).
/// Does not print Redis secrets. Safe to call on heal/enable.
pub fn heal_rspamd_for_auth_results() -> Result<String, String> {
    use std::fs;
    use std::path::Path;
    let local = Path::new("/etc/rspamd/local.d");
    if !local.is_dir() {
        return Ok("rspamd local.d missing (package not fully installed)".into());
    }
    let mut notes: Vec<String> = Vec::new();

    // Authentication-Results for webmail; do not stall on local/auth skips.
    let milter_headers = local.join("milter_headers.conf");
    let want_headers = concat!(
        "# CPN: emit Auth-Results for Tachyon/SnappyMail\n",
        "use = [\"authentication-results\", \"x-spamd-result\", \"x-rspamd-action\"];\n",
        "skip_local = false;\n",
        "skip_authenticated = false;\n"
    );
    if fs::read_to_string(&milter_headers).unwrap_or_default() != want_headers {
        fs::write(&milter_headers, want_headers)
            .map_err(|e| format!("write milter_headers.conf: {e}"))?;
        notes.push("milter_headers".into());
    }

    // Avoid milter timeout when clamav module is enabled but clamd is down.
    let antivirus = local.join("antivirus.conf");
    let clam_up = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], 3310)),
        std::time::Duration::from_millis(200),
    )
    .is_ok();
    if !clam_up {
        let disabled = "# CPN: clamd not on 127.0.0.1:3310; enabled scanners stall milter Auth-Results\nenabled = false;\n";
        if fs::read_to_string(&antivirus).unwrap_or_default() != disabled {
            fs::write(&antivirus, disabled).map_err(|e| format!("write antivirus.conf: {e}"))?;
            notes.push("antivirus disabled (clamd down)".into());
        }
    }

    // Wire Redis password from redis.conf without logging the secret.
    if let Some(pw) = read_redis_requirepass() {
        let redis_conf = local.join("redis.conf");
        let body = format!(
            "write_servers = \"127.0.0.1\";\nread_servers = \"127.0.0.1\";\npassword = \"{pw}\";\n"
        );
        let prev = fs::read_to_string(&redis_conf).unwrap_or_default();
        if prev != body {
            fs::write(&redis_conf, &body).map_err(|e| format!("write redis.conf: {e}"))?;
            // rspamd drops privileges; config must be group-readable by _rspamd/rspamd.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&redis_conf, fs::Permissions::from_mode(0o640));
            }
            for group in ["_rspamd", "rspamd"] {
                let _ = std::process::Command::new("chown")
                    .args([&format!("root:{group}")])
                    .arg(&redis_conf)
                    .status();
            }
            notes.push("redis password synced".into());
        }
    }

    let options = local.join("options.inc");
    let want_opts = "# CPN: allow SPF/DKIM/DMARC symbols to finish before milter EOM\ntask_timeout = 12s;\n";
    if fs::read_to_string(&options).unwrap_or_default() != want_opts {
        fs::write(&options, want_opts).map_err(|e| format!("write options.inc: {e}"))?;
        notes.push("task_timeout".into());
    }

    if notes.is_empty() {
        Ok("rspamd auth heal unchanged".into())
    } else {
        Ok(format!("rspamd auth heal: {}", notes.join(", ")))
    }
}

fn read_redis_requirepass() -> Option<String> {
    for path in [
        "/etc/redis/redis.conf",
        "/etc/redis.conf",
        "/etc/redis/redis.conf.d/cpn.conf",
    ] {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with('#') {
                continue;
            }
            let lower = t.to_ascii_lowercase();
            if !lower.starts_with("requirepass") {
                continue;
            }
            let mut parts = t.splitn(2, char::is_whitespace);
            let _ = parts.next();
            if let Some(rest) = parts.next() {
                let pw = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                if !pw.is_empty() {
                    return Some(pw);
                }
            }
        }
    }
    None
}

pub fn enable_mailscanner() -> Result<String, String> {
    let st = mailscanner_status();
    if !st.installed {
        return Err(
            "MailScanner package is not available for install on this OS. Status page remains available."
                .into(),
        );
    }
    if let Err(e1) = enable_now(&["mailscanner"]) {
        enable_now(&["MailScanner"]).map_err(|_| e1)?;
    }
    Ok("MailScanner enable requested.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_structs_construct() {
        let s = spamassassin_status();
        assert_eq!(s.name, "SpamAssassin");
        let r = rspamd_status();
        assert_eq!(r.name, "Rspamd");
        let m = mailscanner_status();
        assert_eq!(m.name, "MailScanner");
    }
}
