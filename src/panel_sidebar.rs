//! Sidebar brand mark, menu search, and host IP/uptime card for CPN Panel.

use crate::panel_host_info::{HostSidebarInfo, host_sidebar_info};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Original CPN mark: rounded hex with linked nodes (not a lightning bolt).
pub fn brand_mark_svg() -> &'static str {
    r##"<svg class="cpn-brand-mark" viewBox="0 0 32 32" width="28" height="28" aria-hidden="true" focusable="false">
  <defs>
    <linearGradient id="cpnBrandGrad" x1="4" y1="2" x2="28" y2="30" gradientUnits="userSpaceOnUse">
      <stop stop-color="#3b82f6"/>
      <stop offset="1" stop-color="#06b6d4"/>
    </linearGradient>
  </defs>
  <path fill="url(#cpnBrandGrad)" d="M16 2.2 27.5 8.8v14.4L16 29.8 4.5 23.2V8.8L16 2.2z"/>
  <path fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"
    d="M11.2 16.2c0-2.6 2.1-4.6 4.8-4.6 1.7 0 3.1.8 4 2M20.8 15.8c0 2.6-2.1 4.6-4.8 4.6-1.7 0-3.1-.8-4-2"/>
  <circle cx="11.2" cy="16.2" r="1.55" fill="#fff"/>
  <circle cx="20.8" cy="15.8" r="1.55" fill="#fff"/>
  <circle cx="16" cy="11.6" r="1.35" fill="#e0f2fe"/>
  <circle cx="16" cy="20.4" r="1.35" fill="#e0f2fe"/>
</svg>"##
}

