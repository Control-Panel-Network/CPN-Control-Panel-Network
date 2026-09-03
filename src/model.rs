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
    Rainloop,
    Roundcube,
    Thunderbird,
}

impl MailSystem {
    pub fn label(self) -> &'static str {
        match self {
            Self::Snappymail => "SnappyMail",
            Self::Rainloop => "RainLoop",
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
    pub account: Option<AccountPublic>,
    pub password_policy: PasswordPolicy,
    pub panel_login_path: String,
    pub panel_login_url: Option<String>,
    pub version: String,
    /// True after the web server install finished successfully at least once.
    pub server_ready: bool,
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
            language: "es".into(),
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
}

#[derive(Debug, Deserialize)]
pub struct MailInstallRequest {
    pub mail: MailSystem,
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
}

#[derive(Debug, Deserialize)]
pub struct LanguageRequest {
    pub language: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct OptionalTokenQuery {
    pub token: Option<String>,
}
