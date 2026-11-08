//! Panel audit event schema (CRA Annex I activity trail).
//!
//! Compatible with the legacy panel-actions JSONL shape (`ts`, `actor`, `action`,
//! `target`, `ok`, `message`). Newer fields are optional with serde defaults so old
//! lines still deserialize.

use serde::{Deserialize, Serialize};

/// Security-relevant category for filtering, retention opt-out, and alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditCategory {
    Auth,
    Identity,
    Accounts,
    ApiTokens,
    Packages,
    HostingPackages,
    Websites,
    Database,
    Firewall,
    Dns,
    Email,
    Backup,
    Docker,
    Files,
    Settings,
    Repair,
    Migrate,
    Updates,
    Cli,
    Themes,
    Notifications,
    Ui,
    Other,
}

impl AuditCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::Identity => "identity",
            Self::Accounts => "accounts",
            Self::ApiTokens => "api_tokens",
            Self::Packages => "packages",
            Self::HostingPackages => "hosting_packages",
            Self::Websites => "websites",
            Self::Database => "database",
            Self::Firewall => "firewall",
            Self::Dns => "dns",
            Self::Email => "email",
            Self::Backup => "backup",
            Self::Docker => "docker",
            Self::Files => "files",
            Self::Settings => "settings",
            Self::Repair => "repair",
            Self::Migrate => "migrate",
            Self::Updates => "updates",
            Self::Cli => "cli",
            Self::Themes => "themes",
            Self::Notifications => "notifications",
            Self::Ui => "ui",
            Self::Other => "other",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "auth" => Some(Self::Auth),
            "identity" => Some(Self::Identity),
            "accounts" => Some(Self::Accounts),
            "api_tokens" | "tokens" => Some(Self::ApiTokens),
            "packages" | "plugins" => Some(Self::Packages),
            "hosting_packages" => Some(Self::HostingPackages),
            "websites" | "wordpress" => Some(Self::Websites),
            "database" | "db" | "ftp" | "sftp" => Some(Self::Database),
            "firewall" | "security" | "ssh" | "ssl" => Some(Self::Firewall),
            "dns" | "cloudflare" => Some(Self::Dns),
            "email" | "mail" => Some(Self::Email),
            "backup" | "restore" => Some(Self::Backup),
            "docker" | "php" | "services" => Some(Self::Docker),
            "files" => Some(Self::Files),
            "settings" => Some(Self::Settings),
            "repair" => Some(Self::Repair),
            "migrate" => Some(Self::Migrate),
            "updates" | "upgrade" => Some(Self::Updates),
            "cli" => Some(Self::Cli),
            "themes" => Some(Self::Themes),
            "notifications" => Some(Self::Notifications),
            "ui" | "prefs" => Some(Self::Ui),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    /// Categories that operators cannot disable (CRA security-essential).
    pub fn is_security_essential(self) -> bool {
        matches!(
            self,
            Self::Auth
                | Self::Identity
                | Self::Accounts
                | Self::ApiTokens
                | Self::Firewall
                | Self::Updates
                | Self::Cli
                | Self::Migrate
                | Self::Repair
                | Self::Backup
                | Self::Settings
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    Info,
    Notice,
    Warning,
    Critical,
}

impl AuditSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Notice => "notice",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "info" => Some(Self::Info),
            "notice" => Some(Self::Notice),
            "warning" | "warn" => Some(Self::Warning),
            "critical" | "crit" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// One append-only audit row (JSONL).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditRecord {
    /// Unix seconds (UTC).
    pub ts: u64,
    pub actor: String,
    /// Machine key such as `plugin.install-host` or `auth.login`.
    pub action: String,
    /// Domain, username, rule id, or empty for host-wide actions.
    #[serde(default)]
    pub target: String,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ip: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ua: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub request_id: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub severity: String,
}

impl AuditRecord {
    pub fn category_enum(&self) -> AuditCategory {
        AuditCategory::parse(&self.category).unwrap_or(AuditCategory::Other)
    }

    pub fn severity_enum(&self) -> AuditSeverity {
        AuditSeverity::parse(&self.severity).unwrap_or(AuditSeverity::Info)
    }
}

/// Builder for richer records without forcing every caller to fill optional fields.
#[derive(Debug, Clone)]
pub struct AuditEvent<'a> {
    pub actor: &'a str,
    pub action: &'a str,
    pub target: &'a str,
    pub ok: bool,
    pub message: &'a str,
    pub ip: &'a str,
    pub ua: &'a str,
    pub request_id: &'a str,
    pub category: AuditCategory,
    pub severity: AuditSeverity,
}

impl<'a> AuditEvent<'a> {
    pub fn new(actor: &'a str, action: &'a str, ok: bool, message: &'a str) -> Self {
        Self {
            actor,
            action,
            target: "",
            ok,
            message,
            ip: "",
            ua: "",
            request_id: "",
            category: AuditCategory::Other,
            severity: if ok {
                AuditSeverity::Info
            } else {
                AuditSeverity::Warning
            },
        }
    }

    pub fn target(mut self, target: &'a str) -> Self {
        self.target = target;
        self
    }

    pub fn ip(mut self, ip: &'a str) -> Self {
        self.ip = ip;
        self
    }

    pub fn ua(mut self, ua: &'a str) -> Self {
        self.ua = ua;
        self
    }

    pub fn request_id(mut self, request_id: &'a str) -> Self {
        self.request_id = request_id;
        self
    }

    pub fn category(mut self, category: AuditCategory) -> Self {
        self.category = category;
        self
    }

    pub fn severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn essential_categories_locked() {
        assert!(AuditCategory::Auth.is_security_essential());
        assert!(AuditCategory::ApiTokens.is_security_essential());
        assert!(!AuditCategory::Ui.is_security_essential());
        assert!(!AuditCategory::Themes.is_security_essential());
    }

    #[test]
    fn legacy_json_deserializes() {
        let raw = r#"{"ts":1,"actor":"a","action":"plugin.install","target":"","ok":true,"message":"ok"}"#;
        let row: AuditRecord = serde_json::from_str(raw).unwrap();
        assert_eq!(row.actor, "a");
        assert!(row.ip.is_empty());
        assert_eq!(row.category_enum(), AuditCategory::Other);
    }
}
