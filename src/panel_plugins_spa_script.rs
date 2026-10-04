//! Client-side Plugin Store: filter Host/Site, categories, search, and pagination
//! from the already-rendered catalog (no full hub fetch on tab switches).

pub fn plugins_hub_script() -> String {
    r#"
<script>
(function () {
  if (window.__cpnPluginsHubBound) return;
  window.__cpnPluginsHubBound = true;

  function hubRoot() {
    return document.getElementById('plugins-hub');
  }

  function storeSource() {
    var root = hubRoot();
    return root ? root.querySelector('#plugin-store-source') : null;
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
      applyStoreFilter(true);
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
    if (currentView === 'host') {
      currentView = 'store';
      u.searchParams.set('target', 'host');
      if ((u.searchParams.get('category') || '').toLowerCase() === 'host') {
        u.searchParams.delete('category');
      }
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

  function allowHost() {
    var root = hubRoot();
    return !!(root && root.getAttribute('data-store-allow-host') === '1');
  }

  function readStoreState() {
    var u = new URL(window.location.href);
    var target = (u.searchParams.get('target') || '').toLowerCase();
    var category = u.searchParams.get('category') || '';
    if (category.toLowerCase() === 'host') {
      target = 'host';
      category = '';
    }
    if (target !== 'host' && target !== 'site') target = 'site';
    if (target === 'host' && !allowHost()) target = 'site';
    var mode = (u.searchParams.get('mode') || 'page').toLowerCase();
    if (mode === 'scrollbar') mode = 'scroll';
    if (mode !== 'scroll') mode = 'page';
    var per = parseInt(u.searchParams.get('per_page') || '4', 10);
    if ([4, 8, 12, 16, 24, 48].indexOf(per) < 0) per = 4;
    var page = parseInt(u.searchParams.get('page') || '1', 10);
    if (!page || page < 1) page = 1;
    return {
      target: target,
      category: category,
      q: u.searchParams.get('q') || '',
      mode: mode,
      page: page,
      per_page: per
    };
  }

  function itemMatches(el, st) {
    var t = (el.getAttribute('data-store-target') || 'site').toLowerCase();
    if (st.target === 'host' && t !== 'host' && t !== 'both') return false;
    if (st.target === 'site' && t !== 'site' && t !== 'both') return false;
    var cat = (st.category || '').trim().toLowerCase();
    var itemCat = (el.getAttribute('data-cat') || '').toLowerCase();
    if (cat && cat !== 'all') {
      if (cat === 'featured') {
        if (el.getAttribute('data-featured') !== '1') return false;
      } else if (cat === 'paid') {
        if (el.getAttribute('data-paid') !== '1') return false;
      } else if (cat === 'free') {
        if (el.getAttribute('data-paid') === '1') return false;
      } else if (cat === 'host') {
        if (t !== 'host' && t !== 'both') return false;
      } else if (itemCat !== cat) {
        return false;
      }
    }
    var q = (st.q || '').trim().toLowerCase();
    if (!q) return true;
    if (q === 'paid' || q === 'premium') return el.getAttribute('data-paid') === '1';
    if (q === 'free') return el.getAttribute('data-paid') !== '1';
    var hay = (el.getAttribute('data-search') || '').toLowerCase();
    return hay.indexOf(q) >= 0;
  }

  function applyStoreFilter(replaceHistory) {
    var src = storeSource();
    if (!src) return false;
    var st = readStoreState();
    var href = buildStoreUrl({
      view: 'store',
      target: st.target,
      category: st.category || null,
      q: st.q || null,
      mode: st.mode,
      per_page: String(st.per_page),
      page: st.mode === 'scroll' ? null : String(st.page)
    });
    syncUrl(href, !!replaceHistory);
    var cards = [];
    src.querySelectorAll('article.plugin-card').forEach(function (el) {
      if (itemMatches(el, st)) cards.push(el);
    });
    var total = cards.length;
    var per = st.mode === 'scroll' ? Math.max(total, 1) : st.per_page;
    var pages = st.mode === 'scroll' ? 1 : Math.max(Math.ceil(total / per) || 1, 1);
    var page = Math.min(st.page, pages);
    var start = st.mode === 'scroll' ? 0 : (page - 1) * per;
    var slice = cards.slice(start, st.mode === 'scroll' ? total : start + per);
    var grid = document.querySelector('#plugin-store-live .plugin-grid');
    var wrap = document.querySelector('#plugin-store-live .plugin-grid-scroll');
    var empty = document.getElementById('plugin-store-empty');
    if (grid) {
      grid.innerHTML = '';
      slice.forEach(function (el) { grid.appendChild(el.cloneNode(true)); });
    }
    if (wrap) {
      if (st.mode === 'scroll') wrap.classList.add('is-scroll');
      else wrap.classList.remove('is-scroll');
    }
    if (empty) empty.hidden = total > 0;
    if (grid) grid.hidden = total === 0;
    var count = document.querySelector('#plugins-hub .plugin-list-toolbar .muted');
    if (count) count.textContent = total + ' matching';
    var status = document.querySelector('#plugins-hub .page-status');
    if (status) status.textContent = 'Page ' + page + ' / ' + pages;
    var pager = document.querySelector('#plugins-hub .plugin-pager');
    if (pager) pager.style.display = st.mode === 'scroll' ? 'none' : '';
    document.querySelectorAll('#plugins-hub [data-plugin-mode]').forEach(function (btn) {
      btn.classList.toggle('active', btn.getAttribute('data-plugin-mode') === st.mode);
    });
    var prev = document.querySelector('#plugins-hub [data-plugin-page="prev"]')
      || document.querySelector('#plugins-hub [data-plugin-page]');
    document.querySelectorAll('#plugins-hub [data-plugin-page]').forEach(function (btn) {
      var label = (btn.textContent || '').trim().toLowerCase();
      if (label === 'prev') {
        btn.setAttribute('data-plugin-page', String(Math.max(page - 1, 1)));
        btn.disabled = page <= 1;
      } else if (label === 'next') {
        btn.setAttribute('data-plugin-page', String(Math.min(page + 1, pages)));
        btn.disabled = page >= pages;
      }
    });
    var goto = document.getElementById('plugin-goto-page');
    if (goto) {
      goto.value = String(page);
      goto.max = String(pages);
    }
    var perSel = document.getElementById('plugin-per-page');
    if (perSel) perSel.value = String(st.per_page);
    document.querySelectorAll('#plugins-hub .category-pills a').forEach(function (a) {
      var val = (a.getAttribute('data-store-cat') || '').toLowerCase();
      var active = val === (st.category || '').toLowerCase()
        || (!st.category && (val === '' || val === 'all'));
      a.classList.toggle('active', active);
    });
    document.querySelectorAll('#plugins-hub [data-store-target-btn]').forEach(function (a) {
      a.classList.toggle('active', a.getAttribute('data-store-target-btn') === st.target);
    });
    var hostHint = document.getElementById('store-host-hint');
    var siteBox = document.getElementById('store-site-picker');
    if (hostHint) hostHint.hidden = st.target !== 'host';
    if (siteBox) siteBox.hidden = st.target !== 'site';
    var qInput = document.getElementById('q');
    if (qInput && document.activeElement !== qInput) qInput.value = st.q;
    document.querySelectorAll('#plugins-hub input[name="target"]').forEach(function (inp) {
      inp.value = st.target;
    });
    document.querySelectorAll('#plugins-hub input[name="category"]').forEach(function (inp) {
      inp.value = st.category;
    });
    return true;
  }

  function isStoreClientNav(a) {
    if (!storeSource()) return false;
    if (a.hasAttribute('data-store-cat') || a.hasAttribute('data-store-target-btn')) return true;
    var href = a.getAttribute('href') || '';
    if (href.indexOf('view=installed') >= 0) return false;
    if (href.indexOf('refresh=1') >= 0) return false;
    if (href.indexOf('view=store') >= 0 && href.indexOf('/plugins') === 0) {
      if (href.indexOf('category=') >= 0 || href.indexOf('target=') >= 0) return true;
    }
    return false;
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
    if (isStoreClientNav(a)) {
      ev.preventDefault();
      var next = new URL(href, window.location.origin);
      var cur = new URL(window.location.href);
      if (!next.searchParams.get('q') && cur.searchParams.get('q')) {
        next.searchParams.set('q', cur.searchParams.get('q'));
      }
      if (!next.searchParams.get('mode') && cur.searchParams.get('mode')) {
        next.searchParams.set('mode', cur.searchParams.get('mode'));
      }
      if (!next.searchParams.get('per_page') && cur.searchParams.get('per_page')) {
        next.searchParams.set('per_page', cur.searchParams.get('per_page'));
      }
      next.searchParams.set('page', '1');
      syncUrl(next.pathname + next.search, false);
      applyStoreFilter(true);
      return;
    }
    ev.preventDefault();
    loadHub(href, { replace: false });
  }, true);

  document.addEventListener('submit', function (ev) {
    var form = ev.target;
    if (!form || !form.closest || !form.closest('#plugins-hub')) return;
    if ((form.getAttribute('method') || 'get').toLowerCase() !== 'get') return;
    if ((form.getAttribute('action') || '/plugins').indexOf('/plugins') !== 0) return;
    if (form.querySelector('[name="refresh"]') && ev.submitter && ev.submitter.name === 'refresh') {
      return;
    }
    ev.preventDefault();
    var fd = new FormData(form);
    if (storeSource() && form.classList.contains('plugin-search-row')) {
      var q = String(fd.get('q') || '');
      var u = new URL(window.location.href);
      if (q) u.searchParams.set('q', q); else u.searchParams.delete('q');
      u.searchParams.set('page', '1');
      syncUrl(u.pathname + u.search, false);
      applyStoreFilter(true);
      return;
    }
    var u = new URL('/plugins', window.location.origin);
    fd.forEach(function (value, key) {
      if (value === '' && key !== 'q') return;
      u.searchParams.set(key, String(value));
    });
    if (!u.searchParams.get('view')) u.searchParams.set('view', 'store');
    if (storeSource() && form.classList.contains('domain-picker')) {
      var domain = String(fd.get('domain') || '');
      if (domain) u.searchParams.set('domain', domain);
      syncUrl(u.pathname + u.search, false);
      document.querySelectorAll('#plugin-store-source input[name="domain"]').forEach(function (inp) {
        inp.value = domain;
      });
      applyStoreFilter(true);
      return;
    }
    loadHub(u.pathname + u.search, { replace: false });
  }, true);

  document.addEventListener('change', function (ev) {
    var el = ev.target;
    if (!el || !el.closest || !el.closest('#plugins-hub')) return;
    if (el.id === 'domain' && el.form) {
      ev.preventDefault();
      var fd = new FormData(el.form);
      var domain = String(fd.get('domain') || '');
      if (storeSource()) {
        var u = new URL(window.location.href);
        if (domain) u.searchParams.set('domain', domain);
        u.searchParams.set('target', 'site');
        syncUrl(u.pathname + u.search, false);
        document.querySelectorAll('#plugin-store-source input[name="domain"]').forEach(function (inp) {
          inp.value = domain;
        });
        applyStoreFilter(true);
        return;
      }
      var nu = new URL('/plugins', window.location.origin);
      fd.forEach(function (value, key) { nu.searchParams.set(key, String(value)); });
      loadHub(nu.pathname + nu.search, { replace: false });
      return;
    }
    if (el.id === 'plugin-per-page') {
      var per = el.value || '4';
      if (storeSource()) {
        var pu = new URL(window.location.href);
        pu.searchParams.set('per_page', per);
        pu.searchParams.set('page', '1');
        syncUrl(pu.pathname + pu.search, false);
        applyStoreFilter(true);
        return;
      }
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
    if (storeSource()) {
      var u = new URL(window.location.href);
      u.searchParams.set('mode', mode);
      if (mode === 'scroll') u.searchParams.delete('page');
      else u.searchParams.set('page', '1');
      syncUrl(u.pathname + u.search, false);
      applyStoreFilter(true);
      return;
    }
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
    if (storeSource()) {
      var u = new URL(window.location.href);
      u.searchParams.set('page', page);
      u.searchParams.set('mode', 'page');
      syncUrl(u.pathname + u.search, false);
      applyStoreFilter(true);
      return;
    }
    loadHub(buildStoreUrl({ page: page, mode: 'page' }), { replace: false });
  }, true);

  document.addEventListener('submit', function (ev) {
    var form = ev.target;
    if (!form || form.id !== 'plugin-goto-form') return;
    ev.preventDefault();
    var input = form.querySelector('[name="goto_page"]');
    var page = input ? parseInt(input.value, 10) : 1;
    if (!page || page < 1) page = 1;
    if (storeSource()) {
      var u = new URL(window.location.href);
      u.searchParams.set('page', String(page));
      u.searchParams.set('mode', 'page');
      syncUrl(u.pathname + u.search, false);
      applyStoreFilter(true);
      return;
    }
    loadHub(buildStoreUrl({ page: page, mode: 'page' }), { replace: false });
  }, true);

  window.addEventListener('popstate', function () {
    if (window.location.pathname !== '/plugins') return;
    if (storeSource() && applyStoreFilter(true)) return;
    loadHub(window.location.pathname + window.location.search, { replace: true });
  });

  applyStoreFilter(true);
})();
</script>
"#
    .to_string()
}
