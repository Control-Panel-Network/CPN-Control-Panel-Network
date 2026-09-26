//! Create Website page: Cloudflare zone picker vs local DNS, ACL owner/docroot fields.

use crate::account_mgmt::list_accounts;
use crate::packages::is_panel_admin;
use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_cloudflare_verify::list_accessible_zones;
use crate::sites::normalize_domain;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn section_heading(title: &str, blurb: &str) -> String {
    format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">CPN PANEL</p>
          <h1>{title}</h1>
          <p>{blurb}</p>
        </div>
      </div>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
    )
}

fn notice_block(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{msg}</p>"#,
        msg = html_escape(message)
    )
}

/// True when the FQDN equals a zone or is a subdomain under it.
pub fn domain_under_cloudflare_zone(domain: &str, zone: &str) -> bool {
    let d = domain.trim().to_ascii_lowercase();
    let z = zone.trim().to_ascii_lowercase();
    if d.is_empty() || z.is_empty() {
        return false;
    }
    d == z || d.ends_with(&format!(".{z}"))
}

/// Resolve submitted domain from free-text or Cloudflare zone + optional subdomain.
pub fn resolve_create_domain(
    domain_raw: &str,
    cf_zone: &str,
    cf_subdomain: &str,
    cloudflare_mode: bool,
) -> Result<String, String> {
    if cloudflare_mode {
        let zone = cf_zone.trim().to_ascii_lowercase();
        if zone.is_empty() {
            return Err("Select a Cloudflare zone (domain)".into());
        }
        let sub = cf_subdomain.trim().trim_matches('.').to_ascii_lowercase();
        let combined = if sub.is_empty() {
            zone.clone()
        } else if sub == zone || sub.ends_with(&format!(".{zone}")) {
            sub
        } else {
            format!("{sub}.{zone}")
        };
        let domain = normalize_domain(&combined)?;
        if !domain_under_cloudflare_zone(&domain, &zone) {
            return Err(
                "Domain must be the selected Cloudflare zone or a subdomain under it".into(),
            );
        }
        return Ok(domain);
    }
    normalize_domain(domain_raw)
}

/// Enforce Cloudflare allowlist when the operator has Cloudflare credentials.
pub fn require_domain_in_cloudflare_zones(domain: &str) -> Result<(), String> {
    if !cloudflare_configured() {
        return Ok(());
    }
    let zones = list_accessible_zones(100)
        .map_err(|e| format!("Cloudflare is connected but zones could not be listed: {e}"))?;
    if zones.is_empty() {
        return Err(
            "Cloudflare is connected but no active zones were found. Add a zone in Cloudflare first."
                .into(),
        );
    }
    if zones
        .iter()
        .any(|z| domain_under_cloudflare_zone(domain, z))
    {
        return Ok(());
    }
    Err(format!(
        "Domain `{domain}` is not under an accessible Cloudflare zone. Pick a zone from Create Website."
    ))
}

