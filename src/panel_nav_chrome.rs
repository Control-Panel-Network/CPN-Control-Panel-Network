//! Sidebar collapse (all breakpoints) and drawer nav chrome for CPN Panel.

/// Extra CSS: brand-row collapse control + drawer mode when user hides the sidebar.
pub fn sidebar_collapse_styles() -> &'static str {
    r#"
.sidebar-brand-row {
  display:flex; align-items:center; justify-content:space-between; gap:6px; min-width:0;
}
.sidebar-brand-row .panel-brand { flex:1 1 auto; min-width:0; }
.sidebar-collapse-btn {
  flex:0 0 auto; width:40px; height:40px; color:var(--muted);
}
.sidebar-collapse-btn:hover { background:#eeeeF0; color:var(--ink); }
.sidebar-collapse-btn svg { display:block; }
@media (max-width:1023.98px) {
  .sidebar-collapse-btn { display:none; }
}
/* User-collapsed: same overlay drawer as narrow screens, on any viewport */
body.sidebar-collapsed.nav-open { overflow:hidden; }
body.sidebar-collapsed .sidebar-backdrop { display:none; }
body.sidebar-collapsed.nav-open .sidebar-backdrop { display:block; }
body.sidebar-collapsed .sidebar {
  position:fixed; left:0; top:0; height:100%; height:100dvh;
  max-height:100%; max-height:100dvh; transform:translateX(-105%);
  transition:transform 180ms ease; box-shadow:none; flex:none;
}
body.sidebar-collapsed.nav-open .sidebar {
  transform:translateX(0); box-shadow:12px 0 32px rgba(0,0,0,.12);
}
body.sidebar-collapsed .panel-main { padding:0 20px 64px; width:100%; }
body.sidebar-collapsed .mobile-header {
  height:58px; margin:0 -20px 28px; padding:0 12px 0 8px; display:flex; align-items:center;
  justify-content:space-between; gap:12px; position:sticky; top:0; z-index:30;
  background:rgba(250,250,252,.94); border-bottom:1px solid var(--hairline);
}
body.sidebar-collapsed .mobile-header strong { flex:1; font-size:16px; }
[data-color-mode="dark"] .sidebar-collapse-btn:hover { background:rgba(255,255,255,.08); }
"#
}

/// Hamburger open/close + persist sidebar collapsed preference (`cpn-sidebar-collapsed`).
pub fn panel_nav_script() -> &'static str {
    r#"
<script>
(function () {
  var STORAGE_KEY = 'cpn-sidebar-collapsed';
  var body = document.body;
  var toggle = document.getElementById('nav-toggle');
  var backdrop = document.getElementById('nav-backdrop');
  var sidebar = document.getElementById('panel-sidebar');
  if (!toggle || !sidebar) return;

  function isNarrow() {
    return window.matchMedia('(max-width: 1023.98px)').matches;
  }
  function isDrawerMode() {
    return body.classList.contains('sidebar-collapsed') || isNarrow();
  }
  function readCollapsed() {
    try { return localStorage.getItem(STORAGE_KEY) === '1'; } catch (e) { return false; }
  }
  function writeCollapsed(collapsed) {
    try { localStorage.setItem(STORAGE_KEY, collapsed ? '1' : '0'); } catch (e) {}
  }

  function ensureCollapseButton() {
    var existing = document.getElementById('sidebar-collapse-btn');
    if (existing) return existing;
    var header = sidebar.querySelector('.sidebar-header');
    if (!header) return null;
    var brand = header.querySelector('.panel-brand');
    if (!brand) return null;
    var row = brand.closest('.sidebar-brand-row');
    if (!row) {
      row = document.createElement('div');
      row.className = 'sidebar-brand-row';
      brand.parentNode.insertBefore(row, brand);
      row.appendChild(brand);
    }
    var btn = document.createElement('button');
    btn.type = 'button';
    btn.id = 'sidebar-collapse-btn';
    btn.className = 'icon-btn sidebar-collapse-btn';
    btn.setAttribute('aria-controls', 'panel-sidebar');
    btn.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2"></rect><path d="M9 4v16"></path><path d="m15 9-3 3 3 3"></path></svg>';
    row.appendChild(btn);
    return btn;
  }

  var collapseBtn = ensureCollapseButton();

  function setCollapsed(collapsed) {
    body.classList.toggle('sidebar-collapsed', collapsed);
    writeCollapsed(collapsed);
    if (collapseBtn) {
      collapseBtn.setAttribute('aria-pressed', collapsed ? 'true' : 'false');
      collapseBtn.setAttribute('aria-label', collapsed ? 'Show sidebar' : 'Hide sidebar');
      collapseBtn.setAttribute('title', collapsed ? 'Show sidebar' : 'Hide sidebar');
    }
    if (!isDrawerMode()) {
      setOpen(false);
    } else if (!body.classList.contains('nav-open')) {
      sidebar.setAttribute('aria-hidden', 'true');
    }
  }

  function setOpen(open) {
    body.classList.toggle('nav-open', open);
    toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
    sidebar.setAttribute('aria-hidden', open ? 'false' : String(isDrawerMode()));
    if (open) {
      var first = sidebar.querySelector('a, button');
      if (first) first.focus();
    } else if (isDrawerMode()) {
      toggle.focus();
    }
  }

  function syncLayout() {
    if (!isDrawerMode()) {
      body.classList.remove('nav-open');
      toggle.setAttribute('aria-expanded', 'false');
      sidebar.setAttribute('aria-hidden', 'false');
    } else if (!body.classList.contains('nav-open')) {
      sidebar.setAttribute('aria-hidden', 'true');
    }
  }

  if (collapseBtn) {
    collapseBtn.addEventListener('click', function () {
      var next = !body.classList.contains('sidebar-collapsed');
      setCollapsed(next);
      if (next) {
        setOpen(false);
      } else {
        setOpen(false);
        syncLayout();
      }
    });
  }

  toggle.addEventListener('click', function () {
    setOpen(!body.classList.contains('nav-open'));
  });
  if (backdrop) {
    backdrop.addEventListener('click', function () { setOpen(false); });
  }
  document.addEventListener('keydown', function (event) {
    if (event.key === 'Escape' && body.classList.contains('nav-open')) {
      setOpen(false);
    }
  });
  window.addEventListener('resize', syncLayout);

  setCollapsed(readCollapsed());
  syncLayout();
})();
</script>
"#
}

#[cfg(test)]
mod tests {
    use super::{panel_nav_script, sidebar_collapse_styles};

    #[test]
    fn collapse_styles_cover_drawer_and_scroll_contract() {
        let css = sidebar_collapse_styles();
        assert!(css.contains("sidebar-collapsed"));
        assert!(
            css.contains("cpn-sidebar-collapsed")
                || panel_nav_script().contains("cpn-sidebar-collapsed")
        );
        assert!(css.contains("mobile-header"));
        assert!(css.contains("sidebar-collapse-btn"));
    }

    #[test]
    fn nav_script_persists_preference() {
        let js = panel_nav_script();
        assert!(js.contains("cpn-sidebar-collapsed"));
        assert!(js.contains("sidebar-collapse-btn"));
        assert!(js.contains("localStorage"));
    }
}
