//! Client-side panel navigation without full document reloads (issue #617).

/// Styles for SPA loading / live region.
pub fn spa_nav_styles() -> &'static str {
    r#"
.cpn-spa-loading .panel-main { opacity:.55; pointer-events:none; transition:opacity .15s ease; }
.cpn-spa-announce { position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden; clip:rect(0,0,0,0); border:0; }
body.cpn-panel-fade-in { animation: cpnPanelFadeIn .28s ease; }
@keyframes cpnPanelFadeIn { from { opacity:0; } to { opacity:1; } }
@media (prefers-reduced-motion: reduce) {
  .cpn-spa-loading .panel-main { transition:none; }
  body.cpn-panel-fade-in { animation:none; }
}
"#
}

/// Intercepts same-origin sidebar/primary nav links; swaps `.panel-main` via fetch.
pub fn spa_nav_script() -> &'static str {
    r#"
<script>
(function () {
  var layout = document.querySelector('.panel-layout');
  var main = document.querySelector('.panel-main');
  if (!layout || !main) return;

  try {
    if (sessionStorage.getItem('cpn-login-fade') === '1') {
      sessionStorage.removeItem('cpn-login-fade');
      document.body.classList.add('cpn-panel-fade-in');
      setTimeout(function () { document.body.classList.remove('cpn-panel-fade-in'); }, 400);
    }
  } catch (_e) {}

  var announce = document.getElementById('cpn-spa-announce');
  if (!announce) {
    announce = document.createElement('div');
    announce.id = 'cpn-spa-announce';
    announce.className = 'cpn-spa-announce';
    announce.setAttribute('aria-live', 'polite');
    announce.setAttribute('aria-atomic', 'true');
    document.body.appendChild(announce);
  }

  var ctrl = null;
  var navigating = false;

  function sameOrigin(href) {
    try {
      var u = new URL(href, location.origin);
      return u.origin === location.origin;
    } catch (_e) { return false; }
  }

  function shouldIntercept(a) {
    if (!a || a.target === '_blank' || a.hasAttribute('download')) return false;
    var href = a.getAttribute('href') || '';
    if (!href || href.charAt(0) === '#' || href.indexOf('javascript:') === 0) return false;
    if (!sameOrigin(href)) return false;
    var path = new URL(href, location.origin).pathname;
    if (path === '/logout' || path.indexOf('/logout') === 0) return false;
    if (path.indexOf('/api/') === 0) return false;
    if (a.closest && a.closest('[data-cpn-no-spa]')) return false;
    return true;
  }

  function runScripts(root) {
    var scripts = Array.prototype.slice.call(root.querySelectorAll('script'));
    scripts.forEach(function (old) {
      var s = document.createElement('script');
      Array.prototype.forEach.call(old.attributes || [], function (attr) {
        s.setAttribute(attr.name, attr.value);
      });
      s.text = old.textContent || '';
      old.parentNode.replaceChild(s, old);
    });
  }

  function syncNav(doc) {
    var nextNav = doc.querySelector('aside.sidebar nav');
    var curNav = document.querySelector('aside.sidebar nav');
    if (nextNav && curNav) curNav.innerHTML = nextNav.innerHTML;
    var nextLayout = doc.querySelector('.panel-layout');
    if (nextLayout && layout) {
      var page = nextLayout.getAttribute('data-page');
      if (page) layout.setAttribute('data-page', page);
    }
  }

  function focusMain() {
    var heading = main.querySelector('h1, h2, [tabindex="-1"]');
    if (heading) {
      if (!heading.hasAttribute('tabindex')) heading.setAttribute('tabindex', '-1');
      try { heading.focus({ preventScroll: false }); } catch (_e) {
        try { heading.focus(); } catch (_e2) {}
      }
    } else {
      main.setAttribute('tabindex', '-1');
      try { main.focus(); } catch (_e) {}
    }
  }

  async function navigate(url, push) {
    if (navigating) {
      if (ctrl) try { ctrl.abort(); } catch (_e) {}
    }
    navigating = true;
    document.body.classList.add('cpn-spa-loading');
    if (announce) announce.textContent = 'Loading page';
    ctrl = new AbortController();
    try {
      var res = await fetch(url, {
        credentials: 'same-origin',
        headers: { 'Accept': 'text/html', 'X-CPN-SPA': '1' },
        signal: ctrl.signal
      });
      if (res.status === 401 || res.redirected && (res.url || '').indexOf('/login') >= 0) {
        location.assign(res.url || '/login');
        return;
      }
      if (!res.ok) {
        location.assign(url);
        return;
      }
      var html = await res.text();
      var doc = new DOMParser().parseFromString(html, 'text/html');
      var nextMain = doc.querySelector('.panel-main');
      if (!nextMain) {
        location.assign(url);
        return;
      }
      main.innerHTML = nextMain.innerHTML;
      runScripts(main);
      syncNav(doc);
      if (doc.title) document.title = doc.title;
      if (push) history.pushState({ cpnSpa: 1 }, '', url);
      var titleText = (document.title || '').split('·')[0].trim() || 'Page';
      if (announce) announce.textContent = titleText + ' loaded';
      focusMain();
      try { window.dispatchEvent(new CustomEvent('cpn:spa-navigated', { detail: { url: url } })); } catch (_e) {}
    } catch (e) {
      if (e && e.name === 'AbortError') return;
      location.assign(url);
    } finally {
      navigating = false;
      document.body.classList.remove('cpn-spa-loading');
    }
  }

  document.addEventListener('click', function (ev) {
    if (ev.defaultPrevented) return;
    if (ev.button !== 0 || ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey) return;
    var a = ev.target && ev.target.closest ? ev.target.closest('a[href]') : null;
    if (!shouldIntercept(a)) return;
    var inChrome = a.closest('aside.sidebar') || a.closest('.mobile-header') || a.closest('.panel-main');
    if (!inChrome) return;
    // Keep full reload for destructive/confirm flows that post via GET rarely; allow SPA for normal tabs.
    if (a.getAttribute('data-cpn-full-nav') === '1') return;
    ev.preventDefault();
    navigate(a.href, true);
  }, true);

  window.addEventListener('popstate', function () {
    navigate(location.href, false);
  });
})();
</script>
"#
}
