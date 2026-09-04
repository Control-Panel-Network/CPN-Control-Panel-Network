use crate::releases::CpnRelease;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerEngine {
    Openlitespeed,
    Nginx,
    Caddy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MailSystem {
    Snappymail,
    Roundcube,
    Thunderbird,
}

impl MailSystem {
    pub fn label(self) -> &'static str {
        match self {
            Self::Snappymail => "SnappyMail",
            Self::Roundcube => "Roundcube",
            Self::Thunderbird => "Thunderbird",
        }
    }
}

impl ServerEngine {
    pub fn label(self) -> &'static str {
        match self {
            Self::Openlitespeed => "OpenLiteSpeed",
            Self::Nginx => "Nginx",
            Self::Caddy => "Caddy",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentInfo {
    pub is_vps: bool,
    /// True when running inside Docker, Podman, or another container runtime.
    pub is_container: bool,
    pub virtualization: Option<String>,
    pub firewall: Option<String>,
    pub port: u16,
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordPolicy {
    pub min_length: u8,
    pub require_special: bool,
    pub require_uppercase: bool,
    pub require_number: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountPublic {
    pub username: String,
    pub recovery_email: String,
    pub configured: bool,
}

/// Safe SMTP summary for `/api/status` (no passwords or SMTP usernames).
#[derive(Debug, Clone, Serialize)]
pub struct SmtpStatusPublic {
    pub configured: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub tls_mode: Option<String>,
    pub from_address: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MailReleaseInfo {
    pub id: String,
    pub label: String,
    pub version: String,
    /// ISO date `YYYY-MM-DD` for UI formatting.
    pub released_on: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceAction {
    Upgrade,
    Downgrade,
    Repair,
    ConfigOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceRequest {
    pub action: MaintenanceAction,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub confirm_downgrade: bool,
    #[serde(default)]
    pub reset_data: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaintenancePlan {
    pub action: MaintenanceAction,
    pub target_version: String,
    pub overwrite_paths: Vec<String>,
    pub preserve_paths: Vec<String>,
    pub reset_data: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceInfo {
    pub existing_install: bool,
    pub installed_version: String,
    pub running_version: String,
    pub latest_version: Option<String>,
    pub latest_tag: Option<String>,
    pub update_available: bool,
    pub downgrade_possible: bool,
    pub repo: String,
    pub source: String,
    pub releases: Vec<CpnRelease>,
    pub has_manifest: bool,
    pub has_bootstrap: bool,
    pub plan: Option<MaintenancePlan>,
    pub check_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallerStatus {
    pub phase: &'static str,
    pub progress: u8,
    pub message: String,
    pub selected_server: Option<ServerEngine>,
    pub selected_mail: Option<MailSystem>,
    pub environment: Option<EnvironmentInfo>,
    pub error: Option<String>,
    pub language: String,
    /// Active installer HTTP listen port (bind port for this process).
    pub listen_port: u16,
    pub account: Option<AccountPublic>,
    pub password_policy: PasswordPolicy,
    pub panel_login_path: String,
    pub panel_login_url: Option<String>,
    pub version: String,
    /// True after the web server install finished successfully at least once.
    pub server_ready: bool,
    pub mail_client_ready: bool,
    pub mail_backend_ready: bool,
    pub external_ports_configured: bool,
    pub access_note: Option<String>,
    pub mail_releases: Vec<MailReleaseInfo>,
    /// Outbound SMTP presence only; never includes passwords.
    pub smtp: Option<SmtpStatusPublic>,
    pub maintenance: Option<MaintenanceInfo>,
}

impl Default for InstallerStatus {
    fn default() -> Self {
        Self {
            phase: "preparing",
            progress: 0,
            message: "Estamos preparando todo...".into(),
            selected_server: None,
            selected_mail: None,
            environment: None,
            error: None,
            language: "en".into(),
            listen_port: crate::listen_port::DEFAULT_PORT,
            account: None,
            password_policy: PasswordPolicy {
                min_length: 8,
                require_special: true,
                require_uppercase: true,
                require_number: true,
            },
            panel_login_path: "/login".into(),
            panel_login_url: None,
            version: env!("CARGO_PKG_VERSION").into(),
            server_ready: false,
            mail_client_ready: false,
            mail_backend_ready: false,
            external_ports_configured: false,
            access_note: None,
            mail_releases: Vec::new(),
            smtp: None,
            maintenance: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InstallerEvent {
    Snapshot { status: InstallerStatus },
    Progress { status: InstallerStatus },
    Log { line: String, level: &'static str },
    Completed { status: InstallerStatus },
    Error { status: InstallerStatus },
}

#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    pub server: ServerEngine,
    /// Explicit reinstall/migrate when a server is already ready (issue #20).
    #[serde(default)]
    pub force_reinstall: bool,
}

#[derive(Debug, Deserialize)]
pub struct MailInstallRequest {
    pub mail: MailSystem,
    /// Explicit mail swap when mail is already installed (issue #20).
    #[serde(default)]
    pub force_reinstall: bool,
}

#[derive(Debug, Deserialize)]
pub struct AccountSetupRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default)]
    pub generate_password: bool,
    pub recovery_email: String,
    pub password_policy: Option<PasswordPolicy>,
    pub language: Option<String>,
    /// Optional outbound SMTP settings saved under `/var/lib/cpn/smtp.json`.
    pub smtp: Option<crate::smtp_settings::SmtpSetupInput>,
    /// When true and SMTP is configured, email the username (and login URL) after setup.
    #[serde(default)]
    pub send_username_email: bool,
    /// Opt-in only: include the plaintext password in the setup email.
    #[serde(default)]
    pub include_password_in_email: bool,
}

#[derive(Debug, Deserialize)]
pub struct LanguageRequest {
    pub language: String,
}

#[derive(Debug, Deserialize)]
pub struct ListenPortRequest {
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    /// Optional when the installer session cookie or Authorization header is set (issue #1).
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Reserved for endpoints that accept optional ?token=
pub struct OptionalTokenQuery {
    pub token: Option<String>,
}
