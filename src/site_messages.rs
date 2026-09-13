//! Global and per-site message templates for suspended sites and new docroots.
//!
//! Primary store: `$CPN_DATA_DIR/site-messages.json` (mode 600).
//! Site-owned suspend copy lives on each `SiteRecord`.

use crate::account::{data_dir, now_unix};
use crate::sites::{SiteRecord, SuspendActor};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

const SCHEMA_VERSION: u32 = 1;
const MAX_SUSPEND_CHARS: usize = 8_000;
const MAX_SITE_READY_CHARS: usize = 64_000;

const BUILTIN_SUSPEND: &str = "This website is temporarily unavailable. Please try again later.";

const BUILTIN_SITE_READY: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Site ready</title>
</head>
<body>
  <h1>Site ready</h1>
  <p>This document root was created by CPN. Replace this file with your site.</p>
</body>
</html>
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteMessageDefaults {
    pub schema_version: u32,
    /// CPN/global default shown when an admin suspends a site (and as owner fallback).
    pub suspend_message_html: String,
    /// Default `index.html` written into new document roots.
    pub site_ready_html: String,
    pub updated_at_unix: u64,
}

impl Default for SiteMessageDefaults {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            suspend_message_html: BUILTIN_SUSPEND.to_string(),
            site_ready_html: BUILTIN_SITE_READY.to_string(),
            updated_at_unix: 0,
        }
    }
}

pub fn site_messages_path() -> PathBuf {
    data_dir().join("site-messages.json")
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
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
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(contents)
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Ensure the global defaults file exists (migration hook / first use).
pub fn ensure_site_messages_migrated() -> Result<(), String> {
    let path = site_messages_path();
    if path.is_file() {
        let _ = load_defaults();
        return Ok(());
    }
    save_defaults(&SiteMessageDefaults::default())
}

pub fn load_defaults() -> SiteMessageDefaults {
    let path = site_messages_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return SiteMessageDefaults::default();
    };
    let mut loaded: SiteMessageDefaults = serde_json::from_str(&raw).unwrap_or_default();
    if loaded.suspend_message_html.trim().is_empty() {
        loaded.suspend_message_html = BUILTIN_SUSPEND.to_string();
    }
    if loaded.site_ready_html.trim().is_empty() {
        loaded.site_ready_html = BUILTIN_SITE_READY.to_string();
    }
    loaded.schema_version = SCHEMA_VERSION;
    loaded
}

pub fn save_defaults(defaults: &SiteMessageDefaults) -> Result<(), String> {
    let mut out = defaults.clone();
    out.schema_version = SCHEMA_VERSION;
    out.updated_at_unix = now_unix();
    out.suspend_message_html = sanitize_message_body(&out.suspend_message_html)?;
    out.site_ready_html = sanitize_site_ready_html(&out.site_ready_html)?;
    let json = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize site messages: {e}"))?;
    write_mode_600(&site_messages_path(), json.as_bytes())
}

pub fn builtin_site_ready_html() -> &'static str {
    BUILTIN_SITE_READY
}

pub fn builtin_suspend_message() -> &'static str {
    BUILTIN_SUSPEND
}

/// Restore only the global suspend message to the built-in factory text.
pub fn restore_factory_suspend_message() -> Result<(), String> {
    let mut defaults = load_defaults();
    defaults.suspend_message_html = BUILTIN_SUSPEND.to_string();
    save_defaults(&defaults)
}

/// Restore only the site-ready template to the built-in factory HTML.
pub fn restore_factory_site_ready() -> Result<(), String> {
    let mut defaults = load_defaults();
    defaults.site_ready_html = BUILTIN_SITE_READY.to_string();
    save_defaults(&defaults)
}

pub fn site_ready_html_for_new_docroot() -> String {
    let defaults = load_defaults();
    if defaults.site_ready_html.trim().is_empty() {
        BUILTIN_SITE_READY.to_string()
    } else {
        defaults.site_ready_html
    }
}

