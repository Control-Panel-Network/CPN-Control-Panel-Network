use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerEngine {
    Openlitespeed,
    Nginx,
    Caddy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MailSystem {
    Snappymail,
    Rainloop,
    Roundcube,
    Thunderbird,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallerPhase {
    Preparing,
    Ready,
    Downloading,
    Installing,
    Testing,
    Completed,
    FailedRolledBack,
    FailedPartial,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SetupStage {
    Domain,
    Dns,
    Server,
    Mail,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsProvider {
    Local,
    Cloudflare,
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
    pub remote_access: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallerStatus {
    pub phase: InstallerPhase,
    pub stage: SetupStage,
    pub progress: u8,
    pub message: String,
    pub domain: Option<String>,
    pub domain_is_cloudflare: bool,
    pub dns_provider: Option<DnsProvider>,
    pub cloudflare_connected: bool,
    pub selected_server: Option<ServerEngine>,
    pub installed_server: Option<ServerEngine>,
    pub selected_mail: Option<MailSystem>,
    pub installed_mail: Option<MailSystem>,
    pub environment: Option<EnvironmentInfo>,
    pub panel_url: Option<String>,
    pub panel_admin_email: Option<String>,
    pub panel_admin_password: Option<String>,
    pub error: Option<String>,
    pub failed_phase: Option<InstallerPhase>,
}

impl Default for InstallerStatus {
    fn default() -> Self {
        Self {
            phase: InstallerPhase::Preparing,
            stage: SetupStage::Server,
            progress: 0,
            message: "Estamos preparando todo...".into(),
            domain: None,
            domain_is_cloudflare: false,
            dns_provider: None,
            cloudflare_connected: false,
            selected_server: None,
            installed_server: None,
            selected_mail: None,
            installed_mail: None,
            environment: None,
            panel_url: None,
            panel_admin_email: None,
            panel_admin_password: None,
            error: None,
            failed_phase: None,
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
pub struct DomainRequest {
    pub domain: String,
}

#[derive(Debug, Deserialize)]
pub struct DnsRequest {
    pub provider: DnsProvider,
}

#[derive(Debug, Deserialize)]
pub struct BootstrapQuery {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct CloudflareCallbackQuery {
    pub session: String,
    pub claim: Option<String>,
    pub oauth_error: Option<String>,
}

#[cfg(test)]
mod callback_tests {
    use super::CloudflareCallbackQuery;

    #[test]
    fn accepts_cloudflare_error_without_claim() {
        let query: CloudflareCallbackQuery = serde_urlencoded::from_str(
            "session=valid-session&oauth_error=The+requested+scope+is+invalid",
        )
        .expect("Cloudflare error callbacks must deserialize without a claim");

        assert_eq!(query.session, "valid-session");
        assert!(query.claim.is_none());
        assert_eq!(
            query.oauth_error.as_deref(),
            Some("The requested scope is invalid")
        );
    }

    #[test]
    fn accepts_successful_cloudflare_claim() {
        let query: CloudflareCallbackQuery =
            serde_urlencoded::from_str("session=valid-session&claim=one-time-claim")
                .expect("successful Cloudflare callbacks must deserialize");

        assert_eq!(query.claim.as_deref(), Some("one-time-claim"));
        assert!(query.oauth_error.is_none());
    }
}
