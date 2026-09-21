//! Cloudflare Manage DNS records table (type chips + pagination + inline edit).

use crate::panel_hub_pages_cloudflare_pager::{
    CfTableOpts, dns_list_toolbar, dns_mode_from_query, list_state_hiddens, manage_list_url,
};
use crate::panel_ops_cloudflare::{RECORD_TYPES, record_type_uses_priority};
use crate::panel_ops_cloudflare_api::CfDnsRecord;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn type_filter_chips(domain: &str, opts: &CfTableOpts) -> String {
    let mut out = String::from(
        r#"<div class="cf-type-row" role="group" aria-label="Filter DNS records by type">
<span class="muted" style="margin-right:4px;">Filter:</span>"#,
    );
    let selected = opts.filter_type.as_str();
    let all_active = if selected.is_empty() || selected.eq_ignore_ascii_case("all") {
        " active"
    } else {
        ""
    };
    let all_href = manage_list_url(domain, "ALL", &opts.mode, opts.per_page, 1);
    out.push_str(&format!(
        r#"<a class="cf-type-chip{all_active}" href="{href}">All</a>"#,
        all_active = all_active,
        href = html_escape(&all_href),
    ));
    for t in RECORD_TYPES {
        let active = if selected.eq_ignore_ascii_case(t) {
            " active"
        } else {
            ""
        };
        let href = manage_list_url(domain, t, &opts.mode, opts.per_page, 1);
        out.push_str(&format!(
            r#"<a class="cf-type-chip{active}" href="{href}">{t}</a>"#,
            active = active,
            href = html_escape(&href),
            t = t,
        ));
    }
    out.push_str("</div>");
    out
}

fn render_record_rows(domain: &str, records: &[&CfDnsRecord], opts: &CfTableOpts) -> String {
    let mut rows = String::new();
    let state = list_state_hiddens(opts);
    for r in records {
        let ttl = if r.ttl == 1 {
            "AUTO".to_string()
        } else {
            r.ttl.to_string()
        };
        let ttl_num = if r.ttl == 0 { 1 } else { r.ttl };
        let pri = r
            .priority
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        let pri_val = r.priority.map(|p| p.to_string()).unwrap_or_default();
        let proxy_ok = matches!(r.record_type.as_str(), "A" | "AAAA" | "CNAME");
        let checked = if r.proxied { " checked" } else { "" };
        let disabled = if proxy_ok { "" } else { " disabled" };
        let proxied_sel_on = if r.proxied { " selected" } else { "" };
        let proxied_sel_off = if r.proxied { "" } else { " selected" };
        let proxy_edit = if proxy_ok {
            format!(
                r#"<select name="proxied" class="cf-edit-input"><option value="0"{off}>Off</option><option value="1"{on}>On</option></select>"#,
                off = proxied_sel_off,
                on = proxied_sel_on,
            )
        } else {
            r#"<input type="hidden" name="proxied" value="0"><span class="muted">n/a</span>"#.into()
        };
        // Match Add form: always show Priority. MX/SRV are editable and saved;
        // other types stay empty/disabled (Cloudflare ignores priority for them).
        let pri_edit = if record_type_uses_priority(&r.record_type) {
            let v = if pri_val.is_empty() {
                "10".to_string()
            } else {
                pri_val.clone()
            };
            format!(
                r#"<input class="cf-edit-input" name="priority" type="number" value="{v}" min="0" max="65535" required>"#,
                v = html_escape(&v),
            )
        } else {
            r#"<input class="cf-edit-input" name="priority" type="number" value="" min="0" max="65535" placeholder="10" disabled title="Priority applies to MX and SRV records"><span class="muted" style="margin-left:4px;">(MX/SRV)</span>"#.into()
        };
        rows.push_str(&format!(
            r#"<tr class="cf-row-view" data-rtype="{ty}" data-rid="{id}">
  <td><code>{name}</code></td>
  <td>{ty}</td>
  <td>{ttl}</td>
  <td><code class="cf-val">{val}</code></td>
  <td>{pri}</td>
  <td>
    <form method="post" action="/dns/cloudflare/proxy" class="inline-form">
      <input type="hidden" name="domain" value="{dom}">
      <input type="hidden" name="record_id" value="{id}">
      <input type="hidden" name="proxied" value="{next}">
      {state}
      <label class="cf-proxy" title="Cloudflare proxy">
        <input type="checkbox" onchange="this.form.submit()"{checked}{disabled}>
        <span></span>
      </label>
    </form>
  </td>
  <td class="cf-actions">
    <button type="button" class="btn-secondary" onclick="cfStartEdit('{id}')">Edit</button>
    <form method="post" action="/dns/cloudflare/delete" onsubmit="return confirm('Delete this DNS record?');" style="display:inline;">
      <input type="hidden" name="domain" value="{dom}">
      <input type="hidden" name="record_id" value="{id}">
      {state}
      <button type="submit" class="btn-danger" aria-label="Delete record">Delete</button>
    </form>
  </td>
</tr>
<tr class="cf-row-edit" data-rtype="{ty}" data-rid="{id}" style="display:none">
  <td colspan="7">
    <form method="post" action="/dns/cloudflare/update" class="cf-add-row" style="margin:0;">
      <input type="hidden" name="domain" value="{dom}">
      <input type="hidden" name="record_id" value="{id}">
      {state}
      <label>Name <input class="cf-edit-input" name="name" value="{name}" required></label>
      <label>TTL <input class="cf-edit-input" name="ttl" type="number" value="{ttl_num}" min="1"></label>
      <label>Value <input class="cf-edit-input" name="content" value="{val}" required></label>
      <label>Priority {pri_edit}</label>
      <label>Proxy {proxy_edit}</label>
      <div class="cf-actions" style="padding-bottom:2px;">
        <button type="submit" class="btn-primary">Save</button>
        <button type="button" class="btn-secondary" onclick="cfCancelEdit('{id}')">Cancel</button>
      </div>
    </form>
  </td>
</tr>"#,
            name = html_escape(&r.name),
            ty = html_escape(&r.record_type),
            ttl = html_escape(&ttl),
            ttl_num = ttl_num,
            val = html_escape(&r.content),
            pri = html_escape(&pri),
            pri_edit = pri_edit,
            proxy_edit = proxy_edit,
            dom = html_escape(domain),
            id = html_escape(&r.id),
            next = if r.proxied { "0" } else { "1" },
            checked = checked,
            disabled = disabled,
            state = state,
        ));
    }
    rows
}

