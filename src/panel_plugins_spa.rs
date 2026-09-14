//! Client-side hub navigation for Plugins (tabs, category, search, pagination).
//! Uses fetch + history.pushState so the panel chrome does not fully reload.

pub fn plugins_hub_styles() -> &'static str {
    r#"
      .plugin-list-toolbar {
        display:flex; flex-wrap:wrap; gap:12px; align-items:center;
        margin:12px 0 8px; padding:10px 12px; border:1px solid var(--hairline);
        border-radius:12px; background:var(--canvas);
      }
      .plugin-mode-switch {
        display:inline-flex; border:1px solid var(--hairline); border-radius:999px; overflow:hidden;
      }
      .plugin-mode-switch button {
        min-height:34px; padding:0 14px; border:0; background:transparent; color:var(--ink);
        font:inherit; font-weight:600; cursor:pointer;
      }
      .plugin-mode-switch button.active {
        background:#1d4ed8; color:#ffffff;
      }
      .plugin-pager {
        display:flex; flex-wrap:wrap; gap:8px; align-items:center; margin-left:auto;
      }
      .plugin-pager label { font-size:13px; font-weight:600; color:var(--ink); }
      .plugin-pager select,
      .plugin-pager input[type="number"] {
        min-height:34px; border:1px solid #94a3b8; border-radius:8px; padding:4px 8px;
        font:inherit; color:var(--ink); background:var(--canvas); max-width:88px;
      }
      .plugin-pager .page-status { font-size:13px; font-weight:700; color:var(--ink); }
      .plugin-grid-scroll {
        max-height:none; overflow:visible; margin-top:10px; padding-right:2px;
      }
      .plugin-grid-scroll.is-scroll {
        max-height:min(70vh, 720px); overflow:auto; border:1px solid var(--hairline);
        border-radius:12px; padding:12px;
      }
      .plugin-hub-loading { opacity:.55; pointer-events:none; transition:opacity .15s ease; }
      [data-color-mode="dark"] .plugin-mode-switch button.active {
        background:#2563eb; color:#ffffff;
      }
      [data-color-mode="dark"] .plugin-pager select,
      [data-color-mode="dark"] .plugin-pager input[type="number"] {
        background:#1a1d26; border-color:#475569; color:#f1f5f9;
      }
    "#
}

