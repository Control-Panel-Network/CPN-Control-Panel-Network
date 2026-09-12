//! Webmail public URLs, panel path config, and install detection for CPN Email.

use crate::account::data_dir;
use crate::install_webmail_runtime::webmail_health_url;
use crate::model::MailSystem;
use crate::panel_feature_gate::webmail_installed;
use crate::panel_public_url::load_panel_public_url;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "webmail-panel.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebmailPanelConfig {
    /// Public mount under the panel (for example `/snappymail`). No trailing slash.
    #[serde(default = "default_public_path")]
    pub public_path: String,
    /// Optional mailbox used as best-effort auto-login / Email prefill.
    #[serde(default)]
    pub auto_login_account: String,
    /// When true, Email > Webmail can embed webmail in an iframe.
    #[serde(default)]
    pub internal_embed: bool,
}

fn default_public_path() -> String {
    detect_default_public_path()
}

impl Default for WebmailPanelConfig {
    fn default() -> Self {
        Self {
            public_path: detect_default_public_path(),
            auto_login_account: String::new(),
            internal_embed: false,
        }
    }
}

fn config_path() -> PathBuf {
    data_dir().join(CONFIG_FILE)
}

fn detect_default_public_path() -> String {
    match detect_webmail_client() {
        Some(MailSystem::Roundcube) => "/roundcube".into(),
        _ => "/snappymail".into(),
    }
}

/// Which webmail tree is present on disk (SnappyMail preferred when both exist).
pub fn detect_webmail_client() -> Option<MailSystem> {
    if Path::new("/opt/cpn-webmail/snappymail").is_dir() {
        return Some(MailSystem::Snappymail);
    }
    if Path::new("/opt/cpn-webmail/roundcube").is_dir() {
        return Some(MailSystem::Roundcube);
    }
    if Path::new("/opt/cpn-webmail/current").exists() {
        if let Ok(target) = fs::read_link("/opt/cpn-webmail/current") {
            let s = target.to_string_lossy().to_ascii_lowercase();
            if s.contains("roundcube") {
                return Some(MailSystem::Roundcube);
            }
            if s.contains("snappy") {
                return Some(MailSystem::Snappymail);
            }
        }
        return Some(MailSystem::Snappymail);
    }
    None
}

pub fn webmail_ready() -> bool {
    webmail_installed() && detect_webmail_client().is_some()
}

fn normalize_public_path(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed == "/" || trimmed.chars().all(|ch| ch == '/') {
        return Err("Webmail public path cannot be the panel root `/`".into());
    }
    let mut path = trimmed.trim_end_matches('/').to_string();
    if path.is_empty() {
        path = detect_default_public_path();
    }
    if !path.starts_with('/') {
        path = format!("/{path}");
    }
    if path == "/" {
        return Err("Webmail public path cannot be the panel root `/`".into());
    }
    if path.contains("..") || path.contains("//") || path.contains('?') || path.contains('#') {
        return Err("Webmail public path is invalid".into());
    }
    if !path
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '/' || ch == '-' || ch == '_')
    {
        return Err("Webmail public path may only use letters, digits, /, -, and _".into());
    }
    if path.len() > 64 {
        return Err("Webmail public path is too long".into());
    }
    Ok(path)
}

pub fn load_webmail_config() -> WebmailPanelConfig {
    let path = config_path();
    if !path.is_file() {
        return WebmailPanelConfig::default();
    }
    let Ok(raw) = fs::read_to_string(&path) else {
        return WebmailPanelConfig::default();
    };
    let mut cfg: WebmailPanelConfig = serde_json::from_str(&raw).unwrap_or_default();
    if let Ok(normalized) = normalize_public_path(&cfg.public_path) {
        cfg.public_path = normalized;
    } else {
        cfg.public_path = detect_default_public_path();
    }
    cfg
}

