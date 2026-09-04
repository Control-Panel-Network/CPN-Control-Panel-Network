//! Host security probes: firewall, sshd, fail2ban.

use crate::paths::default_data_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn cmd_ok(bin: &str, args: &[&str]) -> bool {
    Command::new(bin)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub(crate) fn cmd_stdout(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub(crate) fn which_exists(bin: &str) -> bool {
    cmd_ok("which", &[bin])
        || Path::new(&format!("/usr/sbin/{bin}")).exists()
        || Path::new(&format!("/usr/bin/{bin}")).exists()
}

pub(crate) fn systemctl_active(unit: &str) -> String {
    cmd_stdout("systemctl", &["is-active", unit]).unwrap_or_else(|| "unknown".into())
}

fn systemctl_enabled(unit: &str) -> String {
    cmd_stdout("systemctl", &["is-enabled", unit]).unwrap_or_else(|| "unknown".into())
}

#[derive(Debug, Clone)]
pub struct FirewallStatus {
    pub backend: String,
    pub active: bool,
    pub detail: String,
    pub services: Vec<String>,
    pub journal_excerpt: String,
}

pub fn firewall_status() -> FirewallStatus {
    let journal_path = default_data_dir().join("firewall-journal.txt");
    let journal_excerpt = fs::read_to_string(&journal_path)
        .map(|s| s.lines().take(12).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();

    if which_exists("firewall-cmd") {
        let state = cmd_stdout("firewall-cmd", &["--state"]).unwrap_or_else(|| "unknown".into());
        let active = state.eq_ignore_ascii_case("running");
        let services = cmd_stdout("firewall-cmd", &["--list-services"])
            .map(|s| s.split_whitespace().map(str::to_string).collect::<Vec<_>>())
            .unwrap_or_default();
        let list = cmd_stdout("firewall-cmd", &["--list-all"]).unwrap_or_default();
        let detail = if list.is_empty() {
            format!("firewalld state: {state}")
        } else {
            list.chars().take(1200).collect()
        };
        return FirewallStatus {
            backend: "firewalld".into(),
            active,
            detail,
            services,
            journal_excerpt,
        };
    }

    if which_exists("ufw") {
        let status = cmd_stdout("ufw", &["status"]).unwrap_or_else(|| "unknown".into());
        let active = status.to_lowercase().contains("active");
        return FirewallStatus {
            backend: "ufw".into(),
            active,
            detail: status.chars().take(1200).collect(),
            services: Vec::new(),
            journal_excerpt,
        };
    }

    if which_exists("iptables") {
        let rules = cmd_stdout("iptables", &["-L", "-n"])
            .unwrap_or_else(|| "iptables present but could not list rules (may need root)".into());
        return FirewallStatus {
            backend: "iptables".into(),
            active: true,
            detail: rules.chars().take(1200).collect(),
            services: Vec::new(),
            journal_excerpt,
        };
    }

    FirewallStatus {
        backend: "none".into(),
        active: false,
        detail: "No firewalld, ufw, or iptables binary detected on PATH.".into(),
        services: Vec::new(),
        journal_excerpt,
    }
}

#[derive(Debug, Clone)]
pub struct SshdStatus {
    pub config_path: String,
    pub present: bool,
    pub permit_root_login: String,
    pub password_authentication: String,
    pub unit_active: String,
}

fn sshd_config_path() -> PathBuf {
    PathBuf::from("/etc/ssh/sshd_config")
}

fn parse_sshd_directive(raw: &str, key: &str) -> String {
    let key_lower = key.to_ascii_lowercase();
    let mut value = "default (unset)".to_string();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        if name.eq_ignore_ascii_case(&key_lower) {
            value = parts.next().unwrap_or("").to_string();
        }
    }
    value
}

pub fn sshd_status() -> SshdStatus {
    let path = sshd_config_path();
    let present = path.is_file();
    let (permit_root_login, password_authentication) = if present {
        match fs::read_to_string(&path) {
            Ok(raw) => (
                parse_sshd_directive(&raw, "PermitRootLogin"),
                parse_sshd_directive(&raw, "PasswordAuthentication"),
            ),
            Err(_) => ("unreadable".into(), "unreadable".into()),
        }
    } else {
        ("missing".into(), "missing".into())
    };
    SshdStatus {
        config_path: path.display().to_string(),
        present,
        permit_root_login,
        password_authentication,
        unit_active: systemctl_active("sshd"),
    }
}

/// Safe sshd_config toggles with backup. Only allowlisted keys/values.
pub fn apply_sshd_toggle(key: &str, value: &str) -> Result<String, String> {
    let key = key.trim();
    let value = value.trim();
    let allowed = match key {
        "PermitRootLogin" => matches!(
            value,
            "no" | "prohibit-password" | "without-password" | "yes"
        ),
        "PasswordAuthentication" => matches!(value, "yes" | "no"),
        _ => false,
    };
    if !allowed {
        return Err(format!(
            "Refused sshd change: key `{key}` / value `{value}` is not allowlisted"
        ));
    }
    let path = sshd_config_path();
    if !path.is_file() {
        return Err("sshd_config not found".into());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("Cannot read sshd_config: {e}"))?;
    let backup_dir = default_data_dir().join("security-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| format!("Cannot create backup dir: {e}"))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = backup_dir.join(format!("sshd_config.{stamp}.bak"));
    fs::write(&backup, &raw).map_err(|e| format!("Cannot write backup: {e}"))?;

    let mut found = false;
    let mut out_lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        let is_match = !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && trimmed
                .split_whitespace()
                .next()
                .map(|n| n.eq_ignore_ascii_case(key))
                .unwrap_or(false);
        if is_match {
            out_lines.push(format!("{key} {value}"));
            found = true;
        } else {
            out_lines.push(line.to_string());
        }
    }
    if !found {
        out_lines.push(format!("{key} {value}"));
    }
    let mut new_raw = out_lines.join("\n");
    if !new_raw.ends_with('\n') {
        new_raw.push('\n');
    }
    fs::write(&path, &new_raw).map_err(|e| format!("Cannot write sshd_config: {e}"))?;

    if which_exists("sshd") {
        let test = Command::new("sshd").args(["-t"]).output();
        if let Ok(out) = test
            && !out.status.success()
        {
            let _ = fs::write(&path, &raw);
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "sshd -t failed; restored previous config: {}",
                err.trim()
            ));
        }
    }

    let reload = Command::new("systemctl").args(["reload", "sshd"]).output();
    let reload_note = match reload {
        Ok(o) if o.status.success() => "sshd reloaded".to_string(),
        Ok(o) => format!(
            "config saved; reload manually (systemctl reload sshd): {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => format!("config saved; could not reload sshd: {e}"),
    };
    Ok(format!(
        "Set {key}={value}. Backup: {}. {reload_note}",
        backup.display()
    ))
}

#[derive(Debug, Clone)]
pub struct Fail2banStatus {
    pub installed: bool,
    pub active: String,
    pub enabled: String,
    pub jails: Vec<String>,
    pub detail: String,
}

pub fn fail2ban_status() -> Fail2banStatus {
    let installed = which_exists("fail2ban-client") || which_exists("fail2ban-server");
    if !installed {
        return Fail2banStatus {
            installed: false,
            active: "not-installed".into(),
            enabled: "not-installed".into(),
            jails: Vec::new(),
            detail: "fail2ban is not installed. Install with dnf/apt (fail2ban), then return here."
                .into(),
        };
    }
    let active = systemctl_active("fail2ban");
    let enabled = systemctl_enabled("fail2ban");
    let status = cmd_stdout("fail2ban-client", &["status"]).unwrap_or_default();
    let mut jails = Vec::new();
    for line in status.lines() {
        if let Some(rest) = line.split("Jail list:").nth(1) {
            for j in rest.split(',') {
                let t = j.trim();
                if !t.is_empty() {
                    jails.push(t.to_string());
                }
            }
        }
    }
    Fail2banStatus {
        installed: true,
        active,
        enabled,
        jails,
        detail: if status.is_empty() {
            "fail2ban-client status returned no output (service may be stopped).".into()
        } else {
            status.chars().take(1200).collect()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sshd_last_wins() {
        let raw = "# PermitRootLogin yes\nPermitRootLogin prohibit-password\nPermitRootLogin no\n";
        assert_eq!(parse_sshd_directive(raw, "PermitRootLogin"), "no");
    }

    #[test]
    fn parse_sshd_unset() {
        assert_eq!(
            parse_sshd_directive("Port 22\n", "PasswordAuthentication"),
            "default (unset)"
        );
    }

    #[test]
    fn sshd_toggle_rejects_unknown() {
        let err = apply_sshd_toggle("AllowUsers", "root").unwrap_err();
        assert!(err.contains("allowlisted"));
    }
}