/// Extra searchable destinations beyond primary nav labels.
pub fn search_catalog_json() -> String {
    let entries: &[(&str, &str, &str)] = &[
        ("Dashboard", "/dashboard", "overview home"),
        ("Websites", "/websites", "sites domains"),
        ("Email", "/email", "mail postfix"),
        ("Databases & FTP", "/databases", "mariadb mysql ftp"),
        ("MariaDB Manager", "/databases/manager", "database"),
        ("Backups", "/backups", "restore"),
        ("Create Backup", "/backups/create", "backup"),
        ("Apps", "/apps", "applications"),
        ("Plugins", "/plugins", "store extensions"),
        ("Packages", "/packages", "plans hosting"),
        ("Users & Plans", "/account/users", "accounts"),
        ("Server", "/server", "system services"),
        ("Security", "/security", "firewall ssl"),
        ("Settings", "/settings", "design theme"),
        (
            "DNS Zones",
            "/server/dns/zones",
            "dns cloudflare nameserver",
        ),
        ("Nameservers", "/server/dns/nameservers", "dns ns"),
        ("Default Nameservers", "/server/dns/defaults", "dns"),
        (
            "Cloudflare DNS",
            "/server/dns/zones",
            "cloudflare dns management",
        ),
        ("Manage Websites", "/websites", "manage preview"),
    ];
    let items: Vec<String> = entries
        .iter()
        .map(|(label, href, keywords)| {
            format!(
                r#"{{"label":"{}","href":"{}","keywords":"{}"}}"#,
                html_escape(label),
                html_escape(href),
                html_escape(keywords)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

pub fn sidebar_extra_styles() -> &'static str {
    r#"
.panel-brand .cpn-brand-mark { flex:0 0 auto; display:block; border-radius:8px; }
.panel-brand span { line-height:1.15; }
.sidebar-search {
  position:relative; margin:14px 0 0; padding:0 4px;
}
.sidebar-search-field {
  display:flex; align-items:center; gap:8px; min-height:40px; padding:0 12px;
  border:1px solid var(--hairline); border-radius:12px; background:var(--canvas);
}
.sidebar-search-field:focus-within {
  border-color:var(--blue-focus); box-shadow:0 0 0 3px rgba(0,102,204,.15);
}
.sidebar-search-field svg { flex:0 0 auto; color:var(--muted); }
.sidebar-search-field input {
  flex:1; min-width:0; border:0; background:transparent; color:var(--ink);
  font:inherit; font-size:13px; outline:none;
}
.sidebar-search-results {
  display:none; position:absolute; left:4px; right:4px; top:calc(100% + 6px); z-index:60;
  max-height:220px; overflow:auto; margin:0; padding:6px; list-style:none;
  border:1px solid var(--hairline); border-radius:12px; background:var(--canvas);
  box-shadow:0 10px 28px rgba(15,23,42,.14);
}
.sidebar-search-results.is-open { display:block; }
.sidebar-search-results li button {
  width:100%; display:flex; flex-direction:column; align-items:flex-start; gap:2px;
  padding:8px 10px; border:0; border-radius:8px; background:transparent;
  color:var(--ink); text-align:left; font:inherit; font-size:13px; cursor:pointer;
}
.sidebar-search-results li button:hover,
.sidebar-search-results li button.is-active { background:rgba(0,102,204,.1); color:var(--blue); }
.sidebar-search-results .muted { color:var(--muted); font-size:11px; }
.sidebar-search-empty { padding:10px; color:var(--muted); font-size:12px; }
.host-status {
  display:flex; align-items:flex-start; gap:12px; margin:14px 0 12px; padding:12px 14px;
  border-radius:8px; background:var(--surface); border:0;
}
.host-status-icon {
  flex:0 0 auto; width:36px; height:36px; border-radius:10px;
  display:grid; place-items:center; background:rgba(59,130,246,.12); color:var(--blue);
}
.host-status-body { flex:1; min-width:0; display:flex; flex-direction:column; gap:4px; }
.host-status-row {
  display:flex; align-items:center; gap:8px; min-width:0; font-size:12px; color:var(--muted);
}
.host-status-row strong { color:var(--ink); font-weight:600; }
.cpn-ip-blur {
  display:inline-flex; align-items:center; gap:6px; min-width:0; max-width:100%;
}
.cpn-ip-blur__toggle {
  min-width:0; max-width:100%; padding:0; border:0; background:transparent;
  color:var(--blue); font:inherit; font-size:12px; font-weight:600; cursor:pointer;
  text-align:left;
}
.cpn-ip-blur__value {
  display:inline-block; max-width:100%; overflow:hidden; text-overflow:ellipsis;
  white-space:nowrap; transition:filter 120ms ease;
}
.cpn-ip-blur.is-blurred .cpn-ip-blur__value {
  filter:blur(5px); user-select:none;
}
.cpn-ip-blur__copy {
  flex:0 0 auto; width:28px; height:28px; display:grid; place-items:center;
  border:1px solid var(--hairline); border-radius:8px; background:var(--surface);
  color:var(--muted); cursor:pointer;
}
.cpn-ip-blur__copy:hover { color:var(--blue); border-color:var(--blue); }
.cpn-ip-blur__copy:focus-visible,
.cpn-ip-blur__toggle:focus-visible,
.sidebar-search-field input:focus-visible {
  outline:2px solid var(--blue-focus); outline-offset:2px;
}
.server-summary { margin:0 0 12px; }
[data-color-mode="dark"] .sidebar-search-field,
[data-color-mode="dark"] .sidebar-search-results,
[data-color-mode="dark"] .host-status,
[data-color-mode="dark"] .cpn-ip-blur__copy {
  background:var(--canvas); border-color:var(--hairline);
}
[data-color-mode="dark"] .host-status-icon { background:rgba(59,130,246,.2); }
"#
}

pub fn sidebar_header_html(username: &str) -> String {
    let user = html_escape(username);
    let host = host_sidebar_info();
    let catalog = search_catalog_json();
    format!(
        r#"<div class="sidebar-header">
        <a class="panel-brand" href="/dashboard">{mark}<span>CPN Panel</span></a>
        <div class="sidebar-search" data-cpn-menu-search>
          <label class="sidebar-search-field">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
              <circle cx="11" cy="11" r="7"></circle>
              <path d="m20 20-3.5-3.5"></path>
            </svg>
            <span class="sr-only">Search menu</span>
            <input type="search" id="cpn-menu-search" placeholder="Search menu..." autocomplete="off" aria-autocomplete="list" aria-controls="cpn-menu-search-results" aria-expanded="false">
          </label>
          <ul id="cpn-menu-search-results" class="sidebar-search-results" role="listbox" hidden></ul>
          <script type="application/json" id="cpn-menu-search-data">{catalog}</script>
        </div>
        {host_card}
        <div class="server-summary">
          <div>
            <strong>{user}</strong>
            <span>Signed in</span>
          </div>
        </div>
      </div>"#,
        mark = brand_mark_svg(),
        catalog = catalog,
        host_card = host_status_card_html(&host),
        user = user,
    )
}

fn host_status_card_html(host: &HostSidebarInfo) -> String {
    let ip = html_escape(&host.ip);
    let uptime = html_escape(&host.uptime);
    format!(
        r#"<div class="host-status" data-cpn-host-status>
          <div class="host-status-icon" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
              <rect x="3" y="4" width="18" height="12" rx="2"></rect>
              <path d="M8 20h8M12 16v4"></path>
            </svg>
          </div>
          <div class="host-status-body">
            <div class="host-status-row">
              <span>IP:</span>
              <span class="cpn-ip-blur is-blurred" data-ip="{ip}" data-cpn-ip-blur>
                <button type="button" class="cpn-ip-blur__toggle" aria-pressed="false" aria-label="Click to show IP" title="Click to show or hide IP">
                  <span class="cpn-ip-blur__value">{ip}</span>
                </button>
                <button type="button" class="cpn-ip-blur__copy" aria-label="Copy IP" title="Copy IP">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                    <rect x="9" y="9" width="13" height="13" rx="2"></rect>
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                  </svg>
                </button>
              </span>
            </div>
            <div class="host-status-row"><span>Uptime:</span> <strong>{uptime}</strong></div>
          </div>
        </div>"#
    )
}

pub fn sidebar_search_and_ip_script() -> &'static str {
    r#"
<script>
(function () {
  var STORAGE_KEY = 'cpn.sidebar.ipBlurred';
  var root = document.querySelector('[data-cpn-ip-blur]');
  if (root) {
    var toggle = root.querySelector('.cpn-ip-blur__toggle');
    var copyBtn = root.querySelector('.cpn-ip-blur__copy');
    var valueEl = root.querySelector('.cpn-ip-blur__value');
    function setBlurred(blurred) {
      root.classList.toggle('is-blurred', blurred);
      if (toggle) {
        toggle.setAttribute('aria-pressed', blurred ? 'false' : 'true');
        toggle.setAttribute('aria-label', blurred ? 'Click to show IP' : 'Click to hide IP');
      }
      try { localStorage.setItem(STORAGE_KEY, blurred ? '1' : '0'); } catch (e) {}
    }
    var stored = null;
    try { stored = localStorage.getItem(STORAGE_KEY); } catch (e) {}
    if (stored === '0') setBlurred(false);
    else setBlurred(true);
    if (toggle) {
      toggle.addEventListener('click', function () {
        setBlurred(!root.classList.contains('is-blurred'));
      });
    }
    if (copyBtn) {
      copyBtn.addEventListener('click', function () {
        var ip = root.getAttribute('data-ip') || (valueEl ? valueEl.textContent : '') || '';
        if (!ip) return;
        function done() {
          copyBtn.setAttribute('aria-label', 'Copied');
          setTimeout(function () { copyBtn.setAttribute('aria-label', 'Copy IP'); }, 1200);
        }
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText(ip).then(done).catch(function () {
            var ta = document.createElement('textarea');
            ta.value = ip; document.body.appendChild(ta); ta.select();
            try { document.execCommand('copy'); } catch (e) {}
            document.body.removeChild(ta); done();
          });
        } else {
          var ta = document.createElement('textarea');
          ta.value = ip; document.body.appendChild(ta); ta.select();
          try { document.execCommand('copy'); } catch (e) {}
          document.body.removeChild(ta); done();
        }
      });
    }
  }

  var wrap = document.querySelector('[data-cpn-menu-search]');
  if (!wrap) return;
  var input = wrap.querySelector('input');
  var list = wrap.querySelector('#cpn-menu-search-results');
  var dataNode = wrap.querySelector('#cpn-menu-search-data');
  if (!input || !list || !dataNode) return;
  var catalog = [];
  try { catalog = JSON.parse(dataNode.textContent || '[]'); } catch (e) { catalog = []; }
  var hostCard = document.querySelector('[data-cpn-host-status]');
  if (hostCard) {
    var hostIp = (hostCard.querySelector('[data-ip]') || {}).getAttribute
      ? hostCard.querySelector('[data-ip]').getAttribute('data-ip') : '';
    var uptimeEl = hostCard.querySelector('.host-status-row:last-child strong');
    catalog.push({
      label: 'Server IP',
      href: '#cpn-host-ip',
      keywords: 'ip address host ' + (hostIp || '')
    });
    catalog.push({
      label: 'Uptime ' + ((uptimeEl && uptimeEl.textContent) || ''),
      href: '#cpn-host-uptime',
      keywords: 'uptime status host'
    });
  }
  var active = -1;
  function normalize(value) { return String(value || '').toLowerCase().trim(); }
  function matches(item, q) {
    if (!q) return false;
    return normalize(item.label).indexOf(q) !== -1
      || normalize(item.keywords).indexOf(q) !== -1
      || normalize(item.href).indexOf(q) !== -1;
  }
  function closeList() {
    list.classList.remove('is-open');
    list.hidden = true;
    list.innerHTML = '';
    active = -1;
    input.setAttribute('aria-expanded', 'false');
  }
  function go(href) {
    if (!href) return;
    if (href.indexOf('#cpn-host-') === 0) {
      if (hostCard) hostCard.scrollIntoView({ block: 'nearest' });
      closeList();
      return;
    }
    window.location.href = href;
  }
  function render(q) {
    var query = normalize(q);
    if (!query) { closeList(); return; }
    var hits = catalog.filter(function (item) { return matches(item, query); }).slice(0, 8);
    list.innerHTML = '';
    if (!hits.length) {
      list.innerHTML = '<li class="sidebar-search-empty">No matching pages</li>';
    } else {
      hits.forEach(function (item, index) {
        var li = document.createElement('li');
        li.setAttribute('role', 'option');
        var btn = document.createElement('button');
        btn.type = 'button';
        btn.dataset.href = item.href;
        btn.dataset.index = String(index);
        btn.innerHTML = '<span>' + item.label + '</span><span class="muted">' + item.href + '</span>';
        btn.addEventListener('click', function () { go(item.href); });
        li.appendChild(btn);
        list.appendChild(li);
      });
    }
    list.hidden = false;
    list.classList.add('is-open');
    input.setAttribute('aria-expanded', 'true');
    active = hits.length ? 0 : -1;
    updateActive();
  }
  function updateActive() {
    var buttons = list.querySelectorAll('button[data-href]');
    buttons.forEach(function (btn, index) {
      btn.classList.toggle('is-active', index === active);
    });
  }
  input.addEventListener('input', function () { render(input.value); });
  input.addEventListener('keydown', function (event) {
    var buttons = list.querySelectorAll('button[data-href]');
    if (event.key === 'ArrowDown' && buttons.length) {
      event.preventDefault();
      active = (active + 1) % buttons.length;
      updateActive();
    } else if (event.key === 'ArrowUp' && buttons.length) {
      event.preventDefault();
      active = (active - 1 + buttons.length) % buttons.length;
      updateActive();
    } else if (event.key === 'Enter') {
      var target = buttons[active] || buttons[0];
      if (target) {
        event.preventDefault();
        go(target.dataset.href);
      }
    } else if (event.key === 'Escape') {
      closeList();
      input.blur();
    }
  });
  document.addEventListener('click', function (event) {
    if (!wrap.contains(event.target)) closeList();
  });
})();
</script>
"#
}

#[cfg(test)]
mod tests {
    use super::{brand_mark_svg, search_catalog_json, sidebar_header_html};

    #[test]
    fn brand_mark_is_original_cpn() {
        let svg = brand_mark_svg();
        assert!(svg.contains("cpn-brand-mark"));
        assert!(!svg.to_lowercase().contains("cyberpanel"));
        assert!(!svg.contains("lightning"));
    }

    #[test]
    fn catalog_includes_email_and_dns() {
        let json = search_catalog_json();
        assert!(json.contains("/email"));
        assert!(json.contains("Cloudflare DNS") || json.contains("/server/dns/zones"));
    }

    #[test]
    fn header_embeds_blurred_ip_and_search() {
        let html = sidebar_header_html("Admin");
        assert!(html.contains("Search menu"));
        assert!(html.contains("cpn-ip-blur is-blurred"));
        assert!(html.contains("data-ip="));
        assert!(html.contains("Uptime:"));
        assert!(html.contains("CPN Panel"));
    }
}