fn domain_fields_html(cloudflare_mode: bool, zones: &[String], zone_error: Option<&str>) -> String {
    if cloudflare_mode {
        let mut options = String::from(r#"<option value="">Select Cloudflare zone…</option>"#);
        for zone in zones {
            options.push_str(&format!(
                r#"<option value="{v}">{v}</option>"#,
                v = html_escape(zone)
            ));
        }
        let err = zone_error
            .map(|e| {
                format!(
                    r#"<p class="panel-notice error" role="status">{msg}</p>"#,
                    msg = html_escape(e)
                )
            })
            .unwrap_or_default();
        let empty_hint = if zones.is_empty() {
            r#"<p class="muted">No active Cloudflare zones returned. Add a zone in Cloudflare, then reload this page.</p>"#
        } else {
            ""
        };
        format!(
            r#"<p class="muted"><strong>DNS mode:</strong> Cloudflare connected. Domain must be one of your Cloudflare zones (or a subdomain under that zone).</p>
          {err}
          {empty_hint}
          <label for="cf_zone">Cloudflare zone</label>
          <select id="cf_zone" name="cf_zone" required {disabled}>
            {options}
          </select>
          <label for="cf_subdomain">Subdomain (optional)</label>
          <input id="cf_subdomain" name="cf_subdomain" type="text" placeholder="blog" autocomplete="off" {disabled}>
          <p class="muted">Leave subdomain empty to create the apex zone (example.com). Enter <code>blog</code> for <code>blog.example.com</code>. Parent site must exist first for subdomains.</p>"#,
            err = err,
            empty_hint = empty_hint,
            options = options,
            disabled = if zones.is_empty() { "disabled" } else { "" },
        )
    } else {
        r#"<p class="muted"><strong>DNS mode:</strong> Local DNS (Cloudflare not connected). Enter any valid domain; existing validation still applies. Subdomains require the parent domain first.</p>
          <label for="domain">Domain</label>
          <input id="domain" name="domain" type="text" required placeholder="example.com" autocomplete="off">"#
            .to_string()
    }
}

fn owner_fields_html(viewer: &str, admin: bool) -> String {
    if admin {
        let accounts = list_accounts().unwrap_or_default();
        let mut options = String::new();
        let mut saw_self = false;
        for acct in &accounts {
            let selected = if acct.username.eq_ignore_ascii_case(viewer) {
                saw_self = true;
                " selected"
            } else {
                ""
            };
            options.push_str(&format!(
                r#"<option value="{v}"{selected}>{v}</option>"#,
                v = html_escape(&acct.username),
                selected = selected
            ));
        }
        if !saw_self {
            options.insert_str(
                0,
                &format!(
                    r#"<option value="{v}" selected>{v}</option>"#,
                    v = html_escape(viewer)
                ),
            );
        }
        format!(
            r#"<label for="owner">Owner</label>
          <select id="owner" name="owner" required>
            {options}
          </select>
          <p class="muted">Admins may assign the site to any panel account.</p>"#,
            options = options
        )
    } else {
        format!(
            r#"<label for="owner">Owner</label>
          <input id="owner" name="owner" type="text" value="{v}" readonly autocomplete="username">
          <p class="muted">Package accounts own sites as themselves. Owner cannot be changed.</p>"#,
            v = html_escape(viewer)
        )
    }
}

fn docroot_fields_html(admin: bool) -> String {
    if admin {
        r#"<label for="docroot">Docroot (optional)</label>
          <input id="docroot" name="docroot" type="text" placeholder="/home/example.com/public_html">
          <p class="muted"><strong>Warning:</strong> Leave blank to use the default under the domain home. A custom docroot can break package ACL paths and File Manager jails. Override only when you know the path layout.</p>"#
            .to_string()
    } else {
        r#"<input type="hidden" name="docroot" value="">
          <p class="muted">Document root uses the package default under the domain home. Custom docroots are reserved for administrators (changing them can break ACL and package paths).</p>"#
            .to_string()
    }
}

/// Full Create Website page body (form only).
pub fn websites_create_main(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let admin = is_panel_admin(username);
    let cf = cloudflare_configured();
    let (zones, zone_error) = if cf {
        match list_accessible_zones(100) {
            Ok(z) => (z, None),
            Err(e) => (Vec::new(), Some(e)),
        }
    } else {
        (Vec::new(), None)
    };
    // Prefer the zone picker when Cloudflare is connected and zones loaded.
    let use_cf_picker = cf && zone_error.is_none() && !zones.is_empty();
    let domain_html = if cf && !use_cf_picker {
        let fallback_err = zone_error
            .as_deref()
            .unwrap_or("Cloudflare is connected but no zones are available yet.");
        format!(
            r#"<p class="panel-notice error" role="status">{msg}</p>
          <p class="muted"><strong>DNS mode:</strong> Cloudflare connected, but the zone picker is unavailable. Enter a domain that belongs to one of your Cloudflare zones (server will verify).</p>
          <label for="domain">Domain</label>
          <input id="domain" name="domain" type="text" required placeholder="example.com" autocomplete="off">"#,
            msg = html_escape(fallback_err)
        )
    } else {
        domain_fields_html(use_cf_picker, &zones, None)
    };

    format!(
        r#"{heading}
      {ok}
      {err}
      <article class="section-card">
        <h2>Create Website</h2>
        <p>Creates the site home and document root. Subdomains require the parent domain first.</p>
        <p class="muted"><a href="/websites">Back to site list</a></p>
        <form method="post" action="/websites/create" class="stack-form" style="max-width:560px;">
          {domain_html}
          {owner_html}
          {docroot_html}
          <button type="submit" class="btn-primary">Create site</button>
        </form>
      </article>"#,
        heading = section_heading(
            "Create Website",
            "Add a domain home under /home and optional document root.",
        ),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        domain_html = domain_html,
        owner_html = owner_fields_html(username, admin),
        docroot_html = docroot_fields_html(admin),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloudflare_zone_match_apex_and_sub() {
        assert!(domain_under_cloudflare_zone("example.com", "example.com"));
        assert!(domain_under_cloudflare_zone(
            "blog.example.com",
            "example.com"
        ));
        assert!(domain_under_cloudflare_zone(
            "a.b.example.com",
            "example.com"
        ));
        assert!(!domain_under_cloudflare_zone("example.org", "example.com"));
        assert!(!domain_under_cloudflare_zone(
            "notexample.com",
            "example.com"
        ));
    }

    #[test]
    fn resolve_create_domain_local_and_cf() {
        assert_eq!(
            resolve_create_domain("Example.COM", "", "", false).unwrap(),
            "example.com"
        );
        assert_eq!(
            resolve_create_domain("", "example.com", "", true).unwrap(),
            "example.com"
        );
        assert_eq!(
            resolve_create_domain("", "example.com", "blog", true).unwrap(),
            "blog.example.com"
        );
        assert_eq!(
            resolve_create_domain("", "example.com", "blog.example.com", true).unwrap(),
            "blog.example.com"
        );
        assert!(resolve_create_domain("", "", "blog", true).is_err());
    }
}
