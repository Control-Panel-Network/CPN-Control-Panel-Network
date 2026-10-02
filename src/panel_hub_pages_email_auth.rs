//! MTA-STS and BIMI Email hub pages.

use crate::panel_hubs::feature_shell;
use crate::panel_ops_cloudflare::cloudflare_configured;
use crate::panel_ops_email_auth::{
    BimiSettings, DnsRecordPlan, MtaStsSettings, bimi_dns_records, load_bimi, load_mta_sts,
    mta_sts_dns_records, policy_file_path_display, push_dns_plans_to_cloudflare,
    render_mta_sts_policy, save_bimi, save_mta_sts,
};
use crate::sites::list_sites;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn flash(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|v| !v.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{}</p>"#,
        html_escape(message)
    )
}

fn domain_options(selected: &str) -> String {
    domain_options_from(&list_sites().unwrap_or_default(), selected)
}

fn domain_options_from(sites: &[crate::sites::SiteRecord], selected: &str) -> String {
    let mut out = String::new();
    if sites.is_empty() {
        out.push_str(r#"<option value="">No websites yet</option>"#);
        return out;
    }
    for site in sites {
        let sel = if site.domain.eq_ignore_ascii_case(selected) {
            " selected"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<option value="{d}"{sel}>{d}</option>"#,
            d = html_escape(&site.domain),
            sel = sel,
        ));
    }
    out
}

fn dns_records_styles() -> &'static str {
    r#"