/// Escape plain text and turn newlines into `<br>` for safe HTML fragments.
pub fn escape_plain_to_html(text: &str) -> String {
    html_escape(text).replace('\n', "<br>\n")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Suspend / owner messages: store as plain text (XSS-safe). Empty rejected only when required.
pub fn sanitize_message_body(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.chars().count() > MAX_SUSPEND_CHARS {
        return Err(format!(
            "Message is too long (max {MAX_SUSPEND_CHARS} characters)"
        ));
    }
    if trimmed
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
    {
        return Err("Message cannot include control characters".into());
    }
    // Strip any HTML-looking tags so stored copy is plain text.
    let mut out = String::with_capacity(trimmed.len());
    let mut in_tag = false;
    for ch in trimmed.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    Ok(out.trim().to_string())
}

/// Full HTML document for new sites: strip scripts, handlers, and dangerous URLs.
pub fn sanitize_site_ready_html(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(BUILTIN_SITE_READY.to_string());
    }
    if trimmed.chars().count() > MAX_SITE_READY_CHARS {
        return Err(format!(
            "Site ready template is too long (max {MAX_SITE_READY_CHARS} characters)"
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    for needle in [
        "<script",
        "</script",
        "javascript:",
        "vbscript:",
        "data:text/html",
        "<iframe",
        "<object",
        "<embed",
        "<link",
        "<meta http-equiv",
        "onerror=",
        "onload=",
        "onclick=",
        "onmouseover=",
        "onfocus=",
        "onsubmit=",
    ] {
        if lower.contains(needle) {
            return Err(format!(
                "Site ready template cannot include unsafe content ({needle})"
            ));
        }
    }
    Ok(trimmed.to_string())
}

/// Resolve which suspend message to show for a disabled site.
/// Admin suspend always uses the CPN/global default. Owner suspend uses the
/// per-site message, falling back to the global default when empty.
pub fn effective_suspend_message(site: &SiteRecord) -> String {
    let defaults = load_defaults();
    let global = if defaults.suspend_message_html.trim().is_empty() {
        BUILTIN_SUSPEND.to_string()
    } else {
        defaults.suspend_message_html
    };
    match site.suspended_by {
        Some(SuspendActor::Admin) => global,
        Some(SuspendActor::Owner) => {
            let custom = site.owner_suspend_message.trim();
            if custom.is_empty() {
                global
            } else {
                custom.to_string()
            }
        }
        None => {
            // Legacy / unknown: treat like admin (panel default), never owner custom.
            global
        }
    }
}

/// HTML page served in preview (and available for future vhost ErrorDocument).
pub fn render_suspend_page(site: &SiteRecord) -> String {
    let body = escape_plain_to_html(&effective_suspend_message(site));
    let domain = html_escape(&site.domain);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Site suspended · {domain}</title>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 0; min-height: 100vh; display: grid; place-items: center; background: #f4f6f8; color: #1a1a1a; }}
    main {{ max-width: 36rem; padding: 2rem; text-align: center; }}
    h1 {{ font-size: 1.5rem; margin: 0 0 0.75rem; }}
    p {{ line-height: 1.5; margin: 0; }}
  </style>
</head>
<body>
  <main>
    <h1>Site suspended</h1>
    <p>{body}</p>
  </main>
</body>
</html>
"#
    )
}

/// Write sanitized owner message onto the site record field (caller persists).
pub fn sanitize_owner_suspend_message(raw: &str) -> Result<String, String> {
    sanitize_message_body(raw)
}

/// Overwrite `index.html` in the site docroot with the current global site-ready template.
pub fn reset_placeholder_index(docroot: &str) -> Result<(), String> {
    let path = PathBuf::from(docroot).join("index.html");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let html = site_ready_html_for_new_docroot();
    fs::write(&path, html.as_bytes())
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::DATA_DIR_TEST_LOCK;
    use crate::sites::{SiteRecord, SuspendActor};

    fn with_temp_data<T>(f: impl FnOnce() -> T) -> T {
        let _guard = DATA_DIR_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = std::env::temp_dir().join(format!("cpn-msg-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &dir);
        }
        let result = f();
        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
        }
        let _ = fs::remove_dir_all(&dir);
        result
    }

    fn sample_site(enabled: bool, by: Option<SuspendActor>, owner_msg: &str) -> SiteRecord {
        SiteRecord {
            schema_version: 3,
            domain: "demo.example".into(),
            owner: "alice".into(),
            docroot: "/tmp/demo".into(),
            enabled,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
            ssl: Default::default(),
            internal_ip: None,
            owner_suspend_message: owner_msg.into(),
            suspended_by: by,
            php_version: None,
        }
    }

    #[test]
    fn migrate_creates_defaults_file() {
        with_temp_data(|| {
            ensure_site_messages_migrated().unwrap();
            assert!(site_messages_path().is_file());
            let d = load_defaults();
            assert!(d.site_ready_html.contains("Site ready"));
            assert!(!d.suspend_message_html.is_empty());
        });
    }

    #[test]
    fn admin_suspend_ignores_owner_message() {
        with_temp_data(|| {
            let site = sample_site(false, Some(SuspendActor::Admin), "Owner custom text");
            let msg = effective_suspend_message(&site);
            assert!(!msg.contains("Owner custom"));
            assert!(msg.contains("temporarily unavailable") || !msg.is_empty());
        });
    }

    #[test]
    fn owner_suspend_uses_custom_then_fallback() {
        with_temp_data(|| {
            let custom = sample_site(false, Some(SuspendActor::Owner), "Back soon from owner");
            assert_eq!(effective_suspend_message(&custom), "Back soon from owner");
            let empty = sample_site(false, Some(SuspendActor::Owner), "");
            assert_eq!(
                effective_suspend_message(&empty),
                load_defaults().suspend_message_html
            );
        });
    }

    #[test]
    fn sanitize_strips_tags_and_blocks_scripts() {
        assert_eq!(
            sanitize_message_body("<b>Hi</b> there").unwrap(),
            "Hi there"
        );
        assert!(sanitize_site_ready_html("<script>alert(1)</script><h1>x</h1>").is_err());
        assert!(sanitize_site_ready_html(BUILTIN_SITE_READY).is_ok());
    }

    #[test]
    fn restore_factory_suspend_only() {
        with_temp_data(|| {
            ensure_site_messages_migrated().unwrap();
            let mut d = load_defaults();
            d.suspend_message_html = "custom global".into();
            d.site_ready_html = "<html><body>keep</body></html>".into();
            save_defaults(&d).unwrap();
            restore_factory_suspend_message().unwrap();
            let loaded = load_defaults();
            assert_eq!(loaded.suspend_message_html, BUILTIN_SUSPEND);
            assert!(loaded.site_ready_html.contains("keep"));
        });
    }

    #[test]
    fn no_em_dash_in_builtins() {
        assert!(!BUILTIN_SUSPEND.contains('\u{2014}'));
        assert!(!BUILTIN_SUSPEND.contains('\u{2013}'));
        assert!(!BUILTIN_SITE_READY.contains('\u{2014}'));
    }
}