pub fn plugins_hub_script() -> String {
    r#"
<script>
(function () {
  if (window.__cpnPluginsHubBound) return;
  window.__cpnPluginsHubBound = true;

  function hubRoot() {
    return document.getElementById('plugins-hub');
  }

  function toPartialUrl(href) {
    var u = new URL(href, window.location.origin);
    if (u.pathname !== '/plugins') return null;
    u.searchParams.set('partial', '1');
    return u;
  }

  function syncUrl(href, replace) {
    try {
      var u = new URL(href, window.location.origin);
      u.searchParams.delete('partial');
      var next = u.pathname + (u.search ? u.search : '');
      if (replace) history.replaceState({ cpnPlugins: 1 }, '', next);
      else history.pushState({ cpnPlugins: 1 }, '', next);
    } catch (e) {}
  }

  async function loadHub(href, opts) {
    opts = opts || {};
    var partial = toPartialUrl(href);
    if (!partial) {
      window.location.href = href;
      return;
    }
    var root = hubRoot();
    if (root) root.classList.add('plugin-hub-loading');
    try {
      var res = await fetch(partial.toString(), {
        credentials: 'same-origin',
        headers: { 'Accept': 'text/html', 'X-CPN-Partial': '1' }
      });
      if (!res.ok) throw new Error('bad status');
      var html = await res.text();
      var tmp = document.createElement('div');
      tmp.innerHTML = html;
      var next = tmp.querySelector('#plugins-hub');
      if (!next) throw new Error('missing hub');
      var cur = hubRoot();
      if (!cur) {
        window.location.href = href;
        return;
      }
      cur.replaceWith(next);
      syncUrl(href, !!opts.replace);
    } catch (err) {
      window.location.href = href;
    }
  }

  function buildStoreUrl(overrides) {
    var u = new URL(window.location.href);
    u.pathname = '/plugins';
    var currentView = u.searchParams.get('view') || 'store';
    if (currentView !== 'store' && currentView !== 'host' && currentView !== 'installed') {
      currentView = 'store';
    }
    u.searchParams.set('view', currentView);
    Object.keys(overrides || {}).forEach(function (k) {
      var v = overrides[k];
      if (v === null || v === undefined || v === '') u.searchParams.delete(k);
      else u.searchParams.set(k, String(v));
    });
    u.searchParams.delete('partial');
    return u.pathname + u.search;
  }

  document.addEventListener('click', function (ev) {
    var a = ev.target && ev.target.closest
      ? ev.target.closest('#plugins-hub a[href^="/plugins"]')
      : null;
    if (!a) return;
    if (ev.defaultPrevented || ev.button !== 0 || ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey) {
      return;
    }
    var href = a.getAttribute('href');
    if (!href || href.indexOf('/plugins') !== 0) return;
    ev.preventDefault();
    loadHub(href, { replace: false });
  }, true);

  document.addEventListener('submit', function (ev) {
    var form = ev.target;
    if (!form || !form.closest || !form.closest('#plugins-hub')) return;
    if ((form.getAttribute('method') || 'get').toLowerCase() !== 'get') return;
    if ((form.getAttribute('action') || '/plugins').indexOf('/plugins') !== 0) return;
    ev.preventDefault();
    var fd = new FormData(form);
    var u = new URL('/plugins', window.location.origin);
    fd.forEach(function (value, key) {
      if (value === '' && key !== 'q') return;
      u.searchParams.set(key, String(value));
    });
    if (!u.searchParams.get('view')) u.searchParams.set('view', 'store');
    loadHub(u.pathname + u.search, { replace: false });
  }, true);

  document.addEventListener('change', function (ev) {
    var el = ev.target;
    if (!el || !el.closest || !el.closest('#plugins-hub')) return;
    if (el.id === 'domain' && el.form) {
      ev.preventDefault();
      var fd = new FormData(el.form);
      var u = new URL('/plugins', window.location.origin);
      fd.forEach(function (value, key) {
        u.searchParams.set(key, String(value));
      });
      loadHub(u.pathname + u.search, { replace: false });
      return;
    }
    if (el.id === 'plugin-per-page') {
      var per = el.value || '4';
      loadHub(buildStoreUrl({ per_page: per, page: 1 }), { replace: false });
    }
  }, true);

  document.addEventListener('click', function (ev) {
    var btn = ev.target && ev.target.closest
      ? ev.target.closest('#plugins-hub [data-plugin-mode]')
      : null;
    if (!btn) return;
    ev.preventDefault();
    var mode = btn.getAttribute('data-plugin-mode') || 'page';
    loadHub(buildStoreUrl({ mode: mode, page: mode === 'scroll' ? null : '1' }), { replace: false });
  }, true);

  document.addEventListener('click', function (ev) {
    var btn = ev.target && ev.target.closest
      ? ev.target.closest('#plugins-hub [data-plugin-page]')
      : null;
    if (!btn || btn.disabled) return;
    ev.preventDefault();
    var page = btn.getAttribute('data-plugin-page');
    if (!page) return;
    loadHub(buildStoreUrl({ page: page, mode: 'page' }), { replace: false });
  }, true);

  document.addEventListener('submit', function (ev) {
    var form = ev.target;
    if (!form || form.id !== 'plugin-goto-form') return;
    ev.preventDefault();
    var input = form.querySelector('[name="goto_page"]');
    var page = input ? parseInt(input.value, 10) : 1;
    if (!page || page < 1) page = 1;
    loadHub(buildStoreUrl({ page: page, mode: 'page' }), { replace: false });
  }, true);

  window.addEventListener('popstate', function () {
    if (window.location.pathname !== '/plugins') return;
    loadHub(window.location.pathname + window.location.search, { replace: true });
  });
})();
</script>
"#
    .to_string()
}

pub fn list_mode_from_query(raw: &str) -> &'static str {
    if raw.eq_ignore_ascii_case("scroll") || raw.eq_ignore_ascii_case("scrollbar") {
        "scroll"
    } else {
        "page"
    }
}

pub fn per_page_from_query(raw: &str) -> usize {
    match raw.trim().parse::<usize>().unwrap_or(4) {
        n @ (4 | 8 | 12 | 16 | 24 | 48) => n,
        _ => 4,
    }
}