.email-auth-layout { display:grid; gap:16px; width:100%; max-width:100%; min-width:0; }
.email-auth-layout .stack-form { max-width:560px; width:100%; }
.email-auth-layout .email-auth-actions { margin-top:4px; }
.email-auth-layout .email-auth-actions .btn-primary,
.email-auth-layout .email-auth-actions .btn-secondary {
  min-height:40px; padding:0 16px; border-radius:999px; font-weight:700;
}
.email-auth-policy {
  margin:0; padding:14px 16px; border-radius:12px;
  border:1px solid var(--hairline, #2a2f3a); background:rgba(0,0,0,.22);
  white-space:pre-wrap; overflow-wrap:anywhere; word-break:break-word;
  max-width:100%; overflow-x:auto; font-size:13px; line-height:1.45;
}
.dns-cards { display:grid; gap:14px; width:100%; max-width:100%; margin-top:12px; }
.dns-card {
  display:grid; gap:12px; padding:14px 16px; min-width:0; width:100%; box-sizing:border-box;
  border:1px solid var(--hairline, #2a2f3a); border-radius:14px;
  background:var(--panel, #1a1d26);
}
.dns-card-head {
  display:flex; flex-wrap:wrap; gap:10px; align-items:center; justify-content:space-between;
}
.dns-card-type {
  display:inline-flex; align-items:center; min-height:28px; padding:0 12px; border-radius:999px;
  background:rgba(14,165,233,.16); color:#7dd3fc; font-size:12px; font-weight:800;
  letter-spacing:.04em; text-transform:uppercase;
}
.dns-card-meta {
  display:grid; grid-template-columns:repeat(auto-fit,minmax(160px,1fr)); gap:8px; min-width:0;
}
.dns-card-meta > div {
  border:1px solid var(--hairline, #2a2f3a); border-radius:10px; padding:8px 10px; min-width:0;
}
.dns-card-meta span { display:block; font-size:11px; color:#98a2b3; font-weight:600; }
.dns-card-meta strong, .dns-card-meta code {
  display:block; margin-top:4px; font-size:13px; font-weight:600; color:inherit;
  overflow-wrap:anywhere; word-break:break-word; white-space:normal; max-width:100%;
}
.dns-card-meta .dns-card-value { grid-column:1 / -1; }
@media (max-width:720px) {
  .email-auth-layout .stack-form { max-width:100%; }
  .email-auth-layout .email-auth-actions button,
  .email-auth-layout .email-auth-actions .btn-primary,
  .email-auth-layout .email-auth-actions .btn-secondary {
    width:100%; justify-content:center;
  }
  .dns-card-meta { grid-template-columns:1fr; }
  .dns-card-meta .dns-card-value { grid-column:auto; }
}
"#
}

/// Recommended DNS as padded cards (Websites-list style). Avoids crushed table columns.
fn dns_records_cards(records: &[DnsRecordPlan]) -> String {
    if records.is_empty() {
        return r#"<p class="empty-state" style="margin-top:12px;">No DNS records for this plan yet.</p>"#
            .to_string();
    }
    let mut cards = String::new();
    for r in records {
        let note = if r.note.trim().is_empty() {
            "-".to_string()
        } else {
            html_escape(&r.note)
        };
        cards.push_str(&format!(
            r#"<article class="dns-card">
  <div class="dns-card-head">
    <span class="dns-card-type">{ty}</span>
  </div>
  <div class="dns-card-meta">
    <div><span>Name</span><code>{name}</code></div>
    <div class="dns-card-value"><span>Value</span><code>{content}</code></div>
    <div><span>Notes</span><strong class="muted">{note}</strong></div>
  </div>
</article>"#,
            ty = html_escape(&r.record_type),
            name = html_escape(&r.name),
            content = html_escape(&r.content),
            note = note,
        ));
    }
    format!(r#"<div class="dns-cards">{cards}</div>"#)
}

pub fn email_mta_sts_page(domain: &str, notice: Option<&str>, error: Option<&str>) -> String {
    // One sites registry read for default domain + dropdown (avoid double list under disk load).
    let sites = list_sites().unwrap_or_default();
    let settings = if domain.trim().is_empty() {
        let first = sites.first().map(|s| s.domain.clone()).unwrap_or_default();
        load_mta_sts(&first)
    } else {
        load_mta_sts(domain)
    };
    let dns = mta_sts_dns_records(&settings);
    let policy = render_mta_sts_policy(&settings);
    let mx_joined = settings.mx.join("\n");
    let enabled = if settings.enabled { " checked" } else { "" };
    let cf_note = if cloudflare_configured() {
        "Cloudflare API token is present. Push will add or update these records only (nothing else is removed)."
    } else {
        "Cloudflare token not configured. Copy the records below, or add a token under Cloudflare DNS > API Settings."
    };
    let mode_opts = ["none", "testing", "enforce"]
        .iter()
        .map(|m| {
            let sel = if settings.mode == *m { " selected" } else { "" };
            format!(r#"<option value="{m}"{sel}>{m}</option>"#, m = m, sel = sel)
        })
        .collect::<String>();
    let body = format!(
        r#"<style>{dns_css}</style>
      <div class="email-auth-layout">
      {notice}{error}
      <p class="muted">MTA-STS tells receiving MTAs how to expect TLS for your domain. Cloudflare supports this mainly as DNS (TXT + policy host). Client webmail (SnappyMail/Roundcube) does not enforce MTA-STS; receivers do.</p>
      <form method="get" action="/email/mta-sts" class="stack-form">
        <label for="domain">Domain</label>
        <select id="domain" name="domain" onchange="this.form.submit()">{domains}</select>
      </form>
      <form method="post" action="/email/mta-sts/save" class="stack-form">
        <input type="hidden" name="domain" value="{domain}">
        <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
          <input type="checkbox" name="enabled" value="1"{enabled}> Enable MTA-STS plan for this domain
        </label>
        <label for="mode">Mode</label>
        <select id="mode" name="mode">{mode_opts}</select>
        <label for="max_age">max_age (seconds)</label>
        <input id="max_age" name="max_age" type="number" min="60" max="31536000" value="{max_age}">
        <label for="mx">MX hostnames (one per line)</label>
        <textarea id="mx" name="mx" rows="4">{mx}</textarea>
        <div class="email-auth-actions">
          <button type="submit" class="btn-primary">Save policy</button>
        </div>
      </form>
      <div>
      <h3 style="margin:0 0 8px;">Policy file</h3>
      <p class="muted">Stored at <code>{policy_path}</code>. Serve it at <code>https://mta-sts.{domain}/.well-known/mta-sts.txt</code> (create an <code>mta-sts.</code> site or CNAME to a host that serves this file).</p>
      <pre class="email-auth-policy">{policy}</pre>
      </div>
      <div>
      <h3 style="margin:0 0 8px;">Recommended DNS</h3>
      <p class="muted">{cf_note}</p>
      {dns_cards}
      <form method="post" action="/email/mta-sts/push-cloudflare" class="email-auth-actions" style="margin-top:14px;">
        <input type="hidden" name="domain" value="{domain}">
        <button type="submit" class="btn-secondary">Push records to Cloudflare</button>
      </form>
      </div>
      </div>"#,
        dns_css = dns_records_styles(),
        notice = flash("ok", notice),
        error = flash("error", error),
        domains = domain_options_from(&sites, &settings.domain),
        domain = html_escape(&settings.domain),
        enabled = enabled,
        mode_opts = mode_opts,
        max_age = settings.max_age,
        mx = html_escape(&mx_joined),
        policy_path = html_escape(&policy_file_path_display(&settings.domain)),
        policy = html_escape(&policy),
        cf_note = html_escape(cf_note),
        dns_cards = dns_records_cards(&dns),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("MTA-STS", None),
        ],
        "MTA-STS",
        "Strict transport security for mail (DNS + policy).",
        &body,
        None,
        None,
    )
}

pub fn email_bimi_page(domain: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let sites = list_sites().unwrap_or_default();
    let settings = if domain.trim().is_empty() {
        let first = sites.first().map(|s| s.domain.clone()).unwrap_or_default();
        load_bimi(&first)
    } else {
        load_bimi(domain)
    };
    let dns = bimi_dns_records(&settings);
    let enabled = if settings.enabled { " checked" } else { "" };
    let cf_note = if cloudflare_configured() {
        "Cloudflare API token is present. Push adds or updates the BIMI TXT only."
    } else {
        "Cloudflare token not configured. Copy the TXT below, or add a token under Cloudflare DNS."
    };
    let body = format!(
        r#"<style>{dns_css}</style>
      <div class="email-auth-layout">
      {notice}{error}
      <p class="muted">BIMI publishes a brand logo for supporting receivers (often Gmail with a VMC). Cloudflare helps as DNS. SnappyMail and Roundcube generally do not display BIMI logos the same way; CPN still prepares DNS so outbound mail is ready.</p>
      <form method="get" action="/email/bimi" class="stack-form">
        <label for="domain">Domain</label>
        <select id="domain" name="domain" onchange="this.form.submit()">{domains}</select>
      </form>
      <form method="post" action="/email/bimi/save" class="stack-form">
        <input type="hidden" name="domain" value="{domain}">
        <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
          <input type="checkbox" name="enabled" value="1"{enabled}> Enable BIMI plan for this domain
        </label>
        <label for="logo_svg_url">Logo SVG URL (https)</label>
        <input id="logo_svg_url" name="logo_svg_url" type="url" value="{logo}" placeholder="https://example.com/brand.svg">
        <label for="authority_url">Authority / VMC URL (optional)</label>
        <input id="authority_url" name="authority_url" type="url" value="{auth}" placeholder="https://... or leave blank">
        <div class="email-auth-actions">
          <button type="submit" class="btn-primary">Save BIMI</button>
        </div>
      </form>
      <div>
      <h3 style="margin:0 0 8px;">Recommended DNS</h3>
      <p class="muted">{cf_note}</p>
      {dns_cards}
      <form method="post" action="/email/bimi/push-cloudflare" class="email-auth-actions" style="margin-top:14px;">
        <input type="hidden" name="domain" value="{domain}">
        <button type="submit" class="btn-secondary">Push records to Cloudflare</button>
      </form>
      </div>
      </div>"#,
        dns_css = dns_records_styles(),
        notice = flash("ok", notice),
        error = flash("error", error),
        domains = domain_options_from(&sites, &settings.domain),
        domain = html_escape(&settings.domain),
        enabled = enabled,
        logo = html_escape(&settings.logo_svg_url),
        auth = html_escape(&settings.authority_url),
        cf_note = html_escape(cf_note),
        dns_cards = dns_records_cards(&dns),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("BIMI", None),
        ],
        "BIMI",
        "Brand Indicators for Message Identification (DNS).",
        &body,
        None,
        None,
    )
}

pub fn save_mta_sts_form(
    domain: &str,
    enabled: bool,
    mode: &str,
    max_age: &str,
    mx: &str,
) -> Result<String, String> {
    let mut settings = MtaStsSettings {
        domain: domain.to_string(),
        mode: mode.to_string(),
        max_age: max_age.parse().unwrap_or(86400),
        mx: mx
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        enabled,
    };
    if settings.mx.is_empty() {
        settings.mx = vec![format!("mail.{}", settings.domain)];
    }
    save_mta_sts(&settings)?;
    Ok("MTA-STS policy saved".into())
}

pub fn push_mta_sts_cloudflare(domain: &str) -> Result<String, String> {
    let settings = load_mta_sts(domain);
    let plans = mta_sts_dns_records(&settings);
    push_dns_plans_to_cloudflare(&settings.domain, &plans)
}

pub fn save_bimi_form(
    domain: &str,
    enabled: bool,
    logo_svg_url: &str,
    authority_url: &str,
) -> Result<String, String> {
    let settings = BimiSettings {
        domain: domain.to_string(),
        logo_svg_url: logo_svg_url.to_string(),
        authority_url: authority_url.to_string(),
        enabled,
    };
    save_bimi(&settings)?;
    Ok("BIMI settings saved".into())
}

pub fn push_bimi_cloudflare(domain: &str) -> Result<String, String> {
    let settings = load_bimi(domain);
    let plans = bimi_dns_records(&settings);
    push_dns_plans_to_cloudflare(&settings.domain, &plans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_records_cards_use_padded_cards_not_crushed_table() {
        let plans = vec![DnsRecordPlan {
            record_type: "TXT".into(),
            name: "_mta-sts.newstargeted.com".into(),
            content: "v=STSv1; id=1790976446".into(),
            ttl: 3600,
            note: "Policy id TXT".into(),
        }];
        let html = dns_records_cards(&plans);
        assert!(html.contains("dns-cards"));
        assert!(html.contains("dns-card"));
        assert!(html.contains("_mta-sts.newstargeted.com"));
        assert!(html.contains("v=STSv1; id=1790976446"));
        assert!(!html.contains("data-table"));
        assert!(!html.contains("<table"));
        let css = dns_records_styles();
        assert!(css.contains("overflow-wrap:anywhere"));
        assert!(css.contains("word-break:break-word"));
        assert!(css.contains("@media (max-width:720px)"));
    }

    #[test]
    fn dns_records_styles_keep_full_width_cards() {
        let css = dns_records_styles();
        assert!(css.contains(".dns-card"));
        assert!(css.contains("min-width:0"));
        assert!(css.contains("width:100%"));
    }
}
