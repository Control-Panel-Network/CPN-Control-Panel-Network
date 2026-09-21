//! Cloudflare DNS panel pages (Manage DNS + API Settings). UX inspired by common
//! hosting panels; CPN branding only (never CyberPanel).

use crate::panel_hub_pages_cloudflare_api::api_settings_body;
use crate::panel_hub_pages_cloudflare_pager::CfTableOpts;
use crate::panel_hub_pages_cloudflare_table::records_table;
use crate::panel_hubs::feature_shell;
use crate::panel_ops_cloudflare::{RECORD_TYPES, cloudflare_public};
use crate::panel_ops_cloudflare_api::CfDnsRecord;
use crate::panel_ops_cloudflare_oauth::oauth_public;
use crate::panel_ops_cloudflare_verify::list_accessible_zones;
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
.cf-tab.active {{ opacity:1; border-bottom-color: var(--accent, #3b82f6); font-weight:600; }}
.cf-type-row {{ display:flex; flex-wrap:wrap; gap:6px; margin:10px 0; align-items:center; }}
.cf-type-chip {{ appearance:none; padding:6px 12px; border-radius:999px; border:1px solid var(--border,#444); background:transparent; color:inherit; cursor:pointer; font-size:13px; }}
.cf-type-chip.active {{ border-color: var(--accent,#3b82f6); background:rgba(59,130,246,.18); color:var(--ink,#fff); font-weight:600; }}
.cf-proxy {{ position:relative; width:42px; height:24px; display:inline-block; }}
.cf-proxy input {{ opacity:0; width:0; height:0; }}
.cf-proxy span {{ position:absolute; inset:0; background:#444; border-radius:999px; transition:.15s; }}
.cf-proxy span:before {{ content:""; position:absolute; width:18px; height:18px; left:3px; top:3px; background:#fff; border-radius:50%; transition:.15s; }}
.cf-proxy input:checked + span {{ background: var(--accent, #3b82f6); }}
.cf-proxy input:checked + span:before {{ transform: translateX(18px); }}
.cf-table {{ width:100%; border-collapse:collapse; font-size:13px; }}
.cf-table th, .cf-table td {{ text-align:left; padding:8px 6px; border-bottom:1px solid var(--border,#333); vertical-align:middle; }}
.cf-add-row {{ display:flex; flex-wrap:wrap; gap:8px; align-items:end; margin:12px 0; }}
.cf-add-row label {{ display:flex; flex-direction:column; gap:4px; font-size:12px; }}
.cf-add-row input, .cf-add-row select, .cf-edit-input {{ min-width:100px; padding:8px; border-radius:6px; border:1px solid var(--border,#444); background:var(--canvas,#fff); color:var(--ink,#1d1d1f); color-scheme:light; }}
.cf-edit-input {{ width:100%; box-sizing:border-box; }}
.cf-actions {{ display:flex; flex-wrap:wrap; gap:6px; align-items:center; }}
.cf-row-edit {{ display:none; }}
.cf-row-edit.is-open {{ display:table-row; }}
.cf-row-view.is-editing {{ display:none; }}
.stack-form select, .stack-form input {{ background:var(--canvas,#fff); color:var(--ink,#1d1d1f); }}
[data-color-mode="dark"] .cf-add-row input,
[data-color-mode="dark"] .cf-add-row select,
[data-color-mode="dark"] .cf-edit-input,
[data-color-mode="dark"] .stack-form select,
[data-color-mode="dark"] .stack-form input {{
  background:#12151c; border-color:#3b4558; color:#f3f6fb; color-scheme:dark;
}}
[data-color-mode="dark"] .cf-type-chip {{ color:#e8edf7; border-color:#3b4558; }}
[data-color-mode="dark"] .cf-type-chip.active {{ background:rgba(59,130,246,.28); color:#f3f6fb; }}
[data-color-mode="dark"] .cf-add-row select option,
[data-color-mode="dark"] .stack-form select option {{
  background:#12151c; color:#f3f6fb;
}}
</style>"#
    )
}

/// Domains for Manage DNS: local CPN websites plus Cloudflare zones (OAuth/token).
#[derive(Debug, Default)]
struct DomainChoices {
    local: Vec<String>,
    cloudflare: Vec<String>,
    zone_error: Option<String>,
}

fn collect_domain_choices() -> DomainChoices {
    let mut local: Vec<String> = list_sites()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.domain.trim().to_ascii_lowercase())
        .filter(|d| !d.is_empty())
        .collect();
    local.sort();
    local.dedup();

    // Prefer a fresh OAuth access token before zone list (no-op for API token auth).
    let _ = crate::panel_ops_cloudflare_oauth::refresh_oauth_access_if_needed();

    let (cloudflare, zone_error) = match list_accessible_zones(100) {
        Ok(mut zones) => {
            zones.sort();
            zones.dedup();
            (zones, None)
        }
        Err(err) => (Vec::new(), Some(err)),
    };

    DomainChoices {
        local,
        cloudflare,
        zone_error,
    }
}

/// First domain to open when Manage DNS has no `domain` query (local or Cloudflare).
pub fn preferred_manage_domain() -> Option<String> {
    let choices = collect_domain_choices();
    choices.cloudflare.into_iter().chain(choices.local).next()
}

fn domain_options(selected: &str, choices: &DomainChoices) -> String {
    let mut out = String::from(r#"<option value="">Choose a Cloudflare zone...</option>"#);

    if !choices.local.is_empty() {
        out.push_str(r#"<optgroup label="Local websites">"#);
        for d in &choices.local {
            let sel = if d == selected { " selected" } else { "" };
            out.push_str(&format!(
                r#"<option value="{v}"{sel}>{l}</option>"#,
                v = html_escape(d),
                l = html_escape(d),
                sel = sel,
            ));
        }
        out.push_str("</optgroup>");
    }

    if !choices.cloudflare.is_empty() {
        out.push_str(r#"<optgroup label="Cloudflare zones">"#);
        for d in &choices.cloudflare {
            let sel = if d == selected { " selected" } else { "" };
            out.push_str(&format!(
                r#"<option value="{v}"{sel}>{l}</option>"#,
                v = html_escape(d),
                l = html_escape(d),
                sel = sel,
            ));
        }
        out.push_str("</optgroup>");
    }

    out
}

fn manage_empty_hint(choices: &DomainChoices) -> String {
    let oauth = oauth_public(0);
    let pubv = cloudflare_public();
    let linked = oauth.linked || pubv.oauth_linked || pubv.auth_type == "oauth";
    let configured = pubv.configured || linked;

    if let Some(err) = choices.zone_error.as_deref() {
        return format!(
            "Could not list Cloudflare zones ({err}). Open API Settings, confirm OAuth or token access, then reload Manage DNS."
        );
    }
    if !choices.cloudflare.is_empty() || !choices.local.is_empty() {
        return "Pick a Cloudflare zone (or local website) above to load DNS records.".into();
    }
    if linked {
        return "Cloudflare OAuth is linked, but no zones were returned and this host has no local websites yet. Confirm the linked account can see zones in Cloudflare, or create a website in CPN.".into();
    }
    if configured {
        return "Cloudflare credentials are saved, but no zones were returned and this host has no local websites yet. Use Test connection under API Settings, or create a website in CPN.".into();
    }
    "Connect Cloudflare under API Settings (OAuth or API token), or create a website first. Manage DNS lists Cloudflare zones even when no local website exists yet.".into()
}

fn add_type_options(selected: &str) -> String {
    let sel = if selected.is_empty() || selected.eq_ignore_ascii_case("all") {
        "A"
    } else {
        selected
    };
    let mut out = String::new();
    for t in RECORD_TYPES {
        let s = if *t == sel { " selected" } else { "" };
        out.push_str(&format!(r#"<option value="{t}"{s}>{t}</option>"#));
    }
    out
}

fn manage_body(
    domain: &str,
    records: Result<Vec<CfDnsRecord>, String>,
    table_opts: &CfTableOpts,
    load_error: Option<&str>,
) -> String {
    let filter_type = table_opts.filter_type.as_str();
    let choices = collect_domain_choices();
    let empty_hint = manage_empty_hint(&choices);
    let list_hiddens = format!(
        r#"<input type="hidden" name="page" value="{page}">
<input type="hidden" name="per_page" value="{per_page}">
<input type="hidden" name="mode" value="{mode}">
<input type="hidden" name="sort" value="{sort}">
<input type="hidden" name="order" value="{order}">"#,
        page = table_opts.page.max(1),
        per_page = table_opts.per_page,
        mode = html_escape(&table_opts.mode),
        sort = html_escape(&table_opts.sort),
        order = html_escape(&table_opts.order),
    );
    let rec_html = if domain.trim().is_empty() {
        format!(
            r#"<p class="muted" role="status">{}</p>"#,
            html_escape(&empty_hint)
        )
    } else {
        match &records {
            Ok(r) => records_table(domain, r, table_opts),
            Err(e) => format!(
                r#"<p class="panel-notice error" role="status">{}</p>"#,
                html_escape(e)
            ),
        }
    };
    let mut err = String::new();
    if let Some(zone_err) = choices.zone_error.as_deref() {
        err.push_str(&format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(&format!("Cloudflare zone list failed: {zone_err}"))
        ));
    }
    if let Some(e) = load_error {
        err.push_str(&format!(
            r#"<p class="panel-notice error" role="status">{}</p>"#,
            html_escape(e)
        ));
    }
    let add_type = if filter_type.is_empty() || filter_type.eq_ignore_ascii_case("all") {
        "A"
    } else {
        filter_type
    };
    let zone_count = choices.cloudflare.len();
    let zone_note = if zone_count > 0 {
        format!(
            r#"<p class="muted">{n} Cloudflare zone(s) available from the linked account (local websites not required).</p>"#,
            n = zone_count
        )
    } else {
        String::new()
    };
    format!(
        r#"{tabs}
<div class="cf-manage">
  <form method="get" action="/dns/cloudflare" class="cf-add-row">
    <input type="hidden" name="tab" value="manage">
    <label>Select zone
      <select name="domain" onchange="this.form.submit()">{opts}</select>
    </label>
  </form>
  {zone_note}
  <form method="post" action="/dns/cloudflare/sync" style="display:inline-block;margin:8px 0;">
    <input type="hidden" name="domain" value="{dom}">
    <input type="hidden" name="filter_type" value="{ft}">
    {list_hiddens}
    <button type="submit" class="btn-primary" {sync_dis}>Sync to Cloudflare</button>
  </form>
  <h3>Add DNS Record</h3>
  <form method="post" action="/dns/cloudflare/add" id="cf-add-form">
    <input type="hidden" name="domain" value="{dom}">
    <input type="hidden" name="filter_type" value="{ft}">
    {list_hiddens}
    <div class="cf-add-row">
      <label>Type <select id="cf-add-type" name="record_type">{type_opts}</select></label>
      <label>Name <input name="name" placeholder="@" required></label>
      <label>TTL <input name="ttl" type="number" value="3600" min="1"></label>
      <label>Value <input name="content" placeholder="192.0.2.1 or 2001:db8::1" required></label>
      <label>Priority <input name="priority" type="number" placeholder="10"></label>
      <label>Proxy <select name="proxied"><option value="0">Off</option><option value="1">On</option></select></label>
      <button type="submit" class="btn-primary" {add_dis}>+ Add Record</button>
    </div>
  </form>
  <p class="muted">Type chips below filter the records table. Choosing a type also sets the Add form type. AAAA values must be IPv6.</p>
  {err}
  {rec}
</div>
<p class="muted">Uses the Cloudflare API (OAuth or API token). Local zone files under the CPN DNS data directory sync when enabled in API Settings.</p>"#,
        tabs = tab_bar("manage"),
        opts = domain_options(domain, &choices),
        zone_note = zone_note,
        dom = html_escape(domain),
        ft = html_escape(filter_type),
        list_hiddens = list_hiddens,
        type_opts = add_type_options(add_type),
        err = err,
        rec = rec_html,
        sync_dis = if domain.is_empty() { "disabled" } else { "" },
        add_dis = if domain.is_empty() { "disabled" } else { "" },
    )
}

pub fn cloudflare_dns_page(
    tab: &str,
    domain: &str,
    records: Result<Vec<CfDnsRecord>, String>,
    table_opts: &CfTableOpts,
    listen_port: u16,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let body = if tab == "api" {
        api_settings_body(listen_port, &tab_bar("api"))
    } else {
        manage_body(domain, records, table_opts, None)
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
