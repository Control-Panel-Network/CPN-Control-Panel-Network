//! Fail2Ban manager UI: list, search, filter, unban (issue #627).

use crate::panel_hubs::{feature_shell, not_configured_body, status_kv};
use crate::panel_ops_fail2ban::{fail2ban_csrf_token, list_banned};
use crate::panel_ops_security::fail2ban_status;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fail2ban_styles() -> &'static str {
    r#"
<style>
.f2b-toolbar{display:flex;flex-wrap:wrap;gap:10px;align-items:flex-end;margin:12px 0 16px}
.f2b-toolbar label{display:flex;flex-direction:column;gap:4px;font-size:13px;font-weight:600}
.f2b-toolbar input,.f2b-toolbar select{min-height:36px;min-width:160px;padding:6px 10px;border:1px solid #94a3b8;border-radius:8px;font:inherit;background:var(--canvas);color:var(--ink)}
.f2b-toolbar button{min-height:36px;padding:0 14px;border-radius:8px;border:1px solid #1d4ed8;background:#1d4ed8;color:#fff;font:inherit;font-weight:600;cursor:pointer}
.f2b-toolbar button.secondary{background:transparent;color:var(--ink);border-color:#94a3b8}
.f2b-status{margin:0 0 12px;font-size:13px}
.f2b-status[data-state="stale"]{color:#b45309}
.f2b-status[data-state="error"]{color:#b91c1c}
.f2b-table-wrap{overflow-x:auto;border:1px solid var(--hairline);border-radius:12px}
.f2b-table{width:100%;border-collapse:collapse;font-size:14px}
.f2b-table th,.f2b-table td{padding:10px 12px;text-align:left;border-bottom:1px solid var(--hairline)}
.f2b-table th{background:var(--canvas);font-size:12px;text-transform:uppercase;letter-spacing:.03em}
.f2b-table tr:last-child td{border-bottom:0}
.f2b-empty{padding:18px 12px;margin:0;color:var(--muted,#64748b)}
.f2b-unban{min-height:32px;padding:0 10px;border-radius:8px;border:1px solid #b91c1c;background:transparent;color:#b91c1c;font:inherit;font-weight:600;cursor:pointer}
.f2b-unban:disabled{opacity:.5;cursor:wait}
[data-color-mode="dark"] .f2b-toolbar input,[data-color-mode="dark"] .f2b-toolbar select{background:#1a1d26;border-color:#475569;color:#f1f5f9}
</style>
"#
}

fn fail2ban_script(csrf: &str, is_admin: bool) -> String {
    let admin_js = if is_admin { "true" } else { "false" };
    format!(
        r#"
<script>
(function(){{
  var csrf={csrf_json};
  var isAdmin={admin_js};
  var root=document.getElementById('f2b-root');
  if(!root) return;
  var statusEl=document.getElementById('f2b-status');
  var tbody=document.getElementById('f2b-tbody');
  var emptyEl=document.getElementById('f2b-empty');
  var jailSel=document.getElementById('f2b-jail');
  var searchEl=document.getElementById('f2b-search');
  var refreshBtn=document.getElementById('f2b-refresh');
  var lastBans=[];
  var busy=false;

  function esc(s){{
    return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
  }}
  function setStatus(msg, state){{
    if(!statusEl) return;
    statusEl.textContent=msg||'';
    statusEl.setAttribute('data-state', state||'ok');
    statusEl.setAttribute('role', state==='error' ? 'alert' : 'status');
  }}
  function applyFilter(){{
    var q=(searchEl && searchEl.value || '').trim().toLowerCase();
    var jail=(jailSel && jailSel.value || '').trim();
    var rows=lastBans.filter(function(b){{
      if(jail && b.jail!==jail) return false;
      if(!q) return true;
      return String(b.ip).toLowerCase().indexOf(q)>=0 || String(b.jail).toLowerCase().indexOf(q)>=0;
    }});
    if(!tbody) return;
    tbody.innerHTML='';
    if(rows.length===0){{
      if(emptyEl){{ emptyEl.hidden=false; emptyEl.textContent=lastBans.length===0 ? 'No banned IPs right now.' : 'No matches for the current search or jail filter.'; }}
      return;
    }}
    if(emptyEl) emptyEl.hidden=true;
    rows.forEach(function(b){{
      var tr=document.createElement('tr');
      var unbanCell=isAdmin
        ? '<td><button type="button" class="f2b-unban" data-ip="'+esc(b.ip)+'" data-jail="'+esc(b.jail)+'">Unban</button></td>'
        : '<td><span class="muted">Admin only</span></td>';
      tr.innerHTML='<td><code>'+esc(b.ip)+'</code></td><td><code>'+esc(b.jail)+'</code></td>'+unbanCell;
      tbody.appendChild(tr);
    }});
  }}
  function fillJails(jails){{
    if(!jailSel) return;
    var cur=jailSel.value;
    jailSel.innerHTML='<option value="">All jails</option>';
    (jails||[]).forEach(function(j){{
      var opt=document.createElement('option');
      opt.value=j; opt.textContent=j;
      if(j===cur) opt.selected=true;
      jailSel.appendChild(opt);
    }});
  }}
  async function loadBans(){{
    if(busy) return;
    busy=true;
    if(refreshBtn) refreshBtn.disabled=true;
    setStatus('Loading banned IPs…', 'ok');
    try{{
      var jail=(jailSel && jailSel.value) || '';
      var url='/api/security/fail2ban/bans'+(jail ? ('?jail='+encodeURIComponent(jail)) : '');
      var res=await fetch(url,{{credentials:'same-origin',headers:{{'Accept':'application/json'}}}});
      var data=await res.json().catch(function(){{return null;}});
      if(!res.ok || !data){{
        setStatus((data && data.error) || ('Could not load bans (HTTP '+res.status+').'), 'error');
        lastBans=[];
        applyFilter();
        return;
      }}
      fillJails(data.jails||[]);
      lastBans=Array.isArray(data.bans)?data.bans:[];
      setStatus(data.detail || ('Updated '+new Date().toLocaleTimeString()), data.available===false ? 'stale' : 'ok');
      applyFilter();
    }}catch(e){{
      setStatus('Network error while loading Fail2Ban bans.', 'error');
    }}finally{{
      busy=false;
      if(refreshBtn) refreshBtn.disabled=false;
    }}
  }}
  async function unban(ip, jail, btn){{
    if(!isAdmin) return;
    if(!window.confirm('Unban '+ip+' from jail '+jail+'?')) return;
    if(btn) btn.disabled=true;
    setStatus('Unbanning '+ip+'…', 'ok');
    try{{
      var body=new URLSearchParams();
      body.set('csrf', csrf);
      body.set('ip', ip);
      body.set('jail', jail);
      var res=await fetch('/api/security/fail2ban/unban',{{
        method:'POST',
        credentials:'same-origin',
        headers:{{'Content-Type':'application/x-www-form-urlencoded','Accept':'application/json'}},
        body:body.toString()
      }});
      var data=await res.json().catch(function(){{return null;}});
      if(!res.ok || !data || !data.ok){{
        setStatus((data && data.error) || ('Unban failed (HTTP '+res.status+').'), 'error');
        if(btn) btn.disabled=false;
        return;
      }}
      setStatus(data.message || ('Unbanned '+ip), 'ok');
      await loadBans();
    }}catch(e){{
      setStatus('Network error during unban.', 'error');
      if(btn) btn.disabled=false;
    }}
  }}
  if(refreshBtn) refreshBtn.addEventListener('click', function(ev){{ ev.preventDefault(); loadBans(); }});
  if(searchEl) searchEl.addEventListener('input', applyFilter);
  if(jailSel) jailSel.addEventListener('change', function(){{ loadBans(); }});
  if(tbody) tbody.addEventListener('click', function(ev){{
    var btn=ev.target && ev.target.closest ? ev.target.closest('button.f2b-unban') : null;
    if(!btn) return;
    unban(btn.getAttribute('data-ip')||'', btn.getAttribute('data-jail')||'', btn);
  }});
  loadBans();
}})();
</script>
"#,
        csrf_json = serde_json::to_string(csrf).unwrap_or_else(|_| "\"\"".into()),
        admin_js = admin_js,
    )
}

/// Full Fail2Ban page body for `/security/fail2ban`.
pub fn fail2ban_manager_page(username: &str, is_admin: bool) -> String {
    let st = fail2ban_status();
    if !st.installed {
        return feature_shell(
            &[
                ("Dashboard", Some("/dashboard")),
                ("Security", Some("/security")),
                ("Fail2ban", None),
            ],
            "Fail2ban",
            "Brute-force protection.",
            &not_configured_body(
                &st.detail,
                "Install fail2ban on this host, then reopen this tile for live jail status and ban management.",
            ),
            None,
            None,
        );
    }

    let csrf = fail2ban_csrf_token(username);
    let inv = list_banned(None);
    let kv = status_kv(&[
        ("Installed", "yes"),
        ("Active", &st.active),
        ("Enabled", &st.enabled),
    ]);
    let admin_note = if is_admin {
        "<p class=\"muted\">Search banned IPs, filter by jail, and unban with confirmation. Actions are audited.</p>"
    } else {
        "<p class=\"muted\">You can view banned IPs. Only the panel admin can unban.</p>"
    };
    let body = format!(
        r#"{styles}
{kv}
{admin_note}
<div id="f2b-root" data-f2b="1">
  <p id="f2b-status" class="f2b-status" role="status">{detail}</p>
  <div class="f2b-toolbar">
    <label>Search IP
      <input id="f2b-search" type="search" placeholder="IPv4 or IPv6" autocomplete="off" aria-label="Search banned IP">
    </label>
    <label>Jail
      <select id="f2b-jail" aria-label="Filter by jail">
        <option value="">All jails</option>
        {jail_opts}
      </select>
    </label>
    <button type="button" id="f2b-refresh" class="secondary">Refresh</button>
  </div>
  <div class="f2b-table-wrap">
    <table class="f2b-table" aria-label="Banned IP addresses">
      <thead><tr><th scope="col">IP</th><th scope="col">Jail</th><th scope="col">Action</th></tr></thead>
      <tbody id="f2b-tbody"></tbody>
    </table>
    <p id="f2b-empty" class="f2b-empty" hidden>No banned IPs right now.</p>
  </div>
</div>
{script}"#,
        styles = fail2ban_styles(),
        kv = kv,
        admin_note = admin_note,
        detail = html_escape(&inv.detail),
        jail_opts = st
            .jails
            .iter()
            .map(|j| format!(
                r#"<option value="{}">{}</option>"#,
                html_escape(j),
                html_escape(j)
            ))
            .collect::<String>(),
        script = fail2ban_script(&csrf, is_admin),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Security", Some("/security")),
            ("Fail2ban", None),
        ],
        "Fail2ban",
        "List, search, and unban blocked IPs.",
        &body,
        None,
        None,
    )
}
