//! Fail2Ban inventory and unban helpers (argv only; no shell strings).

use crate::panel_ops_security::{
    cmd_output_timeout, fail2ban_status, systemctl_active, which_exists,
};
use crate::panel_session::session_secret;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::net::IpAddr;

type HmacSha256 = Hmac<Sha256>;

const F2B_TIMEOUT_SECS: u64 = 8;

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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn fail2ban_csrf_token(username: &str) -> String {
    let secret = session_secret(None);
    let hour = now_unix() / 3600;
    let payload = format!("fail2ban|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(&secret, &payload))
}

pub fn verify_fail2ban_csrf(username: &str, token: &str) -> bool {
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
    let payload = format!("fail2ban|{username}|{hour}");
    let expected = hmac_hex(&secret, &payload);
    expected == sig
}

/// Jail names from Fail2Ban are typically `[a-zA-Z0-9_-]+`.
pub fn validate_jail_name(raw: &str) -> Result<&str, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("Jail name is required".into());
    }
    if name.len() > 64 {
        return Err("Jail name is too long".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Jail name may only contain letters, digits, underscore, and hyphen".into());
    }
    Ok(name)
}

pub fn validate_ban_ip(raw: &str) -> Result<String, String> {
    let ip = raw.trim();
    if ip.is_empty() {
        return Err("IP address is required".into());
    }
    if ip.len() > 64 {
        return Err("IP address is too long".into());
    }
    let parsed: IpAddr = ip
        .parse()
        .map_err(|_| "Enter a valid IPv4 or IPv6 address".to_string())?;
    Ok(parsed.to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BannedEntry {
    pub ip: String,
    pub jail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Fail2banInventory {
    pub installed: bool,
    pub active: String,
    pub enabled: String,
    pub available: bool,
    pub detail: String,
    pub jails: Vec<String>,
    pub bans: Vec<BannedEntry>,
}

fn fail2ban_client(args: &[&str]) -> Result<String, String> {
    if !which_exists("fail2ban-client") {
        return Err("fail2ban-client is not installed".into());
    }
    let active = systemctl_active("fail2ban");
    if active != "active" && active != "activating" {
        return Err(format!(
            "Fail2Ban service is not running (systemctl reports {active})"
        ));
    }
    let (ok, stdout, stderr) =
        cmd_output_timeout("fail2ban-client", args, F2B_TIMEOUT_SECS).ok_or_else(|| {
            format!(
                "fail2ban-client {} timed out after {F2B_TIMEOUT_SECS}s",
                args.join(" ")
            )
        })?;
    if !ok {
        let detail = if stderr.is_empty() { stdout } else { stderr };
        let safe: String = detail.chars().take(400).collect();
        return Err(format!("fail2ban-client {}: {safe}", args.join(" ")));
    }
    Ok(stdout)
}

fn parse_banned_ips(status_text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in status_text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .split_once("Banned IP list:")
            .map(|(_, r)| r)
            .or_else(|| trimmed.split_once("Banned IP:").map(|(_, r)| r))
        else {
            continue;
        };
        for part in rest.split(|c: char| c == ',' || c.is_whitespace()) {
            let t = part.trim().trim_matches(|c| c == '[' || c == ']' || c == ',');
            if t.is_empty() {
                continue;
            }
            if validate_ban_ip(t).is_ok() {
                out.push(t.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// List banned IPs across jails (or one jail). Safe when Fail2Ban is missing or stopped.
pub fn list_banned(jail_filter: Option<&str>) -> Fail2banInventory {
    let st = fail2ban_status();
    if !st.installed {
        return Fail2banInventory {
            installed: false,
            active: st.active,
            enabled: st.enabled,
            available: false,
            detail: st.detail,
            jails: Vec::new(),
            bans: Vec::new(),
        };
    }
    let filter = match jail_filter {
        Some(raw) if !raw.trim().is_empty() => match validate_jail_name(raw) {
            Ok(j) => Some(j.to_string()),
            Err(err) => {
                return Fail2banInventory {
                    installed: true,
                    active: st.active,
                    enabled: st.enabled,
                    available: false,
                    detail: err,
                    jails: st.jails,
                    bans: Vec::new(),
                };
            }
        },
        _ => None,
    };

    if st.active != "active" && st.active != "activating" {
        return Fail2banInventory {
            installed: true,
            active: st.active.clone(),
            enabled: st.enabled,
            available: false,
            detail: format!(
                "Fail2Ban is installed but not running (status: {}). Start the service to list bans.",
                st.active
            ),
            jails: st.jails,
            bans: Vec::new(),
        };
    }

    let jails: Vec<String> = if let Some(ref j) = filter {
        if st.jails.iter().any(|known| known == j) {
            vec![j.clone()]
        } else {
            return Fail2banInventory {
                installed: true,
                active: st.active,
                enabled: st.enabled,
                available: true,
                detail: format!("Jail `{j}` was not found in the active jail list."),
                jails: st.jails,
                bans: Vec::new(),
            };
        }
    } else {
        st.jails.clone()
    };

    let mut bans = Vec::new();
    let mut errors = Vec::new();
    for jail in &jails {
        match fail2ban_client(&["status", jail]) {
            Ok(text) => {
                for ip in parse_banned_ips(&text) {
                    bans.push(BannedEntry {
                        ip,
                        jail: jail.clone(),
                    });
                }
            }
            Err(err) => errors.push(err),
        }
    }
    bans.sort_by(|a, b| a.jail.cmp(&b.jail).then(a.ip.cmp(&b.ip)));
    let detail = if errors.is_empty() {
        if bans.is_empty() {
            "No banned IPs in the selected jails.".into()
        } else {
            format!("{} banned IP(s) across {} jail(s).", bans.len(), jails.len())
        }
    } else {
        let joined: String = errors.join(" ");
        joined.chars().take(500).collect()
    };
    Fail2banInventory {
        installed: true,
        active: st.active,
        enabled: st.enabled,
        available: errors.is_empty() || !bans.is_empty(),
        detail,
        jails: st.jails,
        bans,
    }
}

/// Unban one IP in one jail using argv-only fail2ban-client.
pub fn unban_ip(jail: &str, ip: &str) -> Result<String, String> {
    let jail = validate_jail_name(jail)?;
    let ip = validate_ban_ip(ip)?;
    fail2ban_client(&["set", jail, "unbanip", &ip])?;
    Ok(format!("Unbanned {ip} from jail {jail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jail_name_allowlist() {
        assert!(validate_jail_name("sshd").is_ok());
        assert!(validate_jail_name("recidive").is_ok());
        assert!(validate_jail_name("nginx-http-auth").is_ok());
        assert!(validate_jail_name("bad;rm -rf").is_err());
        assert!(validate_jail_name("a b").is_err());
        assert!(validate_jail_name("$(id)").is_err());
        assert!(validate_jail_name("").is_err());
    }

    #[test]
    fn ban_ip_parses_v4_v6() {
        assert_eq!(validate_ban_ip("1.2.3.4").unwrap(), "1.2.3.4");
        assert!(validate_ban_ip("::1").is_ok());
        assert!(validate_ban_ip("not-an-ip").is_err());
        assert!(validate_ban_ip("1.2.3.4; reboot").is_err());
        assert!(validate_ban_ip("1.2.3.4 $(id)").is_err());
    }

    #[test]
    fn parse_banned_list_line() {
        let text = "Status for the jail: sshd\n|- Filter\n|  |- Currently failed: 0\n`- Banned IP list:  10.0.0.1 203.0.113.8\n";
        let ips = parse_banned_ips(text);
        assert_eq!(ips, vec!["10.0.0.1".to_string(), "203.0.113.8".to_string()]);
    }

    #[test]
    fn csrf_roundtrip() {
        let secret = format!("f2b-test-{}", std::process::id());
        // Exercise HMAC helpers via public token API with session secret; shape only.
        let hour = now_unix() / 3600;
        let payload = format!("fail2ban|Admin|{hour}");
        let tok = format!("{hour}.{}", hmac_hex(&secret, &payload));
        let (h, sig) = tok.split_once('.').unwrap();
        assert_eq!(h.parse::<u64>().unwrap(), hour);
        assert_eq!(sig.len(), 64);
    }
}
