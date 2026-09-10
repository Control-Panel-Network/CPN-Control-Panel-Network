//! Manage site SSL tab (provider, coverage Wildcard/SAN, issue, custom upload).

use crate::panel_ops_ssl_le::{effective_coverage, ssl_status_for_domain};
use crate::panel_ops_ssl_provider::{SslCoverageMode, SslProvider};
use crate::panel_website_manage_ui::{html_escape, section, tile};
use crate::sites::SiteRecord;
use crate::website_preview::ssl_material_present;

pub fn tab_ssl(site: &SiteRecord) -> String {
    let domain_q = html_escape(&site.domain);
    let row = ssl_status_for_domain(&site.domain);
    let has = ssl_material_present(&site.domain);
    let mut tiles = String::from(r#"<div class="manage-tile-grid">"#);
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=ssl#provider"),
        "SSL provider",
        &row.provider_label,
    ));
    tiles.push_str(&tile(
        "/security/ssl",
        "Manage SSL (all sites)",
        "Per-domain providers; no account-wide rewrite",
    ));
    tiles.push_str(&tile(
        &format!("/websites/manage?domain={domain_q}&tab=ssl#manual"),
        "Custom upload",
        "PEM cert + key (no auto-renew)",
    ));
    tiles.push_str("</div>");

    let mut opts = String::new();
    for p in SslProvider::all() {
        let sel = if p.as_str() == row.provider {
            " selected"
        } else {
            ""
        };
        opts.push_str(&format!(
            r#"<option value="{v}"{sel}>{l}</option>"#,
            v = p.as_str(),
            l = html_escape(p.label()),
            sel = sel,
        ));
    }
    let coverage = effective_coverage(&site.ssl);
    let wild_sel = if coverage == SslCoverageMode::Wildcard {
        " checked"
    } else {
        ""
    };
    let san_sel = if coverage == SslCoverageMode::San {
        " checked"
    } else {
        ""
    };
    let provider_form = format!(
        r#"<div id="provider"><h3>Provider for {domain} only</h3>
<p class="manage-muted">Changing this domain does not rewrite siblings. Subdomains inherit parent provider only at creation time.</p>
<form method="post" action="/security/ssl/provider" class="stack-form" style="max-width:520px;">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="return" value="/websites/manage?domain={domain_q}&amp;tab=ssl">
  <label for="provider">SSL provider</label>
  <select id="provider" name="provider">{opts}</select>
  <fieldset style="border:1px solid var(--m-line,#2a2f3a);border-radius:12px;padding:12px 14px;margin:12px 0;">
    <legend style="padding:0 6px;font-weight:700;font-size:13px;">Certificate coverage</legend>
    <label style="display:flex;gap:8px;align-items:flex-start;margin:8px 0;">
      <input type="radio" name="coverage_mode" value="wildcard"{wild_sel}>
      <span><strong>Wildcard</strong> (default): one cert for <code>{domain}</code> and <code>*.{domain}</code>. Covers unlimited subdomains. Let's Encrypt / ZeroSSL need DNS-01 (Cloudflare token + certbot-dns-cloudflare).</span>
    </label>
    <label style="display:flex;gap:8px;align-items:flex-start;margin:8px 0;">
      <input type="radio" name="coverage_mode" value="san"{san_sel}>
      <span><strong>SAN</strong>: one multi-name cert for this domain plus matching panel subdomains that share the same provider (listed FQDNs). HTTP-01 webroot is OK when DNS-01 is unavailable.</span>
    </label>
    <p class="manage-muted" style="margin:8px 0 0;">Wildcard suits many hostnames under one domain. SAN suits a fixed list of names (or several distinct hostnames) on one certificate.</p>
  </fieldset>
  <button type="submit" class="manage-btn primary">Save provider</button>
</form>
<p class="manage-muted">Status: {cert}. Coverage: {cov}. Shared owner: {shared}.</p>{err}</div>"#,
        domain = html_escape(&site.domain),
        domain_q = domain_q,
        opts = opts,
        wild_sel = wild_sel,
        san_sel = san_sel,
        cov = html_escape(coverage.label()),
        cert = if has {
            "certificate material on disk"
        } else {
            "no certificate files found"
        },
        shared = html_escape(row.shared_cert_owner.as_deref().unwrap_or("-")),
        err = if row.last_error.is_empty() {
            String::new()
        } else {
            format!(
                r#"<p class="panel-notice error" role="status">{}</p>"#,
                html_escape(&row.last_error)
            )
        },
    );

    let issue = if row.auto_issue {
        format!(
            r#"<div id="issue"><h3>Issue / Renew</h3>
<form method="post" action="/security/ssl/issue">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="return" value="/websites/manage?domain={domain_q}&amp;tab=ssl">
  <button type="submit" class="manage-btn primary">Issue / Renew ({label})</button>
</form>
<p class="manage-muted">certbot available: {cb}. Wildcard uses DNS-01 when required. Honest errors on rate limits or missing public DNS.</p></div>"#,
            domain = html_escape(&site.domain),
            domain_q = domain_q,
            label = html_escape(&row.provider_label),
            cb = if row.certbot { "yes" } else { "no" },
        )
    } else {
        format!(
            r#"<div id="issue"><h3>Issue / Renew</h3>
<p class="manage-muted">Provider <strong>{}</strong> does not auto-issue. Choose Let's Encrypt, ZeroSSL, or Cloudflare CA, or upload Custom SSL.</p></div>"#,
            html_escape(&row.provider_label)
        )
    };

    let manual = format!(
        r#"<div id="manual"><h3>Custom SSL upload</h3>
<p class="manage-muted">Uploading sets this domain to Custom and leaves any shared SAN. Keys stored under <code>/var/lib/cpn/ssl/{domain}/</code> mode 600.</p>
<form method="post" action="/security/ssl/upload" class="stack-form" style="max-width:640px;">
  <input type="hidden" name="domain" value="{domain}">
  <input type="hidden" name="return" value="/websites/manage?domain={domain_q}&amp;tab=ssl">
  <label for="cert_pem">Certificate PEM (fullchain)</label>
  <textarea id="cert_pem" name="cert_pem" rows="6" style="width:100%;font:inherit;" required></textarea>
  <label for="key_pem">Private key PEM</label>
  <textarea id="key_pem" name="key_pem" rows="6" style="width:100%;font:inherit;" required></textarea>
  <button type="submit" class="manage-btn primary">Upload custom SSL</button>
</form></div>"#,
        domain = html_escape(&site.domain),
        domain_q = domain_q,
    );

    format!(
        "{tiles}{provider}{issue}{manual}",
        tiles = section("SSL", &tiles),
        provider = provider_form,
        issue = issue,
        manual = manual,
    )
}
