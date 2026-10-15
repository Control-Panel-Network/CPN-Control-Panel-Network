//! Optional per-site Apache handler toggle beside OpenLiteSpeed (OLS remains default).

use crate::paths::join_data;
use crate::sites::{SiteModify, SiteRecord, load_site, modify_site};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApacheHandlerState {
    pub domain: String,
    pub enabled: bool,
    pub updated_unix: u64,
}

fn state_path(domain: &str) -> PathBuf {
    join_data("apache-handlers").join(format!("{domain}.json"))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn is_apache_handler(domain: &str) -> bool {
    let Ok(raw) = fs::read_to_string(state_path(domain)) else {
        return false;
    };
    serde_json::from_str::<ApacheHandlerState>(&raw)
        .map(|s| s.enabled)
        .unwrap_or(false)
}

pub fn set_apache_handler(domain: &str, enabled: bool) -> Result<String, String> {
    let site = load_site(domain).map_err(|e| format!("Site not found: {e}"))?;
    let dir = join_data("apache-handlers");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create apache-handlers dir: {e}"))?;
    let state = ApacheHandlerState {
        domain: site.domain.clone(),
        enabled,
        updated_unix: now_unix(),
    };
    let raw = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
    let path = state_path(&site.domain);
    fs::write(&path, raw).map_err(|e| format!("Cannot save Apache handler state: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }

    // Persist engine hint on the site record when possible. OLS remains the product default.
    let engine = if enabled {
        Some("httpd".to_string())
    } else {
        Some("openlitespeed".to_string())
    };
    let _ = modify_site(
        &site.domain,
        SiteModify {
            engine,
            ..Default::default()
        },
    );

    if enabled {
        Ok(format!(
            "Apache handler enabled for {}. OpenLiteSpeed remains the default for new sites. Restart httpd after vhost wiring.",
            site.domain
        ))
    } else {
        Ok(format!(
            "Apache handler disabled for {}. Site engine preference restored toward OpenLiteSpeed.",
            site.domain
        ))
    }
}

pub fn apache_toggle_html(site: &SiteRecord, csrf: &str) -> String {
    let on = is_apache_handler(&site.domain);
    let checked = if on { " checked" } else { "" };
    format!(
        r#"<form method="post" action="/websites/apps" class="stack-form" style="margin-top:12px;">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="csrf" value="{csrf}">
  <input type="hidden" name="action" value="apache_handler_toggle">
  <label><input type="checkbox" name="enabled" value="1"{checked}> Apache handler for this site (optional; OpenLiteSpeed stays the default stack)</label>
  <button type="submit" class="btn-secondary" style="margin-top:8px;">Save handler</button>
</form>"#,
        domain = html_escape(&site.domain),
        csrf = html_escape(csrf),
        checked = checked,
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn toggle_state_roundtrip_without_site_registry() {
        with_test_data_dir(|| {
            // Direct state write path used when registry site is absent in unit tests.
            let dir = join_data("apache-handlers");
            fs::create_dir_all(&dir).unwrap();
            let state = ApacheHandlerState {
                domain: "example.test".into(),
                enabled: true,
                updated_unix: 1,
            };
            fs::write(
                state_path("example.test"),
                serde_json::to_string(&state).unwrap(),
            )
            .unwrap();
            assert!(is_apache_handler("example.test"));
        });
    }
}