pub fn page_from_query(raw: &str) -> usize {
    raw.trim().parse::<usize>().unwrap_or(1).max(1)
}

pub fn store_list_toolbar(
    mode: &str,
    per_page: usize,
    page: usize,
    total_pages: usize,
    total_items: usize,
) -> String {
    let page_cls = if mode == "page" {
        "plugin-mode-btn active"
    } else {
        "plugin-mode-btn"
    };
    let scroll_cls = if mode == "scroll" {
        "plugin-mode-btn active"
    } else {
        "plugin-mode-btn"
    };
    let pager_style = if mode == "scroll" {
        " style=\"display:none\""
    } else {
        ""
    };
    let prev = if page > 1 { page - 1 } else { 1 };
    let next = if page < total_pages {
        page + 1
    } else {
        total_pages.max(1)
    };
    let prev_dis = if page <= 1 { " disabled" } else { "" };
    let next_dis = if page >= total_pages.max(1) {
        " disabled"
    } else {
        ""
    };
    let sizes = [4usize, 8, 12, 16, 24, 48];
    let mut options = String::new();
    for size in sizes {
        let sel = if size == per_page { " selected" } else { "" };
        options.push_str(&format!(
            r#"<option value="{size}"{sel}>{size}</option>"#,
            size = size,
            sel = sel
        ));
    }
    format!(
        r#"<div class="plugin-list-toolbar" role="group" aria-label="Catalog display">
        <div class="plugin-mode-switch" role="group" aria-label="List mode">
          <button type="button" class="{page_cls}" data-plugin-mode="page">Pagination</button>
          <button type="button" class="{scroll_cls}" data-plugin-mode="scroll">Scrollbar</button>
        </div>
        <span class="muted">{total} matching</span>
        <div class="plugin-pager"{pager_style}>
          <label for="plugin-per-page">Show</label>
          <select id="plugin-per-page" aria-label="Plugins per page">{options}</select>
          <span>per page</span>
          <span class="page-status">Page {page} / {total_pages}</span>
          <button type="button" class="btn-secondary" data-plugin-page="{prev}"{prev_dis}>Prev</button>
          <button type="button" class="btn-secondary" data-plugin-page="{next}"{next_dis}>Next</button>
          <form id="plugin-goto-form" style="display:inline-flex;flex-wrap:wrap;gap:6px;align-items:center;margin:0;">
            <label for="plugin-goto-page">Go to page</label>
            <input id="plugin-goto-page" name="goto_page" type="number" min="1" max="{total_pages}" value="{page}">
            <button type="submit" class="btn-primary">Go</button>
          </form>
        </div>
      </div>"#,
        page_cls = page_cls,
        scroll_cls = scroll_cls,
        pager_style = pager_style,
        options = options,
        page = page,
        total_pages = total_pages.max(1),
        prev = prev,
        next = next.max(1),
        prev_dis = prev_dis,
        next_dis = next_dis,
        total = total_items,
    )
}

#[cfg(test)]
mod tests {
    use super::{list_mode_from_query, page_from_query, per_page_from_query};

    #[test]
    fn per_page_defaults_to_four() {
        assert_eq!(per_page_from_query(""), 4);
        assert_eq!(per_page_from_query("   "), 4);
        assert_eq!(per_page_from_query("not-a-number"), 4);
        assert_eq!(per_page_from_query("0"), 4);
        assert_eq!(per_page_from_query("7"), 4);
        assert_eq!(per_page_from_query("3"), 4);
    }

    #[test]
    fn per_page_allows_selector_sizes() {
        for size in [4usize, 8, 12, 16, 24, 48] {
            assert_eq!(per_page_from_query(&size.to_string()), size);
        }
    }

    #[test]
    fn list_mode_and_page_helpers() {
        assert_eq!(list_mode_from_query("page"), "page");
        assert_eq!(list_mode_from_query("scroll"), "scroll");
        assert_eq!(list_mode_from_query("scrollbar"), "scroll");
        assert_eq!(page_from_query(""), 1);
        assert_eq!(page_from_query("0"), 1);
        assert_eq!(page_from_query("3"), 3);
    }
}
