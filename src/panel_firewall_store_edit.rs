//! Edit operations for the CPN firewall manager store (rules, bans, trusted IPs).
//!
//! These only touch the persisted JSON store. Host (firewalld) changes are applied by
//! `panel_ops_firewall_rules` so a failed host call never leaves the store half-updated.

use crate::panel_firewall_store::{
    BannedIp, FirewallRule, TrustedSource, is_protected_ip, load_store, now_unix, save_store,
    validate_ip_or_cidr, validate_port, validate_protocol,
};

/// How an edited ban should treat its expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanExpiry {
    /// Leave the current expiry untouched.
    Keep,
    /// Never expire.
    Never,
    /// Expire this many seconds from now.
    InSecs(u64),
}

impl BanExpiry {
    /// Parse the edit form value: `keep`, `0` (never) or a positive number of seconds.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let value = raw.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("keep") {
            return Ok(Self::Keep);
        }
        let secs: u64 = value
            .parse()
            .map_err(|_| "Ban duration is not valid".to_string())?;
        if secs == 0 {
            Ok(Self::Never)
        } else {
            Ok(Self::InSecs(secs))
        }
    }
}

/// Validate a rule display name (1-64 characters, no control characters).
pub fn validate_rule_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err("Rule name must be 1-64 characters".into());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("Rule name cannot include control characters".into());
    }
    Ok(name.to_string())
}

/// Validate a ban target (single host or an IPv4 network of /24 or narrower).
pub fn validate_ban_target(raw: &str) -> Result<String, String> {
    let ip = validate_ip_or_cidr(raw)?;
    if let Some((_, pref)) = ip.split_once('/') {
        let bits: u8 = pref.parse().unwrap_or(0);
        if bits < 24 && ip.contains('.') {
            return Err("IPv4 ban CIDR must be /24 or narrower".into());
        }
    }
    Ok(ip)
}

/// Validate and replace a rule in the store. Returns `(old, new)` for host sync.
pub fn update_rule(
    id: &str,
    name: &str,
    protocol: &str,
    port: &str,
    source: &str,
) -> Result<(FirewallRule, FirewallRule), String> {
    let name = validate_rule_name(name)?;
    let protocol = validate_protocol(protocol)?;
    let port = validate_port(port)?;
    let source = if source.trim().is_empty() {
        "0.0.0.0/0".to_string()
    } else {
        validate_ip_or_cidr(source)?
    };
    let mut store = load_store();
    let Some(slot) = store.rules.iter_mut().find(|r| r.id == id) else {
        return Err("Rule not found".into());
    };
    let old = slot.clone();
    slot.name = name;
    slot.protocol = protocol;
    slot.port = port;
    slot.source = source;
    let new = slot.clone();
    save_store(&store)?;
    Ok((old, new))
}

/// Validate an edited ban and rewrite it in the store. Returns the previous entry.
pub fn update_ban(
    old_ip: &str,
    new_ip: &str,
    reason: &str,
    expiry: BanExpiry,
) -> Result<BannedIp, String> {
    let old_ip = validate_ip_or_cidr(old_ip)?;
    let new_ip = validate_ban_target(new_ip)?;
    let mut store = load_store();
    if is_protected_ip(&store, &new_ip) {
        return Err(
            "Refused: that address is on the trusted (never block) list or is the server/first-admin IP"
                .into(),
        );
    }
    if new_ip != old_ip && store.banned.iter().any(|b| b.ip == new_ip) {
        return Err("That IP is already in the banned list".into());
    }
    let Some(slot) = store.banned.iter_mut().find(|b| b.ip == old_ip) else {
        return Err("Banned IP not found".into());
    };
    let previous = slot.clone();
    slot.ip = new_ip;
    slot.reason = reason.trim().chars().take(200).collect();
    match expiry {
        BanExpiry::Keep => {}
        BanExpiry::Never => slot.expires_at = None,
        BanExpiry::InSecs(secs) => slot.expires_at = Some(now_unix().saturating_add(secs)),
    }
    save_store(&store)?;
    Ok(previous)
}

