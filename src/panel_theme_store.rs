//! Theme Store / Installed UI for Settings > Design (`?tab=store|installed`).

use crate::panel_admin::is_panel_admin;

/// Theme catalog panel. `view` is `"store"` (available) or `"installed"`.
pub fn themes_catalog_panel(username: &str, view: &str) -> String {
    let installed_view = view == "installed";
    let can_edit = is_panel_admin(username);
    let title = if installed_view {
        "Installed themes"
    } else {
        "Theme Store"
    };
    let blurb = if installed_view {
        "Themes already on this panel. Apply sets panel-wide chrome. Uninstall removes the package from storage."
    } else {
        "Free themes from CPN-Themes on GitHub. Paid themes from the News Targeted CPN shop category unlock via Shop Grants or activation key, then Install."
    };
    let refresh = if installed_view {
        String::new()
    } else {
        r#"<button type="button" class="manage-btn" id="cpn-themes-refresh">Refresh catalog</button>"#
            .to_string()
    };
    let redeem_row = if installed_view {
        String::new()
    } else {
        r#"<div class="cpn-theme-redeem-row" id="cpn-theme-redeem-row">
    <input type="email" id="cpn-theme-license-email" placeholder="License email (Shop Grant)" autocomplete="email">
    <input type="text" id="cpn-theme-activation-key" placeholder="Activation key" autocomplete="off">
    <input type="text" id="cpn-theme-redeem-id" placeholder="Theme id (e.g. obsidian-pro)" autocomplete="off">
    <button type="button" class="btn-primary" id="cpn-theme-redeem-btn">Redeem</button>
  </div>"#
        .to_string()
    };
    let catalog_links = if installed_view {
        r#"Catalog: <a href="https://github.com/Control-Panel-Network/CPN-Themes" target="_blank" rel="noopener noreferrer">CPN-Themes</a>"#.to_string()
    } else {
        r#"Free: <a href="https://github.com/Control-Panel-Network/CPN-Themes" target="_blank" rel="noopener noreferrer">CPN-Themes</a>. Paid: <a href="https://shop.newstargeted.com/shop/catalog?category=cpn" target="_blank" rel="noopener noreferrer">Shop CPN</a> / <a href="https://newstargeted.com/store/products/category/cpn" target="_blank" rel="noopener noreferrer">Store CPN</a>. Grants: <a href="https://api.newstargeted.com/admin/shop-grants" target="_blank" rel="noopener noreferrer">Shop Grants</a>"#.to_string()
    };
    let search_ph = if installed_view {
        "Search installed themes by name or description..."
    } else {
        "Search themes by name or description..."
    };
    format!(
        r##"<article class="section-card cpn-themes-catalog" id="cpn-themes-catalog" data-view="{view}">
  <header class="cpn-themes-head">
    <div>
      <h2>{title}</h2>
      <p class="plugin-store-meta">{catalog_links}</p>
      <p class="plugin-store-meta">{blurb}</p>
    </div>
    {refresh}
  </header>
  {redeem_row}
  <div class="plugin-search-row">
    <label for="cpn-themes-q">Search themes</label>
    <input class="plugin-search" id="cpn-themes-q" type="search" placeholder="{search_ph}" autocomplete="off">
    <button type="button" class="btn-primary" id="cpn-themes-search-btn">Search</button>
    <button type="button" class="btn-secondary" id="cpn-themes-clear-btn">Clear</button>
  </div>
  <div class="plugin-list-toolbar" id="cpn-themes-toolbar" role="group" aria-label="Theme list display">
    <div class="plugin-mode-switch" role="group" aria-label="List mode">
      <button type="button" class="plugin-mode-btn active" data-theme-mode="page">Pagination</button>
      <button type="button" class="plugin-mode-btn" data-theme-mode="scroll">Scrollbar</button>
    </div>
    <span class="muted" id="cpn-themes-match-count">0 matching</span>
    <div class="plugin-pager" id="cpn-themes-pager">
      <label for="cpn-themes-per-page">Show</label>
      <select id="cpn-themes-per-page" aria-label="Themes per page">
        <option value="4">4</option>
        <option value="8" selected>8</option>
        <option value="12">12</option>
        <option value="16">16</option>
        <option value="24">24</option>
      </select>
      <span>per page</span>
      <span class="page-status" id="cpn-themes-page-status">Page 1 / 1</span>
      <button type="button" class="btn-secondary" id="cpn-themes-prev" disabled>Prev</button>
      <button type="button" class="btn-secondary" id="cpn-themes-next" disabled>Next</button>
      <label for="cpn-themes-goto">Go to page</label>
      <input id="cpn-themes-goto" type="number" min="1" value="1" style="width:4rem;">
      <button type="button" class="btn-primary" id="cpn-themes-goto-btn">Go</button>
    </div>
  </div>
  <p id="cpn-themes-status" class="plugin-store-meta" role="status">Loading themes...</p>
  <div id="cpn-themes-grid" class="plugin-grid" aria-live="polite"></div>
</article>
<style>
.cpn-themes-catalog {{ margin-top:0; }}
.cpn-themes-head {{ display:flex; flex-wrap:wrap; gap:12px; align-items:flex-start; justify-content:space-between; margin-bottom:12px; }}
.cpn-themes-catalog h2 {{ margin:0 0 6px; font-size:18px; color:var(--ink); }}
.cpn-theme-swatch {{
  width:100%; height:120px; border-radius:12px; border:1px solid var(--hairline);
  background:linear-gradient(90deg, var(--swatch-a, #2563eb), var(--swatch-b, #1d4ed8));
  background-size:cover; background-position:center; overflow:hidden;
}}
.cpn-theme-swatch img {{ display:block; width:100%; height:100%; object-fit:cover; }}
.cpn-themes-catalog .plugin-grid {{
  display:grid; grid-template-columns:repeat(auto-fill,minmax(240px,1fr)); gap:16px; margin-top:10px;
}}
.cpn-themes-catalog.scroll-mode .plugin-grid {{
  max-height:70vh; overflow:auto; padding-right:4px;
}}
.cpn-themes-catalog .plugin-card {{
  display:flex; flex-direction:column; gap:10px; padding:16px;
  border:1px solid var(--hairline); border-radius:16px; background:var(--canvas); color:var(--ink);
}}
.cpn-themes-catalog .plugin-card h3 {{ margin:0; font-size:16px; color:var(--ink); }}
.cpn-themes-catalog .plugin-desc {{ margin:0; font-size:14px; line-height:1.45; color:var(--ink); opacity:.9; }}
.cpn-themes-catalog .plugin-meta {{ margin:0; font-size:13px; color:var(--ink); opacity:.8; }}
.cpn-themes-catalog .plugin-badges {{ display:flex; flex-wrap:wrap; gap:6px; }}
.cpn-themes-catalog .plugin-badge {{
  display:inline-flex; align-items:center; min-height:24px; padding:0 8px; border-radius:999px;
  font-size:11px; font-weight:700; background:#dbeafe; color:#1e3a8a;
}}
.cpn-themes-catalog .plugin-badge.installed {{ background:#d1fae5; color:#065f46; }}
.cpn-themes-catalog .plugin-badge.active {{ background:#fef3c7; color:#92400e; }}
.cpn-themes-catalog .plugin-badge.available {{ background:#e0e7ff; color:#3730a3; }}
.cpn-themes-catalog .plugin-badge.update {{ background:#ffe4e6; color:#9f1239; }}
.cpn-themes-catalog .plugin-badge.paid {{ background:#ede9fe; color:#4c1d95; }}
.cpn-themes-catalog .plugin-badge.locked {{ background:#fee2e2; color:#991b1b; }}
.cpn-themes-catalog .plugin-badge.entitled {{ background:#dbeafe; color:#1e3a8a; }}
.cpn-themes-catalog .plugin-card.is-locked {{ opacity:.92; }}
.cpn-themes-catalog .cpn-theme-redeem-row {{
  display:flex; flex-wrap:wrap; gap:8px; margin:10px 0 4px; align-items:center;
}}
.cpn-themes-catalog .cpn-theme-redeem-row input {{
  min-height:36px; min-width:160px; flex:1; padding:0 10px; border-radius:10px;
  border:1px solid var(--hairline); background:var(--canvas); color:var(--ink); font:inherit;
}}
.cpn-themes-catalog .plugin-actions {{ margin-top:auto; display:flex; flex-wrap:wrap; gap:8px; }}
.cpn-themes-catalog .plugin-store-meta {{ color:var(--ink); font-size:.92rem; }}
.cpn-themes-catalog .plugin-store-meta a {{ color:var(--blue); font-weight:600; }}
.cpn-themes-catalog .plugin-search-row {{
  display:flex; flex-wrap:wrap; gap:10px; align-items:center; margin:12px 0 10px;
}}
.cpn-themes-catalog .plugin-search-row label {{
  position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden;
  clip:rect(0,0,0,0); white-space:nowrap; border:0;
}}
.cpn-themes-catalog .plugin-search {{
  flex:1 1 240px; min-width:180px; max-width:480px; box-sizing:border-box;
  border:1px solid #94a3b8; border-radius:10px; padding:10px 12px; font:inherit;
  color:var(--ink); background:var(--canvas);
}}
.cpn-themes-catalog .plugin-list-toolbar {{
  display:flex; flex-wrap:wrap; gap:12px; align-items:center; margin:0 0 12px;
}}
.cpn-themes-catalog .plugin-mode-switch {{ display:inline-flex; flex-wrap:wrap; gap:6px; }}
.cpn-themes-catalog .plugin-mode-btn {{
  min-height:36px; padding:0 12px; border-radius:999px; border:1px solid var(--hairline);
  background:var(--canvas); color:inherit; font:inherit; cursor:pointer;
}}
.cpn-themes-catalog .plugin-mode-btn.active {{ background:#e7f1ff; color:#0b3d91; border-color:#93c5fd; }}
.cpn-themes-catalog .plugin-pager {{ display:inline-flex; flex-wrap:wrap; gap:8px; align-items:center; }}
.cpn-themes-catalog .manage-btn, .cpn-themes-catalog .btn-primary, .cpn-themes-catalog .btn-secondary {{
  min-height:36px; padding:0 12px; border-radius:10px; border:1px solid var(--hairline);
  background:var(--canvas); color:inherit; font:inherit; cursor:pointer;
}}
.cpn-themes-catalog .btn-primary {{ background:var(--blue); color:#fff; border-color:transparent; }}
.cpn-themes-catalog .btn-secondary {{ background:#e2e8f0; color:#0f172a; }}
</style>
<script>
(function () {{
{helpers}
  var root = document.getElementById("cpn-themes-catalog");
  var statusEl = document.getElementById("cpn-themes-status");
  var grid = document.getElementById("cpn-themes-grid");
  var refreshBtn = document.getElementById("cpn-themes-refresh");
  var qInput = document.getElementById("cpn-themes-q");
  var searchBtn = document.getElementById("cpn-themes-search-btn");
  var clearBtn = document.getElementById("cpn-themes-clear-btn");
  var matchCount = document.getElementById("cpn-themes-match-count");
  var pager = document.getElementById("cpn-themes-pager");
  var perPageSel = document.getElementById("cpn-themes-per-page");
  var pageStatus = document.getElementById("cpn-themes-page-status");
  var prevBtn = document.getElementById("cpn-themes-prev");
  var nextBtn = document.getElementById("cpn-themes-next");
  var gotoInput = document.getElementById("cpn-themes-goto");
  var gotoBtn = document.getElementById("cpn-themes-goto-btn");
  var canEdit = {can_edit};
  var installedView = {installed_view};
  var allThemes = [];
  var metaLabel = "";
  var listMode = "page";
  var page = 1;
  if (!statusEl || !grid || !root) return;

  function esc(s) {{
    return String(s == null ? "" : s)
      .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  }}

  function badgeFor(t) {{
    var badges = '<span class="plugin-badge">v' + esc(t.version) + '</span>';
    var pricing = String(t.pricing || "free").toLowerCase();
    if (pricing === "paid" || pricing === "premium") {{
      badges += '<span class="plugin-badge paid">Paid</span>';
    }}
    if (t.locked) badges += '<span class="plugin-badge locked">Locked</span>';
    else if (t.entitled && (pricing === "paid" || pricing === "premium") && !t.installed) {{
      badges += '<span class="plugin-badge entitled">Entitled</span>';
    }}
    if (t.active) badges += '<span class="plugin-badge active">Active</span>';
    else if (t.update_available) badges += '<span class="plugin-badge update">Update</span>';
    else if (t.installed) badges += '<span class="plugin-badge installed">Installed</span>';
    else if (!t.locked) badges += '<span class="plugin-badge available">Available</span>';
    return badges;
  }}

  function actionsFor(t) {{
    if (!canEdit) return '<span class="plugin-badge">Admin only</span>';
    var id = esc(t.id);
    var html = "";
    var pricing = String(t.pricing || "free").toLowerCase();
    var isPaid = pricing === "paid" || pricing === "premium";
    if (!installedView && (t.locked || (isPaid && !t.entitled && !t.installed))) {{
      var buy = esc(t.purchase_url || "https://shop.newstargeted.com/shop/catalog?category=cpn");
      var store = esc(t.store_url || "https://newstargeted.com/store/products/category/cpn");
      html += '<a class="btn-primary" href="' + buy + '" target="_blank" rel="noopener noreferrer">Purchase</a>';
      html += '<a class="manage-btn" href="' + store + '" target="_blank" rel="noopener noreferrer">Store</a>';
      html += '<button type="button" class="manage-btn cpn-theme-fill-redeem" data-id="' + id + '">Redeem key</button>';
      return html;
    }}
    if (!t.installed) {{
      html += '<button type="button" class="btn-primary cpn-theme-install" data-id="' + id + '">Install</button>';
    }} else {{
      if (t.update_available) {{
        html += '<button type="button" class="btn-primary cpn-theme-install" data-id="' + id + '">Update</button>';
      }}
      if (!t.active) {{
        html += '<button type="button" class="btn-primary cpn-theme-apply" data-id="' + id + '">Apply</button>';
      }} else {{
        html += '<span class="plugin-badge active">In use</span>';
      }}
      html += '<button type="button" class="manage-btn cpn-theme-uninstall" data-id="' + id + '">Uninstall</button>';
    }}
    return html;
  }}

  function bindActions() {{
    grid.querySelectorAll(".cpn-theme-fill-redeem").forEach(function (btn) {{
      btn.addEventListener("click", function () {{
        var id = btn.getAttribute("data-id") || "";
        var idInput = document.getElementById("cpn-theme-redeem-id");
        if (idInput) idInput.value = id;
        var keyInput = document.getElementById("cpn-theme-activation-key");
        if (keyInput) keyInput.focus();
        statusEl.textContent = "Enter activation key for " + id + ", then Redeem.";
      }});
    }});
    grid.querySelectorAll(".cpn-theme-install").forEach(function (btn) {{
      btn.addEventListener("click", function () {{
        var id = btn.getAttribute("data-id");
        statusEl.textContent = "Installing " + id + "...";
        cpnDesignFetchJson("/api/panel/themes/install", {{
          method: "POST",
          headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
          body: JSON.stringify({{ id: id }})
        }}).then(function () {{
          if (!installedView) {{
            window.location.assign("/settings/design?tab=installed");
            return;
          }}
          load(false);
        }}).catch(function (err) {{
          statusEl.textContent = err.message || String(err);
          alert(err.message || String(err));
        }});
      }});
    }});
    grid.querySelectorAll(".cpn-theme-apply").forEach(function (btn) {{
      btn.addEventListener("click", function () {{
        var id = btn.getAttribute("data-id");
        statusEl.textContent = "Applying " + id + "...";
        cpnDesignFetchJson("/api/panel/themes/apply", {{
          method: "POST",
          headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
          body: JSON.stringify({{ id: id }})
        }}).then(function () {{
          window.location.assign("/settings/design?tab=installed");
        }}).catch(function (err) {{
          statusEl.textContent = err.message || String(err);
          alert(err.message || String(err));
        }});
      }});
    }});
    grid.querySelectorAll(".cpn-theme-uninstall").forEach(function (btn) {{
      btn.addEventListener("click", function () {{
        var id = btn.getAttribute("data-id");
        if (!window.confirm("Uninstall theme `" + id + "` from panel storage?")) return;
        statusEl.textContent = "Uninstalling " + id + "...";
        cpnDesignFetchJson("/api/panel/themes/uninstall", {{
          method: "POST",
          headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
          body: JSON.stringify({{ id: id }})
        }}).then(function () {{ load(false); }})
          .catch(function (err) {{
            statusEl.textContent = err.message || String(err);
            alert(err.message || String(err));
          }});
      }});
    }});
  }}

  function filteredThemes() {{
    var q = (qInput && qInput.value ? qInput.value : "").trim().toLowerCase();
    return allThemes.filter(function (t) {{
      var wantInstalled = !!installedView;
      if (!!t.installed !== wantInstalled) return false;
      if (!q) return true;
      var hay = [t.id, t.name, t.description, t.author].join(" ").toLowerCase();
      return hay.indexOf(q) !== -1;
    }});
  }}

  function cardHtml(t) {{
    var a = (t.tokens && t.tokens.accent) || "#2563eb";
    var b = (t.tokens && t.tokens.accent_focus) || a;
    var preview = (t.background && (t.background.preview || t.background.body)) || "";
    var previewUrl = t.preview_url || "";
    var swatchInner = previewUrl
      ? ('<img src="' + esc(previewUrl) + '" alt="' + esc(t.name) + ' preview" loading="lazy">')
      : "";
    var swatchStyle = previewUrl
      ? ""
      : (preview
        ? ('background:' + esc(preview) + ';')
        : ('--swatch-a:' + esc(a) + ';--swatch-b:' + esc(b) + ';'));
    var bgLabel = (t.preview_url || (t.background && t.background.image))
      ? " · Background image"
      : (t.background ? " · Background" : "");
    return '<article class="' + (t.locked ? "plugin-card is-locked" : "plugin-card") + '">' +
      '<div class="cpn-theme-swatch" style="' + swatchStyle + '">' + swatchInner + '</div>' +
      '<h3>' + esc(t.name) + '</h3>' +
      '<div class="plugin-badges">' + badgeFor(t) + '</div>' +
      '<p class="plugin-desc">' + esc(t.description) + '</p>' +
      '<p class="plugin-meta">Author: ' + esc(t.author) + bgLabel +
        (t.installed_version ? (' · Installed v' + esc(t.installed_version)) : '') + '</p>' +
      '<div class="plugin-actions">' + actionsFor(t) + '</div></article>';
  }}

  function render() {{
    var filtered = filteredThemes();
    var total = filtered.length;
    var perPage = perPageSel ? parseInt(perPageSel.value, 10) || 8 : 8;
    var totalPages = Math.max(1, Math.ceil(total / perPage) || 1);
    if (listMode === "scroll") {{
      page = 1;
      totalPages = 1;
    }}
    if (page > totalPages) page = totalPages;
    if (page < 1) page = 1;
    if (matchCount) matchCount.textContent = total + " matching";
    if (pageStatus) pageStatus.textContent = "Page " + page + " / " + totalPages;
    if (gotoInput) {{ gotoInput.value = String(page); gotoInput.max = String(totalPages); }}
    if (prevBtn) prevBtn.disabled = listMode === "scroll" || page <= 1;
    if (nextBtn) nextBtn.disabled = listMode === "scroll" || page >= totalPages;
    if (pager) pager.style.display = listMode === "scroll" ? "none" : "";
    root.classList.toggle("scroll-mode", listMode === "scroll");

    var slice = filtered;
    if (listMode === "page") {{
      var start = (page - 1) * perPage;
      slice = filtered.slice(start, start + perPage);
    }}

    if (!allThemes.length) {{
      statusEl.textContent = "No themes found in the catalog.";
      grid.innerHTML = "";
      return;
    }}
    if (!total) {{
      statusEl.textContent = installedView
        ? "No installed themes match this search."
        : "No available themes match this search.";
      grid.innerHTML = "";
      return;
    }}
    var available = allThemes.filter(function (t) {{ return !t.installed; }}).length;
    var installed = allThemes.filter(function (t) {{ return t.installed; }}).length;
    statusEl.textContent = (installedView ? "Installed" : "Store") + ": showing " + slice.length +
      " of " + total + " · catalog " + allThemes.length + " (" + installed + " installed, " +
      available + " available) - " + (metaLabel || "CPN-Themes");
    grid.innerHTML = slice.map(cardHtml).join("");
    bindActions();
  }}

  function load(force) {{
    statusEl.textContent = "Loading themes...";
    var url = "/api/panel/themes/catalog" + (force ? "?refresh=1" : "");
    cpnDesignFetchJson(url, {{ headers: {{ "Accept": "application/json" }} }})
      .then(function (data) {{
        allThemes = data.themes || [];
        metaLabel = (data.repo || "") + (data.fetched_at_local ? (" - " + data.fetched_at_local) : "");
        page = 1;
        render();
      }})
      .catch(function (err) {{
        statusEl.textContent = "Could not load themes: " + (err.message || String(err));
        grid.innerHTML = "";
      }});
  }}

  if (refreshBtn) refreshBtn.addEventListener("click", function () {{ load(true); }});
  var redeemBtn = document.getElementById("cpn-theme-redeem-btn");
  if (redeemBtn) {{
    redeemBtn.addEventListener("click", function () {{
      var themeId = (document.getElementById("cpn-theme-redeem-id") || {{}}).value || "";
      var key = (document.getElementById("cpn-theme-activation-key") || {{}}).value || "";
      var email = (document.getElementById("cpn-theme-license-email") || {{}}).value || "";
      statusEl.textContent = "Redeeming activation key...";
      cpnDesignFetchJson("/api/panel/themes/redeem", {{
        method: "POST",
        headers: {{ "Content-Type": "application/json", "Accept": "application/json" }},
        body: JSON.stringify({{ id: themeId, activation_key: key, license_email: email }})
      }}).then(function (data) {{
        statusEl.textContent = (data && data.message) ? data.message : "Theme unlocked.";
        load(false);
      }}).catch(function (err) {{
        statusEl.textContent = err.message || String(err);
        alert(err.message || String(err));
      }});
    }});
  }}
  if (searchBtn) searchBtn.addEventListener("click", function () {{ page = 1; render(); }});
  if (clearBtn) clearBtn.addEventListener("click", function () {{
    if (qInput) qInput.value = "";
    page = 1;
    render();
  }});
  if (qInput) qInput.addEventListener("keydown", function (ev) {{
    if (ev.key === "Enter") {{ ev.preventDefault(); page = 1; render(); }}
  }});
  if (perPageSel) perPageSel.addEventListener("change", function () {{ page = 1; render(); }});
  if (prevBtn) prevBtn.addEventListener("click", function () {{ if (page > 1) {{ page -= 1; render(); }} }});
  if (nextBtn) nextBtn.addEventListener("click", function () {{ page += 1; render(); }});
  if (gotoBtn) gotoBtn.addEventListener("click", function () {{
    var n = gotoInput ? parseInt(gotoInput.value, 10) : 1;
    if (!isNaN(n) && n >= 1) {{ page = n; render(); }}
  }});
  root.querySelectorAll("[data-theme-mode]").forEach(function (btn) {{
    btn.addEventListener("click", function () {{
      listMode = btn.getAttribute("data-theme-mode") === "scroll" ? "scroll" : "page";
      root.querySelectorAll("[data-theme-mode]").forEach(function (b) {{
        b.classList.toggle("active", b === btn);
      }});
      page = 1;
      render();
    }});
  }});
  load(false);
}})();
</script>"##,
        view = if installed_view { "installed" } else { "store" },
        title = title,
        blurb = blurb,
        catalog_links = catalog_links,
        redeem_row = redeem_row,
        refresh = refresh,
        search_ph = search_ph,
        can_edit = if can_edit { "true" } else { "false" },
        installed_view = if installed_view { "true" } else { "false" },
        helpers = crate::panel_minimalist_settings::design_fetch_js_helpers(),
    )
}