pub(crate) fn records_table(domain: &str, records: &[CfDnsRecord], opts: &CfTableOpts) -> String {
    if domain.is_empty() {
        return r#"<p class="muted">Pick a Cloudflare zone above to load DNS records (OAuth zones appear even without a local website).</p>"#.into();
    }
    if records.is_empty() {
        return format!(
            r#"<p class="muted">No DNS records returned for <strong>{}</strong>.</p>"#,
            html_escape(domain)
        );
    }

    let filter = opts.filter_type.trim();
    let filtered: Vec<&CfDnsRecord> = if filter.is_empty() || filter.eq_ignore_ascii_case("all") {
        records.iter().collect()
    } else {
        records
            .iter()
            .filter(|r| r.record_type.eq_ignore_ascii_case(filter))
            .collect()
    };
    let total_all = records.len();
    let filtered_count = filtered.len();
    let mode = dns_mode_from_query(&opts.mode);
    let per_page = if mode == "scroll" {
        filtered_count.max(1)
    } else {
        opts.per_page.max(1)
    };
    let total_pages = if mode == "scroll" {
        1
    } else {
        filtered_count.div_ceil(per_page).max(1)
    };
    let page = opts.page.clamp(1, total_pages);
    let start = if mode == "scroll" {
        0
    } else {
        (page - 1) * per_page
    };
    let end = if mode == "scroll" {
        filtered_count
    } else {
        (start + per_page).min(filtered_count)
    };
    let page_slice = &filtered[start..end];

    let toolbar = dns_list_toolbar(domain, opts, page, total_pages, filtered_count, total_all);
    let scroll_cls = if mode == "scroll" {
        "cf-table-scroll is-scroll"
    } else {
        "cf-table-scroll"
    };
    let add_type = if filter.is_empty() || filter.eq_ignore_ascii_case("all") {
        "A"
    } else {
        filter
    };
    let add_type_js = serde_json::to_string(add_type).unwrap_or_else(|_| "\"A\"".into());

    format!(
        r#"<h3>DNS Records</h3>
{chips}
{toolbar}
<div class="{scroll_cls}">
<table class="cf-table" id="cf-records-table">
  <thead><tr><th>NAME</th><th>TYPE</th><th>TTL</th><th>VALUE</th><th>PRIORITY</th><th>PROXY</th><th>ACTIONS</th></tr></thead>
  <tbody>{rows}</tbody>
</table>
</div>
<script>
(function(){{
  var addType = {add_type_js};
  var sel = document.getElementById('cf-add-type');
  if (sel && addType) sel.value = addType;
  window.cfStartEdit = function(id) {{
    document.querySelectorAll('.cf-row-view').forEach(function(tr) {{
      tr.classList.toggle('is-editing', tr.getAttribute('data-rid') === id);
    }});
    document.querySelectorAll('.cf-row-edit').forEach(function(tr) {{
      var open = tr.getAttribute('data-rid') === id;
      tr.classList.toggle('is-open', open);
      tr.style.display = open ? 'table-row' : 'none';
    }});
  }};
  window.cfCancelEdit = function(id) {{
    document.querySelectorAll('.cf-row-view[data-rid=\"'+id+'\"]').forEach(function(tr) {{
      tr.classList.remove('is-editing');
    }});
    document.querySelectorAll('.cf-row-edit[data-rid=\"'+id+'\"]').forEach(function(tr) {{
      tr.classList.remove('is-open');
      tr.style.display = 'none';
    }});
  }};
  var addForm = document.getElementById('cf-add-form');
  if (addForm) {{
    addForm.addEventListener('submit', function(ev) {{
      var typ = (document.getElementById('cf-add-type') || {{}}).value || '';
      var val = (addForm.querySelector('[name=content]') || {{}}).value || '';
      if (typ === 'AAAA' && /^\d{{1,3}}(\.\d{{1,3}}){{3}}$/.test(val.trim())) {{
        ev.preventDefault();
        alert('AAAA records require an IPv6 address, not IPv4 (for example 2001:db8::1).');
      }}
    }});
  }}
}})();
</script>"#,
        chips = type_filter_chips(domain, opts),
        toolbar = toolbar,
        scroll_cls = scroll_cls,
        rows = render_record_rows(domain, page_slice, opts),
        add_type_js = add_type_js,
    )
}
