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
    let sites = list_sites().unwrap_or_default();
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

fn dns_table(records: &[DnsRecordPlan]) -> String {
    let mut rows = String::new();
    for r in records {
        rows.push_str(&format!(
            r#"<tr>
          <td><code>{ty}</code></td>
          <td><code>{name}</code></td>
          <td><code>{content}</code></td>
          <td class="muted">{note}</td>
        </tr>"#,
            ty = html_escape(&r.record_type),
            name = html_escape(&r.name),
            content = html_escape(&r.content),
            note = html_escape(&r.note),
        ));
    }
    format!(
        r#"<table class="data-table" style="width:100%;margin-top:12px;">
      <thead><tr><th>Type</th><th>Name</th><th>Value</th><th>Notes</th></tr></thead>
      <tbody>{rows}</tbody>
    </table>"#
    )
}

pub fn email_mta_sts_page(domain: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let settings = if domain.trim().is_empty() {
        let first = list_sites()
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(|s| s.domain)
            .unwrap_or_default();
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
        r#"{notice}{error}
      <p class="muted">MTA-STS tells receiving MTAs how to expect TLS for your domain. Cloudflare supports this mainly as DNS (TXT + policy host). Client webmail (SnappyMail/Roundcube) does not enforce MTA-STS; receivers do.</p>
      <form method="get" action="/email/mta-sts" class="stack-form" style="max-width:520px;">
        <label for="domain">Domain</label>
        <select id="domain" name="domain" onchange="this.form.submit()">{domains}</select>
      </form>
      <form method="post" action="/email/mta-sts/save" class="stack-form" style="max-width:520px;margin-top:12px;">
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
        <button type="submit" class="btn-primary">Save policy</button>
      </form>
      <h3 style="margin-top:20px;">Policy file</h3>
      <p class="muted">Stored at <code>{policy_path}</code>. Serve it at <code>https://mta-sts.{domain}/.well-known/mta-sts.txt</code> (create an <code>mta-sts.</code> site or CNAME to a host that serves this file).</p>
      <pre style="white-space:pre-wrap;background:rgba(0,0,0,.25);padding:12px;border-radius:8px;">{policy}</pre>
      <h3>Recommended DNS</h3>
      <p class="muted">{cf_note}</p>
      {dns_table}
      <form method="post" action="/email/mta-sts/push-cloudflare" style="margin-top:12px;">
        <input type="hidden" name="domain" value="{domain}">
        <button type="submit" class="btn-secondary">Push records to Cloudflare</button>
      </form>"#,
        notice = flash("ok", notice),
        error = flash("error", error),
        domains = domain_options(&settings.domain),
        domain = html_escape(&settings.domain),
        enabled = enabled,
        mode_opts = mode_opts,
        max_age = settings.max_age,
        mx = html_escape(&mx_joined),
        policy_path = html_escape(&policy_file_path_display(&settings.domain)),
        policy = html_escape(&policy),
        cf_note = html_escape(cf_note),
        dns_table = dns_table(&dns),
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
    let settings = if domain.trim().is_empty() {
        let first = list_sites()
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(|s| s.domain)
            .unwrap_or_default();
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
        r#"{notice}{error}
      <p class="muted">BIMI publishes a brand logo for supporting receivers (often Gmail with a VMC). Cloudflare helps as DNS. SnappyMail and Roundcube generally do not display BIMI logos the same way; CPN still prepares DNS so outbound mail is ready.</p>
      <form method="get" action="/email/bimi" class="stack-form" style="max-width:520px;">
        <label for="domain">Domain</label>
        <select id="domain" name="domain" onchange="this.form.submit()">{domains}</select>
      </form>
      <form method="post" action="/email/bimi/save" class="stack-form" style="max-width:520px;margin-top:12px;">
        <input type="hidden" name="domain" value="{domain}">
        <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
          <input type="checkbox" name="enabled" value="1"{enabled}> Enable BIMI plan for this domain
        </label>
        <label for="logo_svg_url">Logo SVG URL (https)</label>
        <input id="logo_svg_url" name="logo_svg_url" type="url" value="{logo}" placeholder="https://example.com/brand.svg">
        <label for="authority_url">Authority / VMC URL (optional)</label>
        <input id="authority_url" name="authority_url" type="url" value="{auth}" placeholder="https://... or leave blank">
        <button type="submit" class="btn-primary">Save BIMI</button>
      </form>
      <h3 style="margin-top:20px;">Recommended DNS</h3>
      <p class="muted">{cf_note}</p>
      {dns_table}
      <form method="post" action="/email/bimi/push-cloudflare" style="margin-top:12px;">
        <input type="hidden" name="domain" value="{domain}">
        <button type="submit" class="btn-secondary">Push records to Cloudflare</button>
      </form>"#,
        notice = flash("ok", notice),
        error = flash("error", error),
        domains = domain_options(&settings.domain),
        domain = html_escape(&settings.domain),
        enabled = enabled,
        logo = html_escape(&settings.logo_svg_url),
        auth = html_escape(&settings.authority_url),
        cf_note = html_escape(cf_note),
        dns_table = dns_table(&dns),
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
