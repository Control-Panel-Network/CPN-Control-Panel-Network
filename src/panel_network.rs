//! Panel networking preferences: listen port, optional hostname (subdomain), and port migration.
//! Files live under `/var/lib/cpn/` (override with `CPN_DATA_DIR`). Sensitive JSON uses mode 600.

use crate::account::data_dir;
use crate::listen_port::{
    DEFAULT_PORT, load_preferred_listen_port, save_preferred_listen_port, validate_listen_port,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds in 30 days (approx. one calendar month for migration windows).
pub const MONTH_SECS: u64 = 30 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OldPortPolicy {
    /// Keep HTTP redirect from the old port for about 1 month.
    Redirect1m,
    /// Keep HTTP redirect from the old port for about 3 months.
    Redirect3m,
    /// Do not listen on the old port (connection refused / no redirect).
    Deny,
}

impl OldPortPolicy {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "redirect_1m" | "redirect-1m" | "1m" | "1month" => Ok(Self::Redirect1m),
            "redirect_3m" | "redirect-3m" | "3m" | "3months" => Ok(Self::Redirect3m),
            "deny" | "close" | "none" => Ok(Self::Deny),
            other => Err(format!(
                "Unknown old-port policy '{other}' (use redirect_1m, redirect_3m, or deny)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Redirect1m => "redirect_1m",
            Self::Redirect3m => "redirect_3m",
            Self::Deny => "deny",
        }
    }

    pub fn duration_secs(self) -> Option<u64> {
        match self {
            Self::Redirect1m => Some(MONTH_SECS),
            Self::Redirect3m => Some(3 * MONTH_SECS),
            Self::Deny => None,
        }
    }

    pub fn is_redirect(self) -> bool {
        matches!(self, Self::Redirect1m | Self::Redirect3m)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortMigration {
    pub old_port: u16,
    pub new_port: u16,
    pub mode: OldPortPolicy,
    /// Unix seconds; ignored when mode is deny (file may still record intent).
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortMigrationPublic {
    pub old_port: u16,
    pub new_port: u16,
    pub mode: String,
    pub expires_at: u64,
    pub active: bool,
    pub redirect_active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkPublic {
    pub listen_port: u16,
    pub preferred_listen_port: u16,
    pub panel_hostname: Option<String>,
    pub port_migration: Option<PortMigrationPublic>,
    /// Suggested public login base (hostname without port, or host:port).
    pub public_base_url: String,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn hostname_path() -> PathBuf {
    data_dir().join("panel_hostname")
}

fn migration_path() -> PathBuf {
    data_dir().join("port_migration")
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create data directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.write_all(contents)
        .map_err(|error| format!("Could not save {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Validate a DNS hostname used as panel subdomain (no scheme, no path, no port).
pub fn validate_panel_hostname(raw: &str) -> Result<String, String> {
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Err("Panel hostname cannot be empty".into());
    }
    if value.len() > 253 {
        return Err("Panel hostname is too long (max 253)".into());
    }
    if value.contains("://") || value.contains('/') || value.contains('\\') || value.contains(' ') {
        return Err("Panel hostname must be a bare DNS name (example: panel.example.com)".into());
    }
    if value.contains(':') {
        return Err(
            "Panel hostname must not include a port; TLS on 443 terminates to the CPN listen port"
                .into(),
        );
    }
    if value == "localhost" || value.ends_with(".localhost") {
        return Ok(value);
    }
    let labels: Vec<&str> = value.split('.').collect();
    if labels.len() < 2 {
        return Err("Panel hostname needs at least two labels (example: panel.example.com)".into());
    }
    for label in labels {
        if label.is_empty() || label.len() > 63 {
            return Err("Invalid hostname label length".into());
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("Hostname labels cannot start or end with '-'".into());
        }
        if !label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            return Err("Hostname labels may only use letters, digits, and '-'".into());
        }
    }
    Ok(value)
}

pub fn load_panel_hostname() -> Option<String> {
    let raw = fs::read_to_string(hostname_path()).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    validate_panel_hostname(trimmed).ok()
}

pub fn save_panel_hostname(hostname: &str) -> Result<(), String> {
    let hostname = validate_panel_hostname(hostname)?;
    write_mode_600(&hostname_path(), format!("{hostname}\n").as_bytes())
}

pub fn clear_panel_hostname() -> Result<(), String> {
    let path = hostname_path();
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Could not clear panel hostname: {error}"))?;
    }
    Ok(())
}

pub fn load_port_migration() -> Option<PortMigration> {
    let raw = fs::read_to_string(migration_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_port_migration(migration: &PortMigration) -> Result<(), String> {
    let json = serde_json::to_string_pretty(migration)
        .map_err(|error| format!("Could not serialize port migration: {error}"))?;
    write_mode_600(&migration_path(), format!("{json}\n").as_bytes())
}

pub fn clear_port_migration() -> Result<(), String> {
    let path = migration_path();
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Could not clear port migration: {error}"))?;
    }
    Ok(())
}

/// Drop expired migration records (and deny records older than one year as stale intent).
pub fn purge_expired_migration() {
    let Some(migration) = load_port_migration() else {
        return;
    };
    let now = now_unix();
    let stale = if migration.mode.is_redirect() {
        migration.expires_at > 0 && now >= migration.expires_at
    } else {
        // Deny is immediate; keep the record briefly for operators, then drop after 1 year.
        migration.expires_at > 0 && now >= migration.expires_at
    };
    if stale {
        let _ = clear_port_migration();
    }
}

pub fn migration_is_active(migration: &PortMigration) -> bool {
    if migration.mode.is_redirect() {
        migration.expires_at == 0 || now_unix() < migration.expires_at
    } else {
        // Deny: active until cleared; expires_at marks advisory cleanup.
        migration.expires_at == 0 || now_unix() < migration.expires_at
    }
}

pub fn migration_redirect_active(migration: &PortMigration, bind_port: u16) -> bool {
    migration.mode.is_redirect()
        && migration.new_port == bind_port
        && migration.old_port != bind_port
        && migration_is_active(migration)
}

/// Build migration when changing from `old_port` to `new_port` under `policy`.
pub fn build_port_migration(
    old_port: u16,
    new_port: u16,
    policy: OldPortPolicy,
) -> Result<Option<PortMigration>, String> {
    let old_port = validate_listen_port(old_port)?;
    let new_port = validate_listen_port(new_port)?;
    if old_port == new_port {
        return Ok(None);
    }
    let now = now_unix();
    let expires_at = match policy.duration_secs() {
        Some(secs) => now.saturating_add(secs),
        // Deny: keep an advisory expiry of 1 year so purge can clean stale intent.
        None => now.saturating_add(365 * 24 * 60 * 60),
    };
    Ok(Some(PortMigration {
        old_port,
        new_port,
        mode: policy,
        expires_at,
    }))
}

/// Persist a preferred listen port and optional migration + hostname.
pub fn apply_network_change(
    new_port: u16,
    current_bind_port: u16,
    old_port_policy: Option<OldPortPolicy>,
    panel_hostname: Option<Option<String>>,
) -> Result<(u16, Option<PortMigration>), String> {
    let new_port = validate_listen_port(new_port)?;
    save_preferred_listen_port(new_port)?;

    if let Some(hostname_opt) = panel_hostname {
        match hostname_opt {
            Some(value) if !value.trim().is_empty() => {
                save_panel_hostname(&value)?;
            }
            _ => {
                clear_panel_hostname()?;
            }
        }
    }

    let migration = if new_port != current_bind_port {
        let policy = old_port_policy.ok_or_else(|| {
            "When changing the listen port, choose an old-port policy: redirect_1m, redirect_3m, or deny"
                .to_string()
        })?;
        let built = build_port_migration(current_bind_port, new_port, policy)?;
        if let Some(ref migration) = built {
            save_port_migration(migration)?;
        } else {
            clear_port_migration()?;
        }
        built
    } else if old_port_policy == Some(OldPortPolicy::Deny) {
        clear_port_migration()?;
        None
    } else {
        load_port_migration().filter(migration_is_active)
    };

    Ok((new_port, migration))
}

pub fn preferred_listen_port_or_default() -> u16 {
    load_preferred_listen_port().unwrap_or(DEFAULT_PORT)
}

pub fn public_base_url(listen_port: u16, host_hint: Option<&str>) -> String {
    if let Some(hostname) = load_panel_hostname() {
        // Subdomain without port: operators terminate TLS on 443 and proxy to listen_port.
        return format!("https://{hostname}");
    }
    let host = host_hint.unwrap_or("127.0.0.1");
    if listen_port == 443 {
        format!("https://{host}")
    } else if listen_port == 80 {
        format!("http://{host}")
    } else {
        format!("http://{host}:{listen_port}")
    }
}

pub fn migration_public(migration: &PortMigration, bind_port: u16) -> PortMigrationPublic {
    PortMigrationPublic {
        old_port: migration.old_port,
        new_port: migration.new_port,
        mode: migration.mode.as_str().into(),
        expires_at: migration.expires_at,
        active: migration_is_active(migration),
        redirect_active: migration_redirect_active(migration, bind_port),
    }
}

pub fn network_public(bind_port: u16, host_hint: Option<&str>) -> NetworkPublic {
    purge_expired_migration();
    let preferred = preferred_listen_port_or_default();
    let migration = load_port_migration()
        .filter(migration_is_active)
        .map(|value| migration_public(&value, bind_port));
    NetworkPublic {
        listen_port: bind_port,
        preferred_listen_port: preferred,
        panel_hostname: load_panel_hostname(),
        port_migration: migration,
        public_base_url: public_base_url(bind_port, host_hint),
    }
}

/// Active redirect helper target, if migration says to dual-listen on the old port.
pub fn active_redirect_migration(bind_port: u16) -> Option<PortMigration> {
    purge_expired_migration();
    let migration = load_port_migration()?;
    if migration_redirect_active(&migration, bind_port) {
        Some(migration)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn validates_hostname() {
        assert_eq!(
            validate_panel_hostname("Panel.Example.COM").unwrap(),
            "panel.example.com"
        );
        assert!(validate_panel_hostname("panel.example.com:2087").is_err());
        assert!(validate_panel_hostname("https://panel.example.com").is_err());
        assert!(validate_panel_hostname("panel").is_err());
    }

    #[test]
    fn policy_parse_and_duration() {
        assert_eq!(
            OldPortPolicy::parse("redirect-1m").unwrap(),
            OldPortPolicy::Redirect1m
        );
        assert_eq!(OldPortPolicy::parse("deny").unwrap(), OldPortPolicy::Deny);
        assert_eq!(
            OldPortPolicy::Redirect3m.duration_secs(),
            Some(3 * MONTH_SECS)
        );
        assert!(OldPortPolicy::Deny.duration_secs().is_none());
    }

    #[test]
    fn persists_hostname_and_migration() {
        with_test_data_dir(|| {
            save_panel_hostname("panel.example.com").unwrap();
            assert_eq!(load_panel_hostname().as_deref(), Some("panel.example.com"));
            let migration = build_port_migration(2087, 9443, OldPortPolicy::Redirect1m)
                .unwrap()
                .unwrap();
            save_port_migration(&migration).unwrap();
            let loaded = load_port_migration().unwrap();
            assert_eq!(loaded.old_port, 2087);
            assert_eq!(loaded.new_port, 9443);
            assert!(migration_redirect_active(&loaded, 9443));
            assert!(!migration_redirect_active(&loaded, 2087));
            clear_panel_hostname().unwrap();
            clear_port_migration().unwrap();
            assert!(load_panel_hostname().is_none());
            assert!(load_port_migration().is_none());
        });
    }

    #[test]
    fn public_base_uses_hostname_without_port() {
        with_test_data_dir(|| {
            save_panel_hostname("panel.example.com").unwrap();
            assert_eq!(
                public_base_url(2087, Some("10.0.0.5")),
                "https://panel.example.com"
            );
        });
    }

    #[test]
    fn apply_requires_policy_on_port_change() {
        with_test_data_dir(|| {
            let err = apply_network_change(9443, 2087, None, None).unwrap_err();
            assert!(err.contains("old-port policy"));
            let (port, migration) = apply_network_change(
                9443,
                2087,
                Some(OldPortPolicy::Deny),
                Some(Some("cpn.lab.local".into())),
            )
            .unwrap();
            assert_eq!(port, 9443);
            assert_eq!(migration.unwrap().mode, OldPortPolicy::Deny);
            assert_eq!(load_panel_hostname().as_deref(), Some("cpn.lab.local"));
        });
    }
}
