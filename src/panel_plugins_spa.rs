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
    crate::panel_plugins_spa_script::plugins_hub_script()
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
