//! Live firewalld application of CPN firewall rules, plus edit operations.
//!
//! A rule protocol of `both` expands to two firewalld entries (tcp and udp) so the
//! stored rule stays a single row while the host gets both transports.

use crate::panel_firewall_store::{self, FirewallRule, is_protected_ip, load_store};
use crate::panel_firewall_store_edit::{self as store_edit, BanExpiry};
use crate::panel_ops_firewall::{
    apply_ban_to_host, ensure_panel_port_open, firewall_cmd_timed, panel_listen_port,
    prepare_manager, remove_ban_from_host,
};
use crate::panel_ops_firewall_status::invalidate_firewall_status_cache;

/// Concrete firewalld protocols for a stored protocol (`both` means tcp and udp).
pub fn host_protocols(protocol: &str) -> Vec<&'static str> {
    match protocol.trim().to_ascii_lowercase().as_str() {
        "udp" => vec!["udp"],
        "both" => vec!["tcp", "udp"],
        _ => vec!["tcp"],
    }
}

/// Human label for a stored protocol.
pub fn protocol_label(protocol: &str) -> &'static str {
    match protocol.trim().to_ascii_lowercase().as_str() {
        "udp" => "UDP",
        "both" => "TCP + UDP",
        _ => "TCP",
    }
}

fn is_any_source(source: &str) -> bool {
    source.is_empty() || source == "0.0.0.0/0" || source == "::/0"
}

fn rich_port_rule(rule: &FirewallRule, proto: &str) -> String {
    let family = if rule.source.contains(':') {
        "ipv6"
    } else {
        "ipv4"
    };
    format!(
        "rule family=\"{family}\" source address=\"{}\" port port=\"{}\" protocol=\"{proto}\" accept",
        rule.source, rule.port
    )
}

fn host_port_change(rule: &FirewallRule, add: bool) {
    let verb = if add { "add" } else { "remove" };
    for proto in host_protocols(&rule.protocol) {
        if is_any_source(&rule.source) {
            let arg = format!("--{verb}-port={}/{proto}", rule.port);
            let _ = firewall_cmd_timed(&["--permanent", &arg]);
            let _ = firewall_cmd_timed(&[&arg]);
        } else {
            let rich = rich_port_rule(rule, proto);
            let flag = format!("--{verb}-rich-rule");
            let _ = firewall_cmd_timed(&["--permanent", &flag, &rich]);
            let _ = firewall_cmd_timed(&[&flag, &rich]);
        }
    }
    invalidate_firewall_status_cache();
}

/// Open the rule's port(s) on the host.
pub fn apply_port_rule(rule: &FirewallRule) -> Result<(), String> {
    host_port_change(rule, true);
    Ok(())
}

/// True when the rule opens the panel listen port over tcp.
fn covers_panel_port(rule: &FirewallRule) -> bool {
    host_protocols(&rule.protocol).contains(&"tcp") && rule.port == panel_listen_port().to_string()
}

/// Close the rule's port(s) on the host, refusing the panel listen port.
pub fn remove_port_rule(rule: &FirewallRule) -> Result<(), String> {
    if covers_panel_port(rule) {
        return Err(format!(
            "Refused: cannot remove the panel listen port ({}/tcp)",
            panel_listen_port()
        ));
    }
    host_port_change(rule, false);
    Ok(())
}

/// Edit a rule: validate, update the store, then swap the host entries.
pub fn update_rule_live(
    id: &str,
    name: &str,
    protocol: &str,
    port: &str,
    source: &str,
) -> Result<String, String> {
    let existing = load_store()
        .rules
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| "Rule not found".to_string())?;
    // Validate everything before touching the host or the store.
    let probe = FirewallRule {
        id: id.to_string(),
        name: store_edit::validate_rule_name(name)?,
        protocol: panel_firewall_store::validate_protocol(protocol)?,
        port: panel_firewall_store::validate_port(port)?,
        source: if source.trim().is_empty() {
            "0.0.0.0/0".into()
        } else {
            panel_firewall_store::validate_ip_or_cidr(source)?
        },
    };
    if covers_panel_port(&existing) && !covers_panel_port(&probe) {
        return Err(format!(
            "Refused: the panel listen port ({}/tcp) rule must keep TCP and its port",
            panel_listen_port()
        ));
    }
    let (old, new) = store_edit::update_rule(id, name, protocol, port, source)?;
    host_port_change(&old, false);
    host_port_change(&new, true);
    ensure_panel_port_open()?;
    Ok("Rule updated".into())
}

/// Edit a banned entry (address, reason, expiry) and sync the host drop rule.
pub fn update_ban_live(
    old_ip: &str,
    new_ip: &str,
    reason: &str,
    duration: &str,
) -> Result<String, String> {
    let expiry = BanExpiry::parse(duration)?;
    let store = prepare_manager(None);
    let new_norm = store_edit::validate_ban_target(new_ip)?;
    if is_protected_ip(&store, &new_norm) {
        return Err(
            "Refused: that address is trusted (never block) or is the server/first-admin IP".into(),
        );
    }
    let previous = store_edit::update_ban(old_ip, new_ip, reason, expiry)?;
    if previous.ip != new_norm {
        remove_ban_from_host(&previous.ip)?;
        apply_ban_to_host(&new_norm)?;
    }
    invalidate_firewall_status_cache();
    Ok("Banned IP updated".into())
}

/// Edit a manual trusted IP (address and label).
pub fn update_trusted_live(old_ip: &str, new_ip: &str, label: &str) -> Result<String, String> {
    store_edit::update_trusted(old_ip, new_ip, label)?;
    Ok("Trusted IP updated".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_expands_to_tcp_and_udp() {
        assert_eq!(host_protocols("both"), vec!["tcp", "udp"]);
        assert_eq!(host_protocols("UDP"), vec!["udp"]);
        assert_eq!(host_protocols("tcp"), vec!["tcp"]);
        assert_eq!(protocol_label("both"), "TCP + UDP");
    }

    #[test]
    fn rich_rule_matches_family_and_proto() {
        let rule = FirewallRule {
            id: "x".into(),
            name: "n".into(),
            protocol: "both".into(),
            port: "53".into(),
            source: "2001:db8::/32".into(),
        };
        let udp = rich_port_rule(&rule, "udp");
        assert!(udp.contains("family=\"ipv6\""));
        assert!(udp.contains("protocol=\"udp\""));
        assert!(udp.contains("port=\"53\""));
    }

    #[test]
    fn any_source_detection() {
        assert!(is_any_source("0.0.0.0/0"));
        assert!(is_any_source("::/0"));
        assert!(is_any_source(""));
        assert!(!is_any_source("192.0.2.1"));
    }
}
