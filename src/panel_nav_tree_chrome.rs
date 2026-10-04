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
  display:block;
  margin:14px 0 8px;
  padding:0;
}
.nav-section-label {
  display:block;
  width:100%;
  box-sizing:border-box;
  padding:7px 12px;
  border-radius:8px;
  background:#2563eb;
  color:#fff;
  font-size:11px; font-weight:700;
  letter-spacing:.08em; text-transform:uppercase;
  line-height:1.2;
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
  padding:8px 12px; border-radius:10px; color:#1d1d1f;
  font-size:14px; font-weight:600; line-height:1.25; user-select:none;
  background:#fff; border:1px solid #e0e0e0;
  box-shadow:0 1px 2px rgba(29,29,31,.05);
}
.sidebar nav a.nav-tile > span:not(.nav-icon),
.nav-parent.nav-tile > span:not(.nav-icon) {
  flex:1 1 auto; min-width:0;
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
}
.sidebar nav a.nav-tile:hover,
.nav-parent.nav-tile:hover {
  border-color:#c9d8ef; background:#f8fbff; color:#0066cc;
}
.sidebar nav a.nav-tile.active,
.nav-parent-active {
  border-color:#9ec2f0; background:#e7f1ff; color:#0066cc;
}
.nav-parent .nav-chevron,
.sidebar nav a.nav-tile .nav-chevron {
  margin-left:auto; flex:0 0 auto; color:#6e6e73;
  transition:transform .18s ease;
}
.nav-group[open] > .nav-parent .nav-chevron { transform:rotate(90deg); color:#0066cc; }
.nav-children {
  display:flex;
  flex-direction:column;
  gap:6px;
  margin:8px 0 2px;
  padding:0 0 0 8px;
}
.nav-child-btn {
  display:flex; align-items:center; min-height:40px; width:100%; min-width:0; padding:8px 12px;
  border-radius:10px; background:#fff; border:1px solid #e0e0e0;
  color:#1d1d1f; font-size:13px; font-weight:500;
  box-shadow:0 1px 2px rgba(29,29,31,.04);
}
.nav-child-btn span {
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
}
.nav-child-btn:hover { border-color:#c9d8ef; background:#f8fbff; color:#0066cc; }
.nav-child-btn.active {
  border-color:#9ec2f0; background:#e7f1ff; color:#0066cc; font-weight:600;
}
.sidebar nav a.nav-child { padding-left:22px; font-size:14px; min-height:40px; }
html[data-color-mode="dark"] .nav-section-label,
[data-color-mode="dark"] .nav-section-label {
  background:#1e3a5f;
  color:#93c5fd;
  border:1px solid #2a4a73;
}
html[data-color-mode="dark"] .sidebar nav a.nav-tile,
html[data-color-mode="dark"] .nav-parent.nav-tile,
html[data-color-mode="dark"] .nav-child-btn,
[data-color-mode="dark"] .sidebar nav a.nav-tile,
[data-color-mode="dark"] .nav-parent.nav-tile,
[data-color-mode="dark"] .nav-child-btn {
  background:#1c212b; border-color:#2a3140; color:#e5e7eb;
  box-shadow:none;
}
html[data-color-mode="dark"] .sidebar nav a.nav-tile:hover,
html[data-color-mode="dark"] .nav-parent.nav-tile:hover,
html[data-color-mode="dark"] .nav-child-btn:hover,
[data-color-mode="dark"] .sidebar nav a.nav-tile:hover,
[data-color-mode="dark"] .nav-parent.nav-tile:hover,
[data-color-mode="dark"] .nav-child-btn:hover {
  background:#232a36; border-color:#3b82f6; color:#93c5fd;
}
html[data-color-mode="dark"] .sidebar nav a.nav-tile.active,
html[data-color-mode="dark"] .nav-parent-active,
html[data-color-mode="dark"] .nav-child-btn.active,
[data-color-mode="dark"] .sidebar nav a.nav-tile.active,
[data-color-mode="dark"] .nav-parent-active,
[data-color-mode="dark"] .nav-child-btn.active {
  background:rgba(59,130,246,.2); border-color:#3b82f6; color:#93c5fd;
}
html[data-color-mode="dark"] .nav-parent .nav-chevron,
html[data-color-mode="dark"] .sidebar nav a.nav-tile .nav-chevron,
[data-color-mode="dark"] .nav-parent .nav-chevron,
[data-color-mode="dark"] .sidebar nav a.nav-tile .nav-chevron { color:#9ca3af; }
"#
}

/// Last-in style block: keep sidebar nav tile background + label colors paired
/// for both personal color modes, even when a Theme Store package sets --ink.
pub fn sidebar_nav_contrast_styles() -> &'static str {
    r#"
/* Sidebar nav contrast lock (must follow design/theme CSS). */
html[data-color-mode="light"] body .sidebar nav a.nav-tile,
html[data-color-mode="light"] body .sidebar .nav-parent.nav-tile,
html[data-color-mode="light"] body .sidebar .nav-child-btn,
html[data-color-mode="light"] .sidebar nav a.nav-tile,
html[data-color-mode="light"] .sidebar .nav-parent.nav-tile,
html[data-color-mode="light"] .sidebar .nav-child-btn {
  background:#fff;
  border-color:#e0e0e0;
  color:#1d1d1f;
  box-shadow:0 1px 2px rgba(29,29,31,.05);
}
html[data-color-mode="light"] body .sidebar nav a.nav-tile:hover,
html[data-color-mode="light"] body .sidebar .nav-parent.nav-tile:hover,
html[data-color-mode="light"] body .sidebar .nav-child-btn:hover,
html[data-color-mode="light"] .sidebar nav a.nav-tile:hover,
html[data-color-mode="light"] .sidebar .nav-parent.nav-tile:hover,
html[data-color-mode="light"] .sidebar .nav-child-btn:hover {
  background:#f8fbff;
  border-color:#c9d8ef;
  color:#0066cc;
}
html[data-color-mode="light"] body .sidebar nav a.nav-tile.active,
html[data-color-mode="light"] body .sidebar .nav-parent-active,
html[data-color-mode="light"] body .sidebar .nav-child-btn.active,
html[data-color-mode="light"] .sidebar nav a.nav-tile.active,
html[data-color-mode="light"] .sidebar .nav-parent-active,
html[data-color-mode="light"] .sidebar .nav-child-btn.active {
  background:#e7f1ff;
  border-color:#9ec2f0;
  color:#0066cc;
}
html[data-color-mode="light"] body .sidebar .nav-parent .nav-chevron,
html[data-color-mode="light"] body .sidebar nav a.nav-tile .nav-chevron,
html[data-color-mode="light"] .sidebar .nav-parent .nav-chevron,
html[data-color-mode="light"] .sidebar nav a.nav-tile .nav-chevron {
  color:#6e6e73;
}
html[data-color-mode="light"] body .sidebar nav a,
html[data-color-mode="light"] .sidebar nav a {
  color:#1d1d1f;
}
html[data-color-mode="dark"] body .sidebar nav a.nav-tile,
html[data-color-mode="dark"] body .sidebar .nav-parent.nav-tile,
html[data-color-mode="dark"] body .sidebar .nav-child-btn,
html[data-color-mode="dark"] .sidebar nav a.nav-tile,
html[data-color-mode="dark"] .sidebar .nav-parent.nav-tile,
html[data-color-mode="dark"] .sidebar .nav-child-btn {
  background:#1c212b;
  border-color:#2a3140;
  color:#e5e7eb;
  box-shadow:none;
}
html[data-color-mode="dark"] body .sidebar nav a.nav-tile:hover,
html[data-color-mode="dark"] body .sidebar .nav-parent.nav-tile:hover,
html[data-color-mode="dark"] body .sidebar .nav-child-btn:hover,
html[data-color-mode="dark"] .sidebar nav a.nav-tile:hover,
html[data-color-mode="dark"] .sidebar .nav-parent.nav-tile:hover,
html[data-color-mode="dark"] .sidebar .nav-child-btn:hover {
  background:#232a36;
  border-color:#3b82f6;
  color:#93c5fd;
}
html[data-color-mode="dark"] body .sidebar nav a.nav-tile.active,
html[data-color-mode="dark"] body .sidebar .nav-parent-active,
html[data-color-mode="dark"] body .sidebar .nav-child-btn.active,
html[data-color-mode="dark"] .sidebar nav a.nav-tile.active,
html[data-color-mode="dark"] .sidebar .nav-parent-active,
html[data-color-mode="dark"] .sidebar .nav-child-btn.active {
  background:rgba(59,130,246,.2);
  border-color:#3b82f6;
  color:#93c5fd;
}
html[data-color-mode="dark"] body .sidebar .nav-parent .nav-chevron,
html[data-color-mode="dark"] body .sidebar nav a.nav-tile .nav-chevron,
html[data-color-mode="dark"] .sidebar .nav-parent .nav-chevron,
html[data-color-mode="dark"] .sidebar nav a.nav-tile .nav-chevron {
  color:#9ca3af;
}
html[data-color-mode="dark"] body .sidebar nav a,
html[data-color-mode="dark"] .sidebar nav a {
  color:#c5cad3;
}
html[data-color-mode="dark"] body .sidebar nav a.active,
html[data-color-mode="dark"] .sidebar nav a.active {
  color:var(--blue, #93c5fd);
}
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
