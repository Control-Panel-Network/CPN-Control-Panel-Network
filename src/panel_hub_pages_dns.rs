//! DNS Zones list, create, and record management pages.

use crate::panel_host_info::host_sidebar_info;
use crate::panel_hubs::feature_shell;
use crate::panel_ops_dns::{
    ALLOWED_TYPES, dns_csrf_token, list_zones, load_default_nameservers, load_zone_records,
    read_zone,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn dns_styles() -> &'static str {
    r#"<style>
.dns-hero{display:flex;flex-direction:column;gap:10px;margin-bottom:18px;}
.dns-hero-actions{display:flex;flex-wrap:wrap;gap:10px;align-items:center;}
.dns-feature-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px;margin:16px 0 20px;}
.dns-feature{border:1px solid var(--border,rgba(255,255,255,.08));border-radius:10px;padding:14px 16px;background:rgba(255,255,255,.02);}
.dns-feature strong{display:block;margin-bottom:6px;}
.dns-feature p{margin:0;color:var(--muted,#9aa4b2);font-size:.92rem;line-height:1.45;}
.dns-zone-table{width:100%;border-collapse:collapse;margin-top:8px;}
.dns-zone-table th,.dns-zone-table td{text-align:left;padding:10px 8px;border-bottom:1px solid rgba(255,255,255,.08);vertical-align:middle;}
.dns-zone-table th{color:var(--muted,#9aa4b2);font-weight:600;font-size:.85rem;}
.dns-actions{display:flex;flex-wrap:wrap;gap:8px;align-items:center;}
.dns-create-card{max-width:520px;margin:0 auto;text-align:center;padding:8px 0 4px;}
.dns-create-card label{display:block;text-align:left;margin-bottom:6px;font-size:.8rem;letter-spacing:.04em;text-transform:uppercase;color:var(--muted,#9aa4b2);}
.dns-create-card input[type=text]{width:100%;box-sizing:border-box;padding:12px 14px;border-radius:8px;border:1px solid rgba(255,255,255,.12);background:rgba(0,0,0,.25);color:inherit;font:inherit;}
.dns-hint{margin-top:8px;color:var(--muted,#9aa4b2);font-size:.9rem;}
.dns-record-form{display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:10px;margin:12px 0 18px;}
.dns-record-form label{display:flex;flex-direction:column;gap:4px;font-size:.85rem;}
.dns-record-form input,.dns-record-form select,.dns-record-form textarea{padding:8px 10px;border-radius:8px;border:1px solid rgba(255,255,255,.12);background:rgba(0,0,0,.25);color:inherit;font:inherit;}
.dns-advanced{margin-top:20px;}
.dns-advanced summary{cursor:pointer;font-weight:600;margin-bottom:8px;}
.btn-secondary{display:inline-flex;align-items:center;gap:6px;padding:8px 14px;border-radius:999px;border:1px solid rgba(255,255,255,.14);background:transparent;color:inherit;text-decoration:none;font:inherit;cursor:pointer;}
@media (max-width:900px){.dns-feature-grid{grid-template-columns:1fr;}}
</style>"#
}

fn feature_cards() -> String {
    r#"<div class="dns-feature-grid">
  <div class="dns-feature"><strong>DNS Zone</strong><p>A DNS zone contains all DNS records for a particular domain.</p></div>
  <div class="dns-feature"><strong>Nameservers</strong><p>Default nameservers are assigned automatically when you create a zone.</p></div>
  <div class="dns-feature"><strong>DNS Records</strong><p>After creation, add A, AAAA, CNAME, MX, TXT, NS, and SRV records.</p></div>
</div>"#
    .into()
}

pub fn dns_zones_page(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = dns_csrf_token(username);
    let zones = list_zones().unwrap_or_default();
    let defaults = load_default_nameservers();
    let mut body = String::from(dns_styles());
    body.push_str(r#"<div class="dns-hero"><div class="dns-hero-actions">"#);
    body.push_str(r#"<a class="btn-primary" href="/server/dns/zones/create">Create DNS Zone</a>"#);
    body.push_str(r#"<a class="btn-secondary" href="/server/dns/nameservers">Nameservers</a>"#);
    body.push_str(
        r#"<a class="btn-secondary" href="/server/dns/defaults">Default Nameservers</a></div>"#,
    );
    if defaults.is_empty() {
        body.push_str(
            r#"<p class="dns-hint">Set Default Nameservers before creating zones so SOA and NS records can be seeded.</p>"#,
        );
    } else {
        body.push_str(&format!(
            r#"<p class="dns-hint">New zones use: <code>{}</code></p>"#,
            html_escape(&defaults.join(", "))
        ));
    }
    body.push_str("</div>");
    body.push_str(&feature_cards());
    if zones.is_empty() {
        body.push_str(
            r#"<p class="empty-state">No zones yet. Create a DNS zone to get started.</p>"#,
        );
    } else {
        body.push_str(
            r#"<table class="dns-zone-table"><thead><tr><th>Zone</th><th>Actions</th></tr></thead><tbody>"#,
        );
        for z in &zones {
            let ze = html_escape(z);
            body.push_str(&format!(
                r#"<tr><td><code>{ze}</code></td><td class="dns-actions">
              <a class="btn-secondary" href="/server/dns/zones/manage?name={ze}">Manage</a>
              <form method="post" action="/server/dns/zones/delete" class="inline-form" style="display:inline;">
                <input type="hidden" name="csrf" value="{csrf}">
                <input type="hidden" name="name" value="{ze}">
                <button type="submit" class="btn-danger" onclick="return confirm('Delete zone {ze}?');">Delete</button>
              </form></td></tr>"#,
                csrf = html_escape(&csrf),
            ));
        }
        body.push_str("</tbody></table>");
    }
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("DNS Zones", None),
        ],
        "DNS Zones",
        "Create and manage DNS zones stored under the CPN data directory.",
        &body,
        notice,
        error,
    )
}

pub fn dns_zone_create_page(username: &str, notice: Option<&str>, error: Option<&str>) -> String {
    let csrf = dns_csrf_token(username);
    let host_ip = host_sidebar_info().ip;
    let mut body = String::from(dns_styles());
    body.push_str(&feature_cards());
    body.push_str(&format!(
        r#"<div class="section-subhead" style="margin:8px 0 16px;"><strong>New DNS Zone</strong></div>
        <div class="dns-create-card">
          <form method="post" action="/server/dns/zones/create" class="stack-form">
            <input type="hidden" name="csrf" value="{csrf}">
            <label for="name">Domain name</label>
            <input id="name" name="name" type="text" required placeholder="example.com" autocomplete="off" spellcheck="false">
            <p class="dns-hint">Enter the domain name without http:// or www. Apex A record uses host IP when known ({ip}).</p>
            <button type="submit" class="btn-primary" style="margin-top:14px;">Create DNS Zone</button>
          </form>
          <p style="margin-top:16px;"><a class="btn-secondary" href="/server/dns/zones">Back to zones</a></p>
        </div>"#,
        csrf = html_escape(&csrf),
        ip = html_escape(&host_ip),
    ));
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("DNS Zones", Some("/server/dns/zones")),
            ("Create", None),
        ],
        "Create DNS Zone",
        "Set up a new DNS zone for your domain to manage its DNS records.",
        &body,
        notice,
        error,
    )
}

pub fn dns_zone_manage_page(
    username: &str,
    zone: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    let csrf = dns_csrf_token(username);
    let records = load_zone_records(zone).unwrap_or_default();
    let raw = read_zone(zone).unwrap_or_default();
    let mut type_opts = String::new();
    for t in ALLOWED_TYPES.iter().filter(|t| **t != "SOA") {
        type_opts.push_str(&format!(r#"<option value="{t}">{t}</option>"#));
    }
    let mut rows = String::new();
    for rec in &records {
        let id = html_escape(&rec.id);
        let name = html_escape(&rec.name);
        let rtype = html_escape(&rec.rtype);
        let content = html_escape(&rec.content);
        let prio = rec
            .priority
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".into());
        rows.push_str(&format!(
            r#"<tr>
              <td><code>{name}</code></td>
              <td>{rtype}</td>
              <td>{ttl}</td>
              <td>{prio}</td>
              <td><code>{content}</code></td>
              <td class="dns-actions">
                <form method="post" action="/server/dns/zones/record/delete" style="display:inline;">
                  <input type="hidden" name="csrf" value="{csrf}">
                  <input type="hidden" name="zone" value="{zone}">
                  <input type="hidden" name="id" value="{id}">
                  <button type="submit" class="btn-danger">Delete</button>
                </form>
              </td>
            </tr>"#,
            ttl = rec.ttl,
            csrf = html_escape(&csrf),
            zone = html_escape(zone),
        ));
    }
    if rows.is_empty() {
        rows = r#"<tr><td colspan="6">No records yet.</td></tr>"#.into();
    }
    let body = format!(
        r#"{styles}
        <div class="dns-hero-actions" style="margin-bottom:14px;">
          <a class="btn-secondary" href="/server/dns/zones">All zones</a>
          <a class="btn-secondary" href="/server/dns/zones/create">Create another</a>
        </div>
        <h3 style="margin:0 0 8px;">Records for <code>{zone}</code></h3>
        <form method="post" action="/server/dns/zones/record/add" class="dns-record-form">
          <input type="hidden" name="csrf" value="{csrf}">
          <input type="hidden" name="zone" value="{zone}">
          <label>Name<input name="record_name" type="text" required placeholder="@ or www"></label>
          <label>Type<select name="rtype">{type_opts}</select></label>
          <label>TTL<input name="ttl" type="number" min="60" max="2147483647" value="3600"></label>
          <label>Priority<input name="priority" type="number" min="0" max="65535" placeholder="MX/SRV"></label>
          <label>Weight<input name="weight" type="number" min="0" max="65535" placeholder="SRV"></label>
          <label>Port<input name="port" type="number" min="0" max="65535" placeholder="SRV"></label>
          <label style="grid-column:1/-1;">Content<input name="content" type="text" required placeholder="203.0.113.10 or target host"></label>
          <div style="grid-column:1/-1;"><button type="submit" class="btn-primary">Add record</button></div>
        </form>
        <table class="dns-zone-table">
          <thead><tr><th>Name</th><th>Type</th><th>TTL</th><th>Priority</th><th>Content</th><th></th></tr></thead>
          <tbody>{rows}</tbody>
        </table>
        <details class="dns-advanced">
          <summary>Advanced: raw zone file</summary>
          <form method="post" action="/server/dns/zones/save" class="stack-form">
            <input type="hidden" name="csrf" value="{csrf}">
            <input type="hidden" name="name" value="{zone}">
            <textarea name="content" rows="12" style="width:100%;font-family:ui-monospace,monospace;">{raw}</textarea>
            <button type="submit" class="btn-primary" style="margin-top:10px;">Save raw zone</button>
          </form>
        </details>"#,
        styles = dns_styles(),
        zone = html_escape(zone),
        csrf = html_escape(&csrf),
        type_opts = type_opts,
        rows = rows,
        raw = html_escape(&raw),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Server", Some("/server")),
            ("DNS Zones", Some("/server/dns/zones")),
            (zone, None),
        ],
        &format!("Manage {}", zone),
        "Add, edit, or delete DNS records. Raw zone editing remains available under Advanced.",
        &body,
        notice,
        error,
    )
}
