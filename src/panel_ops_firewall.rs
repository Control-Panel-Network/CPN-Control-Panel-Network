//! Host firewalld operations for the CPN firewall manager (no shell strings).

use crate::listen_port::{load_preferred_listen_port, DEFAULT_PORT};
use crate::panel_firewall_store::{
    self, FirewallManagerStore, FirewallRule, TrustedSource, ensure_protected_seeds, is_protected_ip,
    load_store, purge_expired_bans, save_store, validate_ip_or_cidr,
};
use crate::panel_host_info::host_sidebar_info;
use crate::panel_ops_security::{cmd_ok, firewall_status, which_exists};
use crate::panel_session::session_secret;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::process::Command;

type HmacSha256 = Hmac<Sha256>;

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn now_unix() -> u64 {
    panel_firewall_store::now_unix()
}

pub fn firewall_csrf_token(username: &str) -> String {
    let secret = session_secret(None);
    let hour = now_unix() / 3600;
    let payload = format!("firewall|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(&secret, &payload))
}

pub fn verify_firewall_csrf(username: &str, token: &str) -> bool {
    let secret = session_secret(None);
    let Some((hour_s, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(hour) = hour_s.parse::<u64>() else {
        return false;
    };
    let current = now_unix() / 3600;
    if hour + 2 < current || hour > current + 1 {
        return false;
    }
    let payload = format!("firewall|{username}|{hour}");
    let expected = hmac_hex(&secret, &payload);
    expected == sig
}

fn firewall_cmd(args: &[&str]) -> Result<String, String> {
    if !which_exists("firewall-cmd") {
        return Err("firewall-cmd not found".into());
    }
    let out = Command::new("firewall-cmd")
        .args(args)
        .output()
        .map_err(|e| format!("firewall-cmd failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !out.status.success() {
        let detail = if stderr.is_empty() { stdout } else { stderr };
        return Err(format!("firewall-cmd {}: {detail}", args.join(" ")));
    }
    Ok(stdout)
}

pub fn panel_listen_port() -> u16 {
    load_preferred_listen_port().unwrap_or(DEFAULT_PORT)
}

/// Ensure the panel listen port stays open (never lock operators out).
pub fn ensure_panel_port_open() -> Result<(), String> {
    let port = panel_listen_port();
    let port_arg = format!("{port}/tcp");
    let listed = firewall_cmd(&["--list-ports"]).unwrap_or_default();
    if listed.split_whitespace().any(|p| p == port_arg) {
        return Ok(());
    }
    let add = format!("--add-port={port_arg}");
    let _ = firewall_cmd(&["--permanent", &add]);
    let _ = firewall_cmd(&[&add]);
    Ok(())
}

pub fn start_firewall() -> Result<String, String> {
    if !which_exists("firewall-cmd") {
        return Err("firewalld is not installed".into());
    }
    if !cmd_ok("systemctl", &["enable", "--now", "firewalld"]) {
        return Err("Could not start firewalld".into());
    }
    ensure_panel_port_open()?;
    let _ = firewall_cmd(&["--permanent", "--add-service=http"]);
    let _ = firewall_cmd(&["--permanent", "--add-service=https"]);
    let _ = firewall_cmd(&["--permanent", "--add-service=ssh"]);
    let _ = firewall_cmd(&["--reload"]);
    ensure_panel_port_open()?;
    Ok("Firewall started and panel port kept open".into())
}

pub fn stop_firewall() -> Result<String, String> {
    if !cmd_ok("systemctl", &["stop", "firewalld"]) {
        return Err("Could not stop firewalld".into());
    }
    Ok("Firewall stopped. Panel remains reachable until you start it again.".into())
}

pub fn reload_firewall() -> Result<String, String> {
    firewall_cmd(&["--reload"])?;
    ensure_panel_port_open()?;
    Ok("Firewall reloaded; panel port verified open".into())
}

fn rich_drop_rule(ip: &str) -> Result<String, String> {
    let ip = validate_ip_or_cidr(ip)?;
    let family = if ip.contains(':') { "ipv6" } else { "ipv4" };
    Ok(format!(
        "rule family=\"{family}\" source address=\"{ip}\" drop"
    ))
}

pub fn apply_ban_to_host(ip: &str) -> Result<(), String> {
    let store = load_store();
    if is_protected_ip(&store, ip) {
        return Err(
            "Refused: cannot ban a trusted (never block) or protected server/admin IP".into(),
        );
    }
    let rule = rich_drop_rule(ip)?;
    let _ = firewall_cmd(&["--permanent", "--add-rich-rule", &rule]);
    let _ = firewall_cmd(&["--add-rich-rule", &rule]);
    Ok(())
}

pub fn remove_ban_from_host(ip: &str) -> Result<(), String> {
    let rule = rich_drop_rule(ip)?;
    let _ = firewall_cmd(&["--permanent", "--remove-rich-rule", &rule]);
    let _ = firewall_cmd(&["--remove-rich-rule", &rule]);
    Ok(())
}

fn apply_port_rule(rule: &FirewallRule) -> Result<(), String> {
    let port_spec = format!("{}/{}", rule.port, rule.protocol);
    let any = rule.source == "0.0.0.0/0" || rule.source == "::/0" || rule.source.is_empty();
    if any {
        let _ = firewall_cmd(&["--permanent", &format!("--add-port={port_spec}")]);
        let _ = firewall_cmd(&[&format!("--add-port={port_spec}")]);
    } else {
        let family = if rule.source.contains(':') {
            "ipv6"
        } else {
            "ipv4"
        };
        let rich = format!(
            "rule family=\"{family}\" source address=\"{}\" port port=\"{}\" protocol=\"{}\" accept",
            rule.source, rule.port, rule.protocol
        );
        let _ = firewall_cmd(&["--permanent", "--add-rich-rule", &rich]);
        let _ = firewall_cmd(&["--add-rich-rule", &rich]);
    }
    Ok(())
}

fn remove_port_rule(rule: &FirewallRule) -> Result<(), String> {
    let port = panel_listen_port();
    if rule.protocol == "tcp" && rule.port == port.to_string() {
        return Err(format!(
            "Refused: cannot remove the panel listen port ({port}/tcp)"
        ));
    }
    let port_spec = format!("{}/{}", rule.port, rule.protocol);
    let any = rule.source == "0.0.0.0/0" || rule.source == "::/0" || rule.source.is_empty();
    if any {
        let _ = firewall_cmd(&["--permanent", &format!("--remove-port={port_spec}")]);
        let _ = firewall_cmd(&[&format!("--remove-port={port_spec}")]);
    } else {
        let family = if rule.source.contains(':') {
            "ipv6"
        } else {
            "ipv4"
        };
        let rich = format!(
            "rule family=\"{family}\" source address=\"{}\" port port=\"{}\" protocol=\"{}\" accept",
            rule.source, rule.port, rule.protocol
        );
        let _ = firewall_cmd(&["--permanent", "--remove-rich-rule", &rich]);
        let _ = firewall_cmd(&["--remove-rich-rule", &rich]);
    }
    Ok(())
}

pub fn seed_default_rules_if_empty(store: &mut FirewallManagerStore) -> bool {
    if !store.rules.is_empty() {
        return false;
    }
    let port = panel_listen_port();
    let stamp = now_unix();
    store.rules.push(FirewallRule {
        id: format!("seed-panel-{stamp}"),
        name: "panel".into(),
        protocol: "tcp".into(),
        port: port.to_string(),
        source: "0.0.0.0/0".into(),
    });
    store.rules.push(FirewallRule {
        id: format!("seed-http-{stamp}"),
        name: "http".into(),
        protocol: "tcp".into(),
        port: "80".into(),
        source: "0.0.0.0/0".into(),
    });
    store.rules.push(FirewallRule {
        id: format!("seed-https-{stamp}"),
        name: "https".into(),
        protocol: "tcp".into(),
        port: "443".into(),
        source: "0.0.0.0/0".into(),
    });
    store.rules.push(FirewallRule {
        id: format!("seed-ssh-{stamp}"),
        name: "ssh".into(),
        protocol: "tcp".into(),
        port: "22".into(),
        source: "0.0.0.0/0".into(),
    });
    true
}

/// Load store, seed protected IPs and default rules, purge expired bans.
pub fn prepare_manager(login_ip: Option<&str>) -> FirewallManagerStore {
    let host = host_sidebar_info();
    let server = if host.ip == "Unavailable" {
        None
    } else {
        Some(host.ip.as_str())
    };
    let mut store = ensure_protected_seeds(server, login_ip);
    let expired = purge_expired_bans(&mut store);
    for ip in &expired {
        let _ = remove_ban_from_host(ip);
    }
    let seeded = seed_default_rules_if_empty(&mut store);
    if seeded || !expired.is_empty() {
        let _ = save_store(&store);
    }
    store
}

pub fn add_rule_live(
    name: &str,
    protocol: &str,
    port: &str,
    source: &str,
) -> Result<String, String> {
    let store = panel_firewall_store::add_rule(name, protocol, port, source)?;
    if let Some(rule) = store.rules.last() {
        apply_port_rule(rule)?;
    }
    ensure_panel_port_open()?;
    Ok("Rule added".into())
}

pub fn delete_rule_live(id: &str) -> Result<String, String> {
    let store = load_store();
    let Some(rule) = store.rules.iter().find(|r| r.id == id).cloned() else {
        return Err("Rule not found".into());
    };
    remove_port_rule(&rule)?;
    panel_firewall_store::remove_rule(id)?;
    ensure_panel_port_open()?;
    Ok("Rule removed".into())
}

pub fn ban_ip_live(ip: &str, reason: &str, duration_secs: Option<u64>) -> Result<String, String> {
    let store = prepare_manager(None);
    if is_protected_ip(&store, ip) {
        return Err(
            "Refused: that address is trusted (never block) or is the server/first-admin IP".into(),
        );
    }
    apply_ban_to_host(ip)?;
    panel_firewall_store::add_ban(ip, reason, duration_secs)?;
    Ok("IP banned".into())
}

pub fn unban_ip_live(ip: &str) -> Result<String, String> {
    remove_ban_from_host(ip)?;
    panel_firewall_store::remove_ban(ip)?;
    Ok("IP unbanned".into())
}

pub fn add_trusted_live(ip: &str, label: &str) -> Result<String, String> {
    panel_firewall_store::add_trusted_manual(ip, label)?;
    Ok("Trusted IP added (never block)".into())
}

pub fn remove_trusted_live(ip: &str) -> Result<String, String> {
    panel_firewall_store::remove_trusted(ip)?;
    Ok("Trusted IP removed".into())
}

pub fn export_rules_json() -> String {
    let store = load_store();
    serde_json::to_string_pretty(&store.rules).unwrap_or_else(|_| "[]".into())
}

pub fn export_banned_json() -> String {
    let store = load_store();
    serde_json::to_string_pretty(&store.banned).unwrap_or_else(|_| "[]".into())
}

pub fn import_rules_json(raw: &str) -> Result<String, String> {
    let rules: Vec<FirewallRule> =
        serde_json::from_str(raw).map_err(|e| format!("Invalid rules JSON: {e}"))?;
    if rules.len() > 200 {
        return Err("Too many rules in import".into());
    }
    let mut store = load_store();
    for rule in rules {
        let _ = panel_firewall_store::validate_protocol(&rule.protocol)?;
        let _ = panel_firewall_store::validate_port(&rule.port)?;
        let _ = validate_ip_or_cidr(&rule.source)?;
        apply_port_rule(&rule)?;
        if !store.rules.iter().any(|r| r.id == rule.id) {
            store.rules.push(rule);
        }
    }
    save_store(&store)?;
    ensure_panel_port_open()?;
    Ok("Rules imported".into())
}

pub fn import_banned_json(raw: &str) -> Result<String, String> {
    let banned: Vec<panel_firewall_store::BannedIp> =
        serde_json::from_str(raw).map_err(|e| format!("Invalid banned JSON: {e}"))?;
    if banned.len() > 500 {
        return Err("Too many banned entries in import".into());
    }
    let store = prepare_manager(None);
    let mut count = 0u32;
    for entry in banned {
        if is_protected_ip(&store, &entry.ip) {
            continue;
        }
        if apply_ban_to_host(&entry.ip).is_ok() {
            let _ = panel_firewall_store::add_ban(
                &entry.ip,
                &entry.reason,
                entry
                    .expires_at
                    .map(|e| e.saturating_sub(panel_firewall_store::now_unix())),
            );
            count += 1;
        }
    }
    Ok(format!("Imported {count} banned IP(s); protected addresses skipped"))
}

pub fn status_on() -> bool {
    firewall_status().active
}

pub fn trusted_source_label(source: &TrustedSource) -> &'static str {
    match source {
        TrustedSource::Server => "server",
        TrustedSource::FirstAdmin => "first admin",
        TrustedSource::Manual => "manual",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn csrf_roundtrip() {
        with_test_data_dir(|| {
            let tok = firewall_csrf_token("Admin");
            assert!(verify_firewall_csrf("Admin", &tok));
            assert!(!verify_firewall_csrf("Other", &tok));
        });
    }

    #[test]
    fn rich_rule_rejects_injection() {
        assert!(rich_drop_rule("1.2.3.4; reboot").is_err());
        assert!(rich_drop_rule("192.0.2.1").is_ok());
    }
}
