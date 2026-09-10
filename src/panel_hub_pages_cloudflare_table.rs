//! Cloudflare Manage DNS records table (filter chips + inline edit rows).

use crate::panel_ops_cloudflare::RECORD_TYPES;
use crate::panel_ops_cloudflare_api::CfDnsRecord;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn type_filter_chips(selected: &str) -> String {
    let mut out = String::from(
        r#"<div class="cf-type-row" role="group" aria-label="Filter DNS records by type">
<span class="muted" style="margin-right:4px;">Filter:</span>"#,
    );
    let all_active = if selected.is_empty() || selected.eq_ignore_ascii_case("all") {
        " active"
    } else {
        ""
    };
    out.push_str(&format!(
        r#"<button type="button" class="cf-type-chip{all_active}" data-cf-filter="ALL" onclick="cfFilterType('ALL')">All</button>"#,
        all_active = all_active,
    ));
    for t in RECORD_TYPES {
        let active = if selected.eq_ignore_ascii_case(t) {
            " active"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<button type="button" class="cf-type-chip{active}" data-cf-filter="{t}" onclick="cfFilterType('{t}')">{t}</button>"#,
            active = active,
            t = t,
        ));
    }
    out.push_str(
        r#" <span id="cf-filter-count" class="muted" style="margin-left:8px;"></span></div>"#,
    );
    out
}

pub(crate) fn records_table(domain: &str, records: &[CfDnsRecord], filter_type: &str) -> String {
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
        let pri_edit = if matches!(r.record_type.as_str(), "MX" | "SRV") {
            format!(
                r#"<input class="cf-edit-input" name="priority" type="number" value="{v}" min="0">"#,
                v = html_escape(&pri_val),
            )
        } else {
            r#"<span class="muted">-</span>"#.into()
        };
        let hidden = if !filter_type.is_empty()
            && !filter_type.eq_ignore_ascii_case("all")
            && !r.record_type.eq_ignore_ascii_case(filter_type)
        {
            " style=\"display:none\""
        } else {
            ""
        };
        rows.push_str(&format!(
            r#"<tr class="cf-row-view" data-rtype="{ty}" data-rid="{id}"{hidden}>
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
      <input type="hidden" name="filter_type" value="{ft}">
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
      <input type="hidden" name="filter_type" value="{ft}">
      <button type="submit" class="btn-danger" aria-label="Delete record">Delete</button>
    </form>
  </td>
</tr>
<tr class="cf-row-edit" data-rtype="{ty}" data-rid="{id}"{hidden}>
  <td colspan="7">
    <form method="post" action="/dns/cloudflare/update" class="cf-add-row" style="margin:0;">
      <input type="hidden" name="domain" value="{dom}">
      <input type="hidden" name="record_id" value="{id}">
      <input type="hidden" name="filter_type" value="{ft}">
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
            ft = html_escape(filter_type),
            hidden = hidden,
        ));
    }
    let initial = if filter_type.is_empty() {
        "ALL"
    } else {
        filter_type
    };
    let initial_js = serde_json::to_string(initial).unwrap_or_else(|_| "\"ALL\"".into());
    format!(
        r#"<h3>DNS Records</h3>
{chips}
<table class="cf-table" id="cf-records-table">
  <thead><tr><th>NAME</th><th>TYPE</th><th>TTL</th><th>VALUE</th><th>PRIORITY</th><th>PROXY</th><th>ACTIONS</th></tr></thead>
  <tbody>{rows}</tbody>
</table>
<script>
(function(){{
  var initial = {initial_js};
  window.cfFilterType = function(t) {{
    t = (t || 'ALL').toUpperCase();
    document.querySelectorAll('.cf-type-chip').forEach(function(btn) {{
      btn.classList.toggle('active', (btn.getAttribute('data-cf-filter') || '') === t);
    }});
    var shown = 0, total = 0;
    document.querySelectorAll('#cf-records-table tr[data-rtype]').forEach(function(tr) {{
      if (tr.classList.contains('cf-row-edit') && !tr.classList.contains('is-open')) {{
        tr.style.display = 'none';
        return;
      }}
      var rt = (tr.getAttribute('data-rtype') || '').toUpperCase();
      if (tr.classList.contains('cf-row-view')) total++;
      var match = (t === 'ALL' || rt === t);
      if (tr.classList.contains('cf-row-view')) {{
        tr.style.display = match ? '' : 'none';
        if (match) shown++;
      }} else if (tr.classList.contains('cf-row-edit')) {{
        if (!match) {{ tr.classList.remove('is-open'); tr.style.display = 'none'; }}
      }}
    }});
    var cnt = document.getElementById('cf-filter-count');
    if (cnt) cnt.textContent = (t === 'ALL')
      ? ('Showing ' + shown + ' of ' + total + ' records')
      : ('Showing ' + shown + ' of ' + total + ' · ' + t);
    var sel = document.getElementById('cf-add-type');
    if (sel && t !== 'ALL') sel.value = t;
    try {{
      var u = new URL(window.location.href);
      u.searchParams.set('tab', 'manage');
      if (t === 'ALL') u.searchParams.delete('type'); else u.searchParams.set('type', t);
      history.replaceState(null, '', u.pathname + '?' + u.searchParams.toString());
    }} catch (e) {{}}
  }};
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
  cfFilterType(initial);
}})();
</script>"#,
        chips = type_filter_chips(filter_type),
        rows = rows,
        initial_js = initial_js,
    )
}
