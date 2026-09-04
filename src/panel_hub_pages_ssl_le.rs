//! Manage SSL UI: per-domain providers (Let's Encrypt, ZeroSSL, Cloudflare CA, Custom, None).

use crate::panel_hubs::feature_shell;
use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_ssl_le::{
    SslStatusRow, certbot_available, cloudflare_dns_plugin_available, ssl_status_all_sites,
};
use crate::panel_ops_ssl_provider::{SslProvider, load_ssl_defaults};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn policy_blurb() -> String {
    let defaults = load_ssl_defaults();
    let certbot = if certbot_available() {
        "certbot detected"
    } else {
        "certbot not on PATH"
    };
    let cf = if cloudflare_configured() {
        "Cloudflare API configured (token reused for Cloudflare CA / DNS-01)"
    } else {
        "Cloudflare API not configured"
    };
    let dns01 = if cloudflare_dns_plugin_available() {
        "DNS-01 Cloudflare plugin available"
    } else {
        "DNS-01 plugin not detected (webroot when possible)"
    };
    format!(
        r#"<ul class="kv-list">
  <li><span>New-site default</span><strong>{def}</strong> (installer/CLI only; never rewrites existing domains)</li>
  <li><span>certbot</span><strong>{certbot}</strong></li>
  <li><span>Cloudflare</span><strong>{cf}</strong></li>
  <li><span>Challenge</span><strong>{dns01}</strong></li>
</ul>
<p class="muted">Each domain and subdomain has its own SSL provider. None skips issue/renew.
Custom is upload-only (no auto-renew). SAN/shared certs: enable "include subdomains" on the apex when children share the same auto provider; a child on Custom/None leaves the shared cert.</p>"#,
        def = html_escape(defaults.default_provider.label()),
        certbot = html_escape(certbot),
        cf = html_escape(cf),
        dns01 = html_escape(dns01),
    )
}

fn provider_badge(label: &str) -> String {
    format!(r#"<span class="ssl-badge">{}</span>"#, html_escape(label))
}

fn rows_table(rows: &[SslStatusRow]) -> String {
    if rows.is_empty() {
        return r#"<p class="empty-state">No sites registered. Create websites first.</p>"#.into();
    }
    let mut body = String::new();
    for r in rows {
        let status = if r.has_cert {
            "Certificate on disk"
        } else if r.needs_issue {
            "Needs issue"
        } else if r.provider == "none" {
            "None (skipped)"
        } else {
            "Custom / waiting"
        };
        let shared = r
            .shared_cert_owner
            .as_deref()
            .map(|o| format!("shared:{o}"))
            .unwrap_or_else(|| "-".into());
        let err = if r.last_error.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="muted" style="max-width:280px;">{}</div>"#,
                html_escape(&r.last_error)
            )
        };
        let issue_btn = if r.auto_issue {
            format!(
                r#"<form method="post" action="/security/ssl/issue" style="display:inline;">
      <input type="hidden" name="domain" value="{d}">
      <button type="submit" class="btn-primary">Issue / Renew</button>
    </form>"#,
                d = html_escape(&r.domain)
            )
        } else {
            String::new()
        };
        body.push_str(&format!(
            r#"<tr>
  <td><code>{domain}</code> {badge}</td>
  <td>{status}</td>
  <td>{shared}</td>
  <td>
    <form method="post" action="/security/ssl/provider" class="inline-form" style="display:inline-flex;gap:4px;flex-wrap:wrap;">
      <input type="hidden" name="domain" value="{domain}">
      <select name="provider">{opts}</select>
      <button type="submit">Set</button>
    </form>
    {issue}
    {err}
  </td>
</tr>"#,
            domain = html_escape(&r.domain),
            badge = provider_badge(&r.provider_label),
            status = html_escape(status),
            shared = html_escape(&shared),
            opts = provider_options(&r.provider),
            issue = issue_btn,
            err = err,
        ));
    }
    format!(
        r#"<table class="cf-table" style="width:100%;border-collapse:collapse;">
  <thead><tr><th>Domain</th><th>Status</th><th>Shared</th><th>Provider / Actions</th></tr></thead>
  <tbody>{body}</tbody>
</table>
<style>
.cf-table th,.cf-table td {{ text-align:left; padding:8px 6px; border-bottom:1px solid var(--border,#333); font-size:13px; vertical-align:top; }}
.ssl-badge {{ display:inline-block; margin-left:6px; padding:2px 8px; border-radius:999px; font-size:11px; background:rgba(124,92,255,.2); }}
</style>"#
    )
}

fn provider_options(selected: &str) -> String {
    let mut out = String::new();
    for p in SslProvider::all() {
        let sel = if p.as_str() == selected {
            " selected"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<option value="{v}"{sel}>{l}</option>"#,
            v = p.as_str(),
            l = html_escape(p.label()),
            sel = sel,
        ));
    }
    out
}

pub fn manage_ssl_page(notice: Option<&str>, error: Option<&str>) -> String {
    let rows = ssl_status_all_sites();
    let body = format!(
        r#"{policy}
<div style="margin:16px 0;display:flex;flex-wrap:wrap;gap:8px;">
  <form method="post" action="/security/ssl/issue-all">
    <button type="submit" class="btn-primary">Issue all needing auto ACME</button>
  </form>
  <form method="post" action="/security/ssl/renew">
    <button type="submit">Renew auto providers (certbot renew)</button>
  </form>
  <a class="btn-primary" href="/dns/cloudflare" style="display:inline-flex;align-items:center;text-decoration:none;padding:8px 12px;">Cloudflare DNS</a>
</div>
<form method="post" action="/security/ssl/defaults" class="stack-form" style="max-width:420px;margin-bottom:16px;">
  <label for="default_provider">Default SSL provider for <strong>new</strong> sites</label>
  <select id="default_provider" name="provider">{opts}</select>
  <button type="submit">Save new-site default</button>
</form>
<h3>Per-domain SSL</h3>
{table}
<p class="muted">Labs without public DNS (for example <code>cpn-lab-test.example</code>) show honest ACME failures. Limitations: LE/ZeroSSL rate limits; Cloudflare Origin CA CSR API is partial (token + DNS-01 / Custom upload).</p>"#,
        policy = policy_blurb(),
        opts = provider_options(load_ssl_defaults().default_provider.as_str()),
        table = rows_table(&rows),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Manage SSL", None),
        ],
        "Manage SSL",
        "Per-domain SSL providers. Account-wide rewrite is not supported.",
        &body,
        notice,
        error,
    )
}

pub fn security_ssl_hub_page() -> String {
    feature_shell(
        &[("Dashboard", Some("/dashboard")), ("Security", None)],
        "Security",
        "SSL and related controls for CPN.",
        r#"<div class="hub-tile-grid">
  <a class="hub-tile" href="/security/ssl"><span class="hub-tile-copy"><strong>Manage SSL</strong><span>Per-domain Let's Encrypt, ZeroSSL, Cloudflare CA, Custom, None</span></span><span class="hub-badge live">Live</span></a>
  <a class="hub-tile" href="/dns/cloudflare"><span class="hub-tile-copy"><strong>Cloudflare DNS</strong><span>API settings and DNS records</span></span><span class="hub-badge live">Live</span></a>
</div>"#,
        None,
        None,
    )
}
