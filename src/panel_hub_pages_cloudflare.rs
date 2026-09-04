//! Cloudflare DNS panel pages (Manage DNS + API Settings). UX inspired by common
//! hosting panels; CPN branding only (never CyberPanel).

use crate::panel_hubs::feature_shell;
use crate::panel_ops_cloudflare::{RECORD_TYPES, cloudflare_public};
use crate::panel_ops_cloudflare_api::CfDnsRecord;
use crate::sites::list_sites;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn tab_bar(active: &str) -> String {
    let manage = if active == "manage" { " active" } else { "" };
    let api = if active == "api" { " active" } else { "" };
    format!(
        r#"<div class="cf-tabs" role="tablist">
  <a class="cf-tab{manage}" href="/dns/cloudflare?tab=manage" role="tab">Manage DNS</a>
  <a class="cf-tab{api}" href="/dns/cloudflare?tab=api" role="tab">API Settings</a>
</div>
<style>
.cf-tabs {{ display:flex; gap:8px; border-bottom:1px solid var(--border, #333); margin-bottom:16px; }}
.cf-tab {{ padding:10px 14px; text-decoration:none; color:inherit; opacity:0.75; border-bottom:2px solid transparent; }}
.cf-tab.active {{ opacity:1; border-bottom-color: var(--accent, #7c5cff); font-weight:600; }}
.cf-type-row {{ display:flex; flex-wrap:wrap; gap:6px; margin:10px 0; }}
.cf-type-row label {{ display:inline-flex; align-items:center; gap:4px; padding:6px 10px; border-radius:999px; border:1px solid var(--border,#444); cursor:pointer; font-size:13px; }}
.cf-type-row input {{ accent-color: var(--accent, #7c5cff); }}
.cf-proxy {{ position:relative; width:42px; height:24px; display:inline-block; }}
.cf-proxy input {{ opacity:0; width:0; height:0; }}
.cf-proxy span {{ position:absolute; inset:0; background:#444; border-radius:999px; transition:.15s; }}
.cf-proxy span:before {{ content:""; position:absolute; width:18px; height:18px; left:3px; top:3px; background:#fff; border-radius:50%; transition:.15s; }}
.cf-proxy input:checked + span {{ background: var(--accent, #7c5cff); }}
.cf-proxy input:checked + span:before {{ transform: translateX(18px); }}
.cf-table {{ width:100%; border-collapse:collapse; font-size:13px; }}
.cf-table th, .cf-table td {{ text-align:left; padding:8px 6px; border-bottom:1px solid var(--border,#333); vertical-align:middle; }}
.cf-add-row {{ display:flex; flex-wrap:wrap; gap:8px; align-items:end; margin:12px 0; }}
.cf-add-row label {{ display:flex; flex-direction:column; gap:4px; font-size:12px; }}
.cf-add-row input, .cf-add-row select {{ min-width:120px; padding:8px; border-radius:6px; border:1px solid var(--border,#444); background:transparent; color:inherit; }}
</style>"#
    )
}

fn domain_options(selected: &str) -> String {
    let mut out = String::from(r#"<option value="">Choose a domain...</option>"#);
    let mut domains: Vec<String> = list_sites()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.domain)
        .collect();
    domains.sort();
    domains.dedup();
    for d in domains {
        let sel = if d == selected { " selected" } else { "" };
        out.push_str(&format!(
            r#"<option value="{v}"{sel}>{l}</option>"#,
            v = html_escape(&d),
            l = html_escape(&d),
            sel = sel,
        ));
    }
    out
}

fn records_table(domain: &str, records: &[CfDnsRecord]) -> String {
    if domain.is_empty() {
        return r#"<p class="muted">Select a domain to load Cloudflare DNS records.</p>"#.into();
    }
    if records.is_empty() {
        return format!(
            r#"<p class="muted">No DNS records returned for <strong>{}</strong>.</p>"#,
            html_escape(domain)
        );
    }
    let mut rows = String::new();
    for r in records {
        let ttl = if r.ttl == 1 {
            "AUTO".to_string()
        } else {
            r.ttl.to_string()
        };
        let pri = r
            .priority
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        let proxy_disabled = !matches!(r.record_type.as_str(), "A" | "AAAA" | "CNAME");
        let checked = if r.proxied { " checked" } else { "" };
        let disabled = if proxy_disabled { " disabled" } else { "" };
        rows.push_str(&format!(
            r#"<tr>
  <td><code>{name}</code></td>
  <td>{ty}</td>
  <td>{ttl}</td>
  <td><code>{val}</code></td>
  <td>{pri}</td>
  <td>
    <form method="post" action="/dns/cloudflare/proxy" class="inline-form">
      <input type="hidden" name="domain" value="{dom}">
      <input type="hidden" name="record_id" value="{id}">
      <input type="hidden" name="proxied" value="{next}">
      <label class="cf-proxy" title="Cloudflare proxy (orange-cloud style)">
        <input type="checkbox" onchange="this.form.submit()"{checked}{disabled}>
        <span></span>
      </label>
    </form>
  </td>
  <td>
    <form method="post" action="/dns/cloudflare/delete" onsubmit="return confirm('Delete this DNS record?');">
      <input type="hidden" name="domain" value="{dom}">
      <input type="hidden" name="record_id" value="{id}">
      <button type="submit" class="btn-danger" aria-label="Delete record">Delete</button>
    </form>
  </td>
</tr>"#,
            name = html_escape(&r.name),
            ty = html_escape(&r.record_type),
            ttl = html_escape(&ttl),
            val = html_escape(&r.content),
            pri = html_escape(&pri),
            dom = html_escape(domain),
            id = html_escape(&r.id),
            next = if r.proxied { "0" } else { "1" },
            checked = checked,
            disabled = disabled,
        ));
    }
    format!(
        r#"<h3>DNS Records</h3>
<table class="cf-table">
  <thead><tr><th>NAME</th><th>TYPE</th><th>TTL</th><th>VALUE</th><th>PRIORITY</th><th>PROXY</th><th>ACTIONS</th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#
    )
}

fn type_buttons(selected: &str) -> String {
    let mut out =
        String::from(r#"<div class="cf-type-row" role="group" aria-label="Record type">"#);
    for t in RECORD_TYPES {
        let checked = if *t == selected { " checked" } else { "" };
        out.push_str(&format!(
            r#"<label><input type="radio" name="record_type" value="{t}"{checked}> {t}</label>"#,
        ));
    }
    out.push_str("</div>");
    out
}

fn manage_body(
    domain: &str,
    records: Result<Vec<CfDnsRecord>, String>,
    load_error: Option<&str>,
) -> String {
    let rec_html = match &records {
        Ok(r) => records_table(domain, r),
        Err(e) => format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(e)
        ),
    };
    let err = load_error
        .map(|e| {
            format!(
                r#"<p class="panel-notice error" role="status">{}</p>"#,
                html_escape(e)
            )
        })
        .unwrap_or_default();
    format!(
        r#"{tabs}
<div class="cf-manage">
  <form method="get" action="/dns/cloudflare" class="cf-add-row">
    <input type="hidden" name="tab" value="manage">
    <label>Select Domain
      <select name="domain" onchange="this.form.submit()">{opts}</select>
    </label>
  </form>
  <form method="post" action="/dns/cloudflare/sync" style="display:inline-block;margin:8px 0;">
    <input type="hidden" name="domain" value="{dom}">
    <button type="submit" class="btn-primary" {sync_dis}>Sync to Cloudflare</button>
  </form>
  <h3>Add DNS Record</h3>
  <form method="post" action="/dns/cloudflare/add">
    <input type="hidden" name="domain" value="{dom}">
    {types}
    <div class="cf-add-row">
      <label>Name <input name="name" placeholder="@" required></label>
      <label>TTL <input name="ttl" type="number" value="3600" min="1"></label>
      <label>Value <input name="content" placeholder="192.168.1.1" required></label>
      <label>Priority <input name="priority" type="number" placeholder="10"></label>
      <label>Proxy <select name="proxied"><option value="0">Off</option><option value="1">On</option></select></label>
      <button type="submit" class="btn-primary" {add_dis}>+ Add Record</button>
    </div>
  </form>
  {err}
  {rec}
</div>
<p class="muted">Uses the Cloudflare API with token auth when configured. Local zone files under the CPN DNS data directory sync when enabled in API Settings.</p>"#,
        tabs = tab_bar("manage"),
        opts = domain_options(domain),
        dom = html_escape(domain),
        types = type_buttons("A"),
        err = err,
        rec = rec_html,
        sync_dis = if domain.is_empty() { "disabled" } else { "" },
        add_dis = if domain.is_empty() { "disabled" } else { "" },
    )
}

fn api_body() -> String {
    let pubv = cloudflare_public();
    let sync_en = if pubv.sync_local { " selected" } else { "" };
    let sync_dis = if pubv.sync_local { "" } else { " selected" };
    let tok_sel = if pubv.auth_type == "global_key" {
        ""
    } else {
        " selected"
    };
    let key_sel = if pubv.auth_type == "global_key" {
        " selected"
    } else {
        ""
    };
    let configured = if pubv.configured {
        format!(
            r#"<p class="muted">Token on disk: <code>{}</code> (masked). Leave the token field blank to keep the current secret.</p>"#,
            html_escape(&pubv.token_masked)
        )
    } else {
        r#"<p class="muted">No Cloudflare API token stored yet. Create a token with Zone DNS Edit permissions.</p>"#.into()
    };
    format!(
        r#"{tabs}
<h3>Cloudflare API Configuration</h3>
{configured}
<form method="post" action="/dns/cloudflare/settings" class="stack-form" style="max-width:520px;">
  <label for="auth_type">Authentication type</label>
  <select id="auth_type" name="auth_type">
    <option value="api_token"{tok_sel}>API Token (recommended)</option>
    <option value="global_key"{key_sel}>Global API Key (email + key)</option>
  </select>
  <label for="email">Cloudflare Email</label>
  <input id="email" name="email" type="email" placeholder="your@email.com" value="{email}">
  <p class="muted">Optional when using an API Token. Required for Global API Key.</p>
  <label for="api_token">API Token</label>
  <input id="api_token" name="api_token" type="password" autocomplete="new-password" placeholder="Enter your Cloudflare API token">
  <label for="sync_local">Sync Local Records to Cloudflare</label>
  <select id="sync_local" name="sync_local">
    <option value="1"{sync_en}>Enable</option>
    <option value="0"{sync_dis}>Disable</option>
  </select>
  <button type="submit" class="btn-primary">Save Configuration</button>
</form>
<p class="muted">Stored at <code>/var/lib/cpn/cloudflare.json</code> (mode 600). Tokens are never logged or shown in full.</p>"#,
        tabs = tab_bar("api"),
        configured = configured,
        email = html_escape(&pubv.email),
        tok_sel = tok_sel,
        key_sel = key_sel,
        sync_en = sync_en,
        sync_dis = sync_dis,
    )
}

pub fn cloudflare_dns_page(
    tab: &str,
    domain: &str,
    records: Result<Vec<CfDnsRecord>, String>,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let body = if tab == "api" {
        api_body()
    } else {
        manage_body(domain, records, None)
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("Cloudflare DNS", None),
        ],
        "Cloudflare DNS",
        "Manage DNS records for your domains through Cloudflare integration.",
        &body,
        notice,
        error,
    )
}
