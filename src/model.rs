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

#[derive(Debug, Clone, Serialize)]
pub struct InstallerStatus {
    pub phase: &'static str,
    pub progress: u8,
    pub message: String,
    pub selected_server: Option<ServerEngine>,
    pub selected_mail: Option<MailSystem>,
    pub environment: Option<EnvironmentInfo>,
    pub error: Option<String>,
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
pub struct TokenQuery {
    pub token: String,
}