/// Edit a manually added trusted IP (address and label). Seeded entries stay locked.
pub fn update_trusted(old_ip: &str, new_ip: &str, label: &str) -> Result<(), String> {
    let old_ip = validate_ip_or_cidr(old_ip)?;
    let new_ip = validate_ip_or_cidr(new_ip)?;
    let mut store = load_store();
    let Some(pos) = store.trusted.iter().position(|t| t.ip == old_ip) else {
        return Err("Trusted IP not found".into());
    };
    if !matches!(store.trusted[pos].source, TrustedSource::Manual) {
        return Err("Cannot edit the protected server or first-admin IP".into());
    }
    if new_ip != old_ip && store.trusted.iter().any(|t| t.ip == new_ip) {
        return Err("That IP is already trusted".into());
    }
    let label: String = label.trim().chars().take(64).collect();
    let entry = &mut store.trusted[pos];
    entry.ip = new_ip;
    entry.label = if label.is_empty() {
        "Trusted IP".into()
    } else {
        label
    };
    entry.protected = true;
    save_store(&store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_firewall_store::{add_ban, add_rule, add_trusted_manual};

    #[test]
    fn expiry_parse_variants() {
        assert_eq!(BanExpiry::parse("keep").unwrap(), BanExpiry::Keep);
        assert_eq!(BanExpiry::parse("").unwrap(), BanExpiry::Keep);
        assert_eq!(BanExpiry::parse("0").unwrap(), BanExpiry::Never);
        assert_eq!(BanExpiry::parse("3600").unwrap(), BanExpiry::InSecs(3600));
        assert!(BanExpiry::parse("abc").is_err());
    }

    #[test]
    fn update_rule_accepts_both_protocol() {
        with_test_data_dir(|| {
            let store = add_rule("web", "tcp", "8080", "0.0.0.0/0").unwrap();
            let id = store.rules[0].id.clone();
            let (old, new) = update_rule(&id, "web2", "both", "8081", "192.0.2.0/24").unwrap();
            assert_eq!(old.protocol, "tcp");
            assert_eq!(new.protocol, "both");
            assert_eq!(new.port, "8081");
            assert_eq!(new.source, "192.0.2.0/24");
            assert!(update_rule("missing", "x", "tcp", "80", "").is_err());
            assert!(update_rule(&id, "x", "icmp", "80", "").is_err());
        });
    }

    #[test]
    fn update_ban_keeps_time_and_rejects_duplicates() {
        with_test_data_dir(|| {
            add_ban("192.0.2.10", "one", None).unwrap();
            add_ban("192.0.2.11", "two", Some(60)).unwrap();
            let prev = update_ban("192.0.2.10", "192.0.2.12", "moved", BanExpiry::Keep).unwrap();
            assert_eq!(prev.ip, "192.0.2.10");
            let err = update_ban("192.0.2.12", "192.0.2.11", "x", BanExpiry::Keep).unwrap_err();
            assert!(err.contains("already"));
            update_ban("192.0.2.11", "192.0.2.11", "two", BanExpiry::Never).unwrap();
            let store = load_store();
            let b = store.banned.iter().find(|b| b.ip == "192.0.2.11").unwrap();
            assert!(b.expires_at.is_none());
        });
    }

    #[test]
    fn update_trusted_only_for_manual_entries() {
        with_test_data_dir(|| {
            add_trusted_manual("198.51.100.5", "Home").unwrap();
            update_trusted("198.51.100.5", "198.51.100.6", "Office").unwrap();
            let store = load_store();
            assert!(store.trusted.iter().any(|t| t.ip == "198.51.100.6"));
            assert!(update_trusted("198.51.100.99", "198.51.100.7", "x").is_err());
        });
    }
}
