//! firewalld heal for minimal installs: start firewalld with a safe baseline so
//! Security > Firewall is not stuck OFF, without ever locking the operator out.
//!
//! The heal is skipped when the operator stopped the firewall on purpose
//! (marker file under the CPN data dir) and when firewalld is not installed.

use crate::paths::default_data_dir;
use crate::service_detect::{systemd_unit_active, systemd_unit_file_exists};
use std::fs;
use std::path::PathBuf;

const SERVICE_START_TIMEOUT_SECS: u64 = 30;
const FIREWALL_CMD_TIMEOUT_SECS: u64 = 15;

/// Present while the operator has stopped the firewall from the panel.
pub fn operator_stop_marker() -> PathBuf {
    default_data_dir().join("firewall-operator-stopped")
}

pub fn mark_operator_stopped() {
    let path = operator_stop_marker();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, b"stopped by operator\n");
    set_private(&path);
}

pub fn clear_operator_stopped() {
    let _ = fs::remove_file(operator_stop_marker());
}

pub fn operator_stopped() -> bool {
    operator_stop_marker().exists()
}

fn set_private(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Ports from `Port N` lines in sshd_config (and drop-ins); defaults to 22.
pub fn parse_sshd_ports(raw: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(name) = parts.next() else { continue };
        if !name.eq_ignore_ascii_case("port") {
            continue;
        }
        if let Some(port) = parts.next().and_then(|p| p.parse::<u16>().ok())
            && port > 0
            && !ports.contains(&port)
        {
            ports.push(port);
        }
    }
    ports
}

pub fn sshd_listen_ports() -> Vec<u16> {
    let mut ports = Vec::new();
    let mut files = vec![PathBuf::from("/etc/ssh/sshd_config")];
    if let Ok(rd) = fs::read_dir("/etc/ssh/sshd_config.d") {
        let mut extra: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "conf"))
            .collect();
        extra.sort();
        files.extend(extra);
    }
    for file in files {
        if let Ok(raw) = fs::read_to_string(&file) {
            for port in parse_sshd_ports(&raw) {
                if !ports.contains(&port) {
                    ports.push(port);
                }
            }
        }
    }
    if ports.is_empty() {
        ports.push(22);
    }
    ports
}

/// Firewalld services that must stay reachable on a CPN host.
pub fn baseline_services() -> Vec<&'static str> {
    let mut services = vec!["ssh", "http", "https"];
    if systemd_unit_active("postfix") || systemd_unit_active("dovecot") {
        services.extend([
            "smtp",
            "smtps",
            "smtp-submission",
            "imap",
            "imaps",
            "pop3",
            "pop3s",
        ]);
    }
    services
}

/// Extra TCP ports (panel listen port, non-default sshd ports, ManageSieve).
pub fn baseline_ports() -> Vec<String> {
    let mut ports = vec![format!(
        "{}/tcp",
        crate::panel_ops_firewall::panel_listen_port()
    )];
    for port in sshd_listen_ports() {
        let spec = format!("{port}/tcp");
        if !ports.contains(&spec) {
            ports.push(spec);
        }
    }
    if systemd_unit_active("dovecot") {
        ports.push("4190/tcp".into());
    }
    ports
}

fn firewall_offline(args: &[&str]) -> bool {
    crate::panel_ops_security::cmd_ok_timeout(
        "firewall-offline-cmd",
        args,
        FIREWALL_CMD_TIMEOUT_SECS,
    )
}

/// Pre-seed the permanent config while firewalld is stopped, so the first start
/// never drops new connections to the panel port.
fn preseed_offline() {
    if !crate::panel_ops_security::which_exists("firewall-offline-cmd") {
        return;
    }
    for svc in baseline_services() {
        let _ = firewall_offline(&[&format!("--add-service={svc}")]);
    }
    for port in baseline_ports() {
        let _ = firewall_offline(&[&format!("--add-port={port}")]);
    }
}

/// Apply the baseline to a running firewalld (permanent + runtime).
pub fn apply_baseline_running() {
    for svc in baseline_services() {
        let add = format!("--add-service={svc}");
        let _ = crate::panel_ops_firewall::firewall_cmd_timed(&["--permanent", &add]);
        let _ = crate::panel_ops_firewall::firewall_cmd_timed(&[&add]);
    }
    for port in baseline_ports() {
        let add = format!("--add-port={port}");
        let _ = crate::panel_ops_firewall::firewall_cmd_timed(&["--permanent", &add]);
        let _ = crate::panel_ops_firewall::firewall_cmd_timed(&[&add]);
    }
}

/// Enable and start firewalld with the baseline; returns a short status message.
pub fn start_with_baseline() -> Result<String, String> {
    if !crate::panel_ops_security::which_exists("firewall-cmd") {
        return Err("firewalld is not installed".into());
    }
    if !systemd_unit_file_exists("firewalld") {
        return Err("firewalld unit file is missing; install the firewalld package".into());
    }
    preseed_offline();
    if !crate::panel_ops_security::cmd_ok_timeout(
        "systemctl",
        &["enable", "--now", "firewalld"],
        SERVICE_START_TIMEOUT_SECS,
    ) {
        return Err("Could not enable/start firewalld via systemctl".into());
    }
    // firewalld can report active a moment before D-Bus answers.
    for _ in 0..10 {
        if crate::panel_ops_firewall::firewall_cmd_timed(&["--state"]).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    apply_baseline_running();
    let _ = crate::panel_ops_firewall::firewall_cmd_timed(&["--reload"]);
    apply_baseline_running();
    Ok("Firewall started; panel, SSH, web and mail ports kept open".into())
}

/// Heal on panel start and after the Email stack starts: bring firewalld up on
/// minimal installs unless the operator stopped it on purpose.
pub fn heal_firewalld(reason: &str) -> Option<String> {
    if operator_stopped() || !crate::panel_ops_security::which_exists("firewall-cmd") {
        return None;
    }
    if !systemd_unit_file_exists("firewalld") {
        return None;
    }
    if systemd_unit_active("firewalld") {
        return None;
    }
    match start_with_baseline() {
        Ok(msg) => {
            crate::panel_ops_security::append_firewall_journal(&format!(
                "firewalld heal ok; reason={reason}; created=true; owner=cpn"
            ));
            eprintln!("[cpn] firewall heal ({reason}): {msg}");
            Some(msg)
        }
        Err(err) => {
            crate::panel_ops_security::append_firewall_journal(&format!(
                "firewalld heal failed; reason={reason}; created=false"
            ));
            eprintln!("[cpn] firewall heal ({reason}) failed: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sshd_ports() {
        let raw = "#Port 22\nPort 2222\n  port 22\nPort 2222\nPort notaport\n";
        assert_eq!(parse_sshd_ports(raw), vec![2222, 22]);
        assert!(parse_sshd_ports("# nothing\n").is_empty());
    }

    #[test]
    fn baseline_always_has_ssh_and_web() {
        let services = baseline_services();
        for needed in ["ssh", "http", "https"] {
            assert!(services.contains(&needed));
        }
    }
}
