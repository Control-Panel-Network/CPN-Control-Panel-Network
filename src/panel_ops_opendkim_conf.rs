//! OpenDKIM `/etc/opendkim.conf` ensure (Auth-Results + KeyTable mode).

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

const OPENDKIM_CONF: &str = "/etc/opendkim.conf";
const OPENDKIM_LISTEN: &str = "inet:8891@localhost";
const KEY_TABLE: &str = "/etc/opendkim/KeyTable";
const SIGNING_TABLE: &str = "/etc/opendkim/SigningTable";
const TRUSTED_HOSTS: &str = "/etc/opendkim/TrustedHosts";

fn postconf_myhostname() -> Option<String> {
    let out = Command::new("postconf")
        .args(["-h", "myhostname"])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let host = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn write_if_changed(path: &str, content: &str) -> Result<bool, String> {
    let prev = fs::read_to_string(path).unwrap_or_default();
    if prev == content {
        return Ok(false);
    }
    if Path::new(path).exists() {
        let bak = format!(
            "{path}.bak.cpn.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );
        let _ = fs::copy(path, &bak);
    }
    fs::write(path, content).map_err(|e| format!("Cannot write {path}: {e}"))?;
    Ok(true)
}

/// Ensure Mode=sv, KeyTable, AlwaysAddARHeader, AuthservID for Tachyon Auth-Results.
pub fn ensure_opendkim_conf() -> Result<bool, String> {
    let path = Path::new(OPENDKIM_CONF);
    let mut raw = if path.exists() {
        fs::read_to_string(path).map_err(|e| format!("Cannot read {OPENDKIM_CONF}: {e}"))?
    } else {
        String::new()
    };
    let mut changed = false;
    let authserv = postconf_myhostname().unwrap_or_else(|| "localhost".into());
    let required = [
        ("Mode", "sv".to_string()),
        ("Syslog", "yes".to_string()),
        ("SyslogSuccess", "yes".to_string()),
        ("LogWhy", "yes".to_string()),
        ("UserID", "opendkim:opendkim".to_string()),
        ("Socket", OPENDKIM_LISTEN.to_string()),
        ("Umask", "002".to_string()),
        ("Canonicalization", "relaxed/relaxed".to_string()),
        ("OversignHeaders", "From".to_string()),
        ("AlwaysAddARHeader", "yes".to_string()),
        ("AuthservID", authserv),
        ("KeyTable", format!("refile:{KEY_TABLE}")),
        ("SigningTable", format!("refile:{SIGNING_TABLE}")),
        ("ExternalIgnoreList", format!("refile:{TRUSTED_HOSTS}")),
        ("InternalHosts", format!("refile:{TRUSTED_HOSTS}")),
    ];
    for (key, value) in &required {
        let prefix = format!("{key}\t");
        let prefix_sp = format!("{key} ");
        let line = format!("{key}\t{value}");
        if let Some(idx) = raw.lines().position(|l| {
            let t = l.trim_start();
            !t.starts_with('#') && (t.starts_with(&prefix) || t.starts_with(&prefix_sp))
        }) {
            let current = raw.lines().nth(idx).unwrap_or("").trim();
            if current != line.as_str() && !current.contains(value.as_str()) {
                let mut rebuilt = String::new();
                for (i, l) in raw.lines().enumerate() {
                    if i == idx {
                        rebuilt.push_str(&line);
                    } else {
                        rebuilt.push_str(l);
                    }
                    rebuilt.push('\n');
                }
                raw = rebuilt;
                changed = true;
            }
        } else {
            if !raw.is_empty() && !raw.ends_with('\n') {
                raw.push('\n');
            }
            raw.push_str(&line);
            raw.push('\n');
            changed = true;
        }
    }
    if raw.lines().any(|l| {
        let t = l.trim_start();
        !t.starts_with('#') && t.starts_with("KeyFile")
    }) {
        let mut rebuilt = String::new();
        for l in raw.lines() {
            let t = l.trim_start();
            if !t.starts_with('#') && t.starts_with("KeyFile") {
                rebuilt.push_str("# ");
                rebuilt.push_str(l);
                changed = true;
            } else {
                rebuilt.push_str(l);
            }
            rebuilt.push('\n');
        }
        raw = rebuilt;
    }
    if changed {
        write_if_changed(OPENDKIM_CONF, &raw)?;
    }
    Ok(changed)
}
