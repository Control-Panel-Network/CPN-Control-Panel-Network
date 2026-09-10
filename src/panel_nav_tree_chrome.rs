//! CSS and client script for expandable sidebar nav groups.

/// CSS for single-column nav tiles, expandable parents, and stacked child buttons.
pub fn nav_tree_styles() -> &'static str {
    r#"
.nav-tile-grid {
  display:flex;
  flex-direction:column;
  gap:8px;
  margin:0 0 10px;
}
.nav-section {
  margin:14px 4px 8px;
  color:var(--muted); font-size:11px; font-weight:700;
  letter-spacing:.08em; text-transform:uppercase;
}
.sidebar nav > .nav-section:first-child { margin-top:4px; }
.nav-group { margin:0; border:0; min-width:0; width:100%; }
.nav-group > summary {
  list-style:none; cursor:pointer;
}
.nav-group > summary::-webkit-details-marker { display:none; }
.sidebar nav a.nav-tile,
.nav-parent.nav-tile {
  display:flex; align-items:center; gap:10px; min-height:44px; width:100%; min-width:0;
  padding:8px 12px; border-radius:10px; color:var(--ink);
  font-size:14px; font-weight:600; line-height:1.25; user-select:none;
  background:#fff; border:1px solid var(--hairline);
  box-shadow:0 1px 2px rgba(29,29,31,.05);
}
.sidebar nav a.nav-tile > span:not(.nav-icon),
.nav-parent.nav-tile > span:not(.nav-icon) {
  flex:1 1 auto; min-width:0;
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
}
.sidebar nav a.nav-tile:hover,
.nav-parent.nav-tile:hover {
  border-color:#c9d8ef; background:#f8fbff; color:var(--blue);
}
.sidebar nav a.nav-tile.active,
.nav-parent-active {
  border-color:#9ec2f0; background:#e7f1ff; color:var(--blue);
}
.nav-parent .nav-chevron,
.sidebar nav a.nav-tile .nav-chevron {
  margin-left:auto; flex:0 0 auto; color:var(--muted);
  transition:transform .18s ease;
}
.nav-group[open] > .nav-parent .nav-chevron { transform:rotate(90deg); color:var(--blue); }
.nav-children {
  display:flex;
  flex-direction:column;
  gap:6px;
  margin:8px 0 2px;
  padding:0 0 0 8px;
}
.nav-child-btn {
  display:flex; align-items:center; min-height:40px; width:100%; min-width:0; padding:8px 12px;
  border-radius:10px; background:#fff; border:1px solid var(--hairline);
  color:var(--ink); font-size:13px; font-weight:500;
  box-shadow:0 1px 2px rgba(29,29,31,.04);
}
.nav-child-btn span {
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
}
.nav-child-btn:hover { border-color:#c9d8ef; background:#f8fbff; color:var(--blue); }
.nav-child-btn.active {
  border-color:#9ec2f0; background:#e7f1ff; color:var(--blue); font-weight:600;
}
.sidebar nav a.nav-child { padding-left:22px; font-size:14px; min-height:40px; }
[data-color-mode="dark"] .sidebar nav a.nav-tile,
[data-color-mode="dark"] .nav-parent.nav-tile,
[data-color-mode="dark"] .nav-child-btn {
  background:#1c212b; border-color:#2a3140; color:#e5e7eb;
  box-shadow:none;
}
[data-color-mode="dark"] .sidebar nav a.nav-tile:hover,
[data-color-mode="dark"] .nav-parent.nav-tile:hover,
[data-color-mode="dark"] .nav-child-btn:hover {
  background:#232a36; border-color:#3b82f6; color:#93c5fd;
}
[data-color-mode="dark"] .sidebar nav a.nav-tile.active,
[data-color-mode="dark"] .nav-parent-active,
[data-color-mode="dark"] .nav-child-btn.active {
  background:rgba(59,130,246,.2); border-color:#3b82f6; color:#93c5fd;
}
[data-color-mode="dark"] .nav-parent .nav-chevron,
[data-color-mode="dark"] .sidebar nav a.nav-tile .nav-chevron { color:#9ca3af; }
"#
}

/// Highlight the child button that best matches the current path.
pub fn nav_tree_script() -> &'static str {
    r#"
<script>
(function () {
  var path = window.location.pathname || '/';
  var best = null;
  var bestLen = -1;
  document.querySelectorAll('a.nav-child-btn[href]').forEach(function (a) {
    var href = a.getAttribute('href') || '';
    if (!href || href.charAt(0) !== '/') return;
    var exact = path === href;
    var prefix = href !== '/' && (path === href || path.indexOf(href + '/') === 0);
    if (!exact && !prefix) return;
    if (href.length > bestLen) {
      best = a;
      bestLen = href.length;
    }
  });
  if (best) {
    best.classList.add('active');
    var group = best.closest('details.nav-group');
    if (group) group.open = true;
  }
})();
</script>
"#
}