pub fn save_webmail_config(cfg: &WebmailPanelConfig) -> Result<(), String> {
    let mut next = cfg.clone();
    next.public_path = normalize_public_path(&next.public_path)?;
    next.auto_login_account = next.auto_login_account.trim().to_string();
    if next.auto_login_account.len() > 254 {
        return Err("Auto-login account is too long".into());
    }
    if let Some(parent) = config_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(&next)
        .map_err(|e| format!("Could not serialize webmail config: {e}"))?;
    fs::write(config_path(), raw).map_err(|e| format!("Could not write webmail config: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(config_path(), fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Regenerate a random public path (mail data under /opt and /var/lib is unchanged).
pub fn regenerate_webmail_path() -> Result<WebmailPanelConfig, String> {
    let mut cfg = load_webmail_config();
    let suffix: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect::<String>()
        .to_ascii_lowercase();
    let prefix = match detect_webmail_client() {
        Some(MailSystem::Roundcube) => "roundcube",
        _ => "snappymail",
    };
    cfg.public_path = format!("/{prefix}-{suffix}");
    save_webmail_config(&cfg)?;
    Ok(cfg)
}

fn panel_base_for_links(listen_port: u16, host_hint: Option<&str>) -> String {
    if let Some(url) = load_panel_public_url() {
        return url.trim_end_matches('/').to_string();
    }
    let host = host_hint
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .unwrap_or("127.0.0.1");
    format!("http://{host}:{listen_port}")
}

/// Docroot the loopback webmail frontend should serve (SnappyMail preferred when both exist).
pub fn webmail_backend_docroot() -> Option<&'static str> {
    match detect_webmail_client()? {
        MailSystem::Snappymail => Some("/opt/cpn-webmail/snappymail"),
        MailSystem::Roundcube => {
            if Path::new("/opt/cpn-webmail/roundcube/public_html").is_dir() {
                Some("/opt/cpn-webmail/roundcube/public_html")
            } else {
                Some("/opt/cpn-webmail/roundcube")
            }
        }
        MailSystem::Thunderbird => None,
    }
}

/// Absolute Open Webmail URL. Unauthenticated clients get the login UI (not an inbox hash).
pub fn webmail_open_url(listen_port: u16, host_hint: Option<&str>) -> Option<String> {
    if !webmail_ready() {
        return None;
    }
    let cfg = load_webmail_config();
    let base = panel_base_for_links(listen_port, host_hint);
    let path = cfg.public_path.trim_end_matches('/');
    let client = detect_webmail_client()?;
    let mut url = match client {
        // Login form first; #/mailbox/INBOX only applies after a successful session.
        MailSystem::Snappymail => format!("{base}{path}/index.php"),
        MailSystem::Roundcube => format!("{base}{path}/"),
        MailSystem::Thunderbird => return None,
    };
    if !cfg.auto_login_account.is_empty() {
        let email = urlencoding_form(&cfg.auto_login_account);
        let sep = if url.contains('?') { "&" } else { "?" };
        match client {
            MailSystem::Snappymail => {
                // Best-effort Email prefill. True SSO needs a stored mailbox secret (not wired).
                url = format!("{url}{sep}Email={email}");
            }
            MailSystem::Roundcube => {
                url = format!("{url}{sep}_user={email}");
            }
            MailSystem::Thunderbird => {}
        }
    }
    Some(url)
}

/// Relative open path for same-origin links and iframes (login UI when not authenticated).
pub fn webmail_open_path() -> Option<String> {
    if !webmail_ready() {
        return None;
    }
    let cfg = load_webmail_config();
    let path = cfg.public_path.trim_end_matches('/');
    match detect_webmail_client()? {
        MailSystem::Snappymail => {
            if cfg.auto_login_account.is_empty() {
                Some(format!("{path}/index.php"))
            } else {
                Some(format!(
                    "{path}/index.php?Email={}",
                    urlencoding_form(&cfg.auto_login_account)
                ))
            }
        }
        MailSystem::Roundcube => {
            if cfg.auto_login_account.is_empty() {
                Some(format!("{path}/"))
            } else {
                Some(format!(
                    "{path}/?_user={}",
                    urlencoding_form(&cfg.auto_login_account)
                ))
            }
        }
        MailSystem::Thunderbird => None,
    }
}

pub fn webmail_admin_path() -> Option<String> {
    if !webmail_ready() {
        return None;
    }
    let cfg = load_webmail_config();
    let path = cfg.public_path.trim_end_matches('/');
    match detect_webmail_client()? {
        MailSystem::Snappymail => Some(format!("{path}/?admin")),
        MailSystem::Roundcube => Some(format!("{path}/?_task=settings")),
        MailSystem::Thunderbird => None,
    }
}

pub fn webmail_label() -> &'static str {
    match detect_webmail_client() {
        Some(MailSystem::Roundcube) => "Roundcube",
        Some(MailSystem::Snappymail) => "SnappyMail",
        _ => "Webmail",
    }
}

pub fn webmail_health_hint() -> &'static str {
    webmail_health_url()
}

fn urlencoding_form(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// True when request path matches the configured webmail mount (with optional trailing slash).
pub fn path_matches_webmail_mount(req_path: &str) -> bool {
    let cfg = load_webmail_config();
    let mount = cfg.public_path.trim_end_matches('/');
    if mount.is_empty() || mount == "/" {
        return false;
    }
    req_path == mount
        || req_path.starts_with(&format!("{mount}/"))
        || req_path == format!("{mount}/")
}

/// SnappyMail HTML uses absolute `/snappymail/v/{ver}/static|themes/...` asset URLs.
/// Those must proxy even when the panel mount was regenerated away from `/snappymail`.
pub fn path_is_snappymail_app_asset(req_path: &str) -> bool {
    matches!(detect_webmail_client(), Some(MailSystem::Snappymail))
        && (req_path.starts_with("/snappymail/v/") || req_path == "/snappymail/v")
}

/// True when the panel catch-all should reverse-proxy to loopback webmail.
pub fn path_should_proxy_webmail(req_path: &str) -> bool {
    path_matches_webmail_mount(req_path) || path_is_snappymail_app_asset(req_path)
}

/// Strip mount prefix so backend (root on :8080) receives the remainder.
pub fn strip_webmail_mount(req_path: &str) -> Option<String> {
    let cfg = load_webmail_config();
    let mount = cfg.public_path.trim_end_matches('/');
    if !path_matches_webmail_mount(req_path) {
        return None;
    }
    let rest = req_path.strip_prefix(mount).unwrap_or("");
    if rest.is_empty() || rest == "/" {
        return Some("/".into());
    }
    Some(rest.to_string())
}

/// Map a panel URL to the loopback (:8080) path.
///
/// SnappyMail ships versioned assets under `{docroot}/snappymail/v/...`, so the backend
/// URL is `/snappymail/v/...`. The panel mount is often also `/snappymail`, and a naive
/// strip turns `/snappymail/v/...` into `/v/...`. Nginx `try_files` then falls back to
/// `index.php` (HTML), which browsers reject as the wrong MIME type for JS/CSS.
pub fn backend_path_for_webmail_proxy(req_path: &str) -> Option<String> {
    if path_is_snappymail_app_asset(req_path) {
        return Some(req_path.to_string());
    }
    let stripped = strip_webmail_mount(req_path)?;
    if matches!(detect_webmail_client(), Some(MailSystem::Snappymail)) {
        return Some(remap_snappymail_stripped_asset(&stripped));
    }
    Some(stripped)
}

fn remap_snappymail_stripped_asset(stripped: &str) -> String {
    if stripped.starts_with("/v/") || stripped == "/v" {
        format!("/snappymail{stripped}")
    } else {
        stripped.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn normalize_rejects_root_and_dotdot() {
        assert!(normalize_public_path("/").is_err());
        assert!(normalize_public_path("/../x").is_err());
        assert_eq!(normalize_public_path("snappymail").unwrap(), "/snappymail");
    }

    #[test]
    fn config_roundtrip_and_regenerate() {
        with_test_data_dir(|| {
            let cfg = WebmailPanelConfig {
                public_path: "/snappymail".into(),
                auto_login_account: "user@example.com".into(),
                internal_embed: true,
            };
            save_webmail_config(&cfg).unwrap();
            let loaded = load_webmail_config();
            assert_eq!(loaded.public_path, "/snappymail");
            assert_eq!(loaded.auto_login_account, "user@example.com");
            assert!(loaded.internal_embed);
            let next = regenerate_webmail_path().unwrap();
            assert!(next.public_path.starts_with('/'));
            assert_ne!(next.public_path, "/snappymail");
            assert!(path_matches_webmail_mount(&format!(
                "{}/index.php",
                next.public_path
            )));
            assert_eq!(
                strip_webmail_mount(&format!("{}/index.php", next.public_path)).as_deref(),
                Some("/index.php")
            );
        });
    }

    #[test]
    fn snappymail_asset_paths_keep_app_prefix() {
        assert_eq!(
            remap_snappymail_stripped_asset("/v/2.38.2/static/js/min/libs.min.js"),
            "/snappymail/v/2.38.2/static/js/min/libs.min.js"
        );
        assert_eq!(remap_snappymail_stripped_asset("/index.php"), "/index.php");
        assert_eq!(remap_snappymail_stripped_asset("/v"), "/snappymail/v");
        assert_eq!(remap_snappymail_stripped_asset("/"), "/");
    }

    #[test]
    fn mount_trailing_slash_strips_to_root() {
        with_test_data_dir(|| {
            let cfg = WebmailPanelConfig {
                public_path: "/snappymail".into(),
                ..WebmailPanelConfig::default()
            };
            save_webmail_config(&cfg).unwrap();
            assert!(path_matches_webmail_mount("/snappymail"));
            assert!(path_matches_webmail_mount("/snappymail/"));
            assert!(path_matches_webmail_mount("/snappymail/index.php"));
            assert_eq!(strip_webmail_mount("/snappymail").as_deref(), Some("/"));
            assert_eq!(strip_webmail_mount("/snappymail/").as_deref(), Some("/"));
            assert_eq!(
                strip_webmail_mount("/snappymail/index.php").as_deref(),
                Some("/index.php")
            );
            assert_eq!(
                strip_webmail_mount("/snappymail/v/2.38.2/static/js/min/libs.min.js").as_deref(),
                Some("/v/2.38.2/static/js/min/libs.min.js")
            );
            assert_eq!(
                remap_snappymail_stripped_asset(
                    strip_webmail_mount("/snappymail/v/2.38.2/static/js/min/libs.min.js")
                        .as_deref()
                        .unwrap()
                ),
                "/snappymail/v/2.38.2/static/js/min/libs.min.js"
            );
        });
    }
}
