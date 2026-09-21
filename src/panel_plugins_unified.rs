//! One Store catalog: host packages + community plugins in a single filtered grid.

use crate::apps::{AppStatus, list_apps};
use crate::host_packages_catalog::{filter_host_packages, host_categories};
use crate::panel_admin::is_panel_admin;
use crate::panel_apps::host_card;
use crate::panel_plugins_markup::{html_escape, urlencoding_simple};
use crate::panel_plugins_spa::{
    list_mode_from_query, page_from_query, per_page_from_query, store_list_toolbar,
};
use crate::panel_plugins_store::{StoreListOpts, filter_store_entries, render_catalog_card};
use crate::plugin_activation::catalog_entry_is_host_scoped;
use crate::plugins::CatalogEntry;

enum UnifiedItem<'a> {
    Host(&'a AppStatus),
    Catalog(&'a CatalogEntry),
}

fn sort_key(item: &UnifiedItem<'_>) -> String {
    match item {
        UnifiedItem::Host(s) => s.id.label().to_ascii_lowercase(),
        UnifiedItem::Catalog(e) => e.name.to_ascii_lowercase(),
    }
}

fn skip_catalog_duplicate(entry: &CatalogEntry) -> bool {
    // Roundcube is the Email host package (cpn app), not a second catalog install.
    entry.id.eq_ignore_ascii_case("roundcubeWebmail")
        || entry.id.eq_ignore_ascii_case("roundcube")
}

fn collect_unified<'a>(
    apps: &'a [AppStatus],
    entries: &'a [CatalogEntry],
    query: &str,
    category: &str,
) -> Vec<UnifiedItem<'a>> {
    let cat = category.trim().to_ascii_lowercase();
    let mut out: Vec<UnifiedItem<'a>> = Vec::new();

    let host_cat = if cat == "host" { "" } else { category };
    let include_host = cat.is_empty()
        || cat == "all"
        || cat == "host"
        || cat == "featured"
        || cat == "paid"
        || cat == "free"
        || host_categories(apps)
            .iter()
            .any(|c| c.eq_ignore_ascii_case(category.trim()));

    if include_host {
        for status in filter_host_packages(apps, query, host_cat) {
            out.push(UnifiedItem::Host(status));
        }
    }

    let include_catalog = cat != "host";
    if include_catalog {
        let catalog_cat = if cat == "host" { "___none___" } else { category };
        for entry in filter_store_entries(entries, query, catalog_cat) {
            if skip_catalog_duplicate(entry) {
                continue;
            }
            // Category Host already excluded; Featured/Paid handled in filter_store_entries.
            // When filtering a host-only category name that catalogs also use, keep both.
            if cat == "host" {
                continue;
            }
            out.push(UnifiedItem::Catalog(entry));
        }
    } else {
        // Host pill: also include host-scoped community Security packages from catalog.
        for entry in entries {
            if skip_catalog_duplicate(entry) {
                continue;
            }
            if !catalog_entry_is_host_scoped(entry) {
                continue;
            }
            if !filter_store_entries(std::slice::from_ref(entry), query, "").is_empty() {
                out.push(UnifiedItem::Catalog(entry));
            }
        }
    }

    out.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    out
}

pub(crate) fn unified_category_pills(
    apps: &[AppStatus],
    entries: &[CatalogEntry],
    active: &str,
    domain: &str,
    mode: &str,
    per_page: usize,
    q: &str,
) -> String {
    let mut cats: Vec<String> = host_categories(apps)
        .iter()
        .map(|c| (*c).to_string())
        .collect();
    for e in entries {
        cats.push(e.category.clone());
    }
    cats.sort_by_key(|c| c.to_ascii_lowercase());
    cats.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

    let mut domain_q = format!("&amp;domain={}", urlencoding_simple(domain));
    domain_q.push_str(&format!(
        "&amp;mode={}&amp;per_page={}",
        urlencoding_simple(mode),
        per_page
    ));
    if !q.trim().is_empty() {
        domain_q.push_str(&format!("&amp;q={}", urlencoding_simple(q)));
    }
    let mut out = String::from(r#"<div class="category-pills">"#);
    for (label, cat_val) in [
        ("All", ""),
        ("Featured", "Featured"),
        ("Host", "Host"),
        ("Paid", "Paid"),
    ] {
        let cls = if cat_val.is_empty() {
            if active.is_empty() || active.eq_ignore_ascii_case("all") {
                "active"
            } else {
                ""
            }
        } else if active.eq_ignore_ascii_case(cat_val) {
            "active"
        } else {
            ""
        };
        let href = if cat_val.is_empty() {
            format!("/plugins?view=store{domain_q}")
        } else {
            format!(
                "/plugins?view=store&amp;category={enc}{domain_q}",
                enc = urlencoding_simple(cat_val)
            )
        };
        out.push_str(&format!(
            r#"<a class="{cls}" href="{href}">{label}</a>"#,
            cls = cls,
            href = href,
            label = label,
        ));
    }
    for cat in cats {
        let cls = if cat.eq_ignore_ascii_case(active) {
            "active"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<a class="{cls}" href="/plugins?view=store&amp;category={enc}{domain_q}">{label}</a>"#,
            cls = cls,
            enc = urlencoding_simple(&cat),
            domain_q = domain_q,
            label = html_escape(&cat),
        ));
    }
    out.push_str("</div>");
    out
}

pub(crate) fn unified_store_catalog(
    entries: &[CatalogEntry],
    installed_ids: &[String],
    opts: StoreListOpts<'_>,
) -> String {
    let apps = list_apps();
    let items = collect_unified(&apps, entries, opts.query, opts.category);
    let total = items.len();
    let mode = list_mode_from_query(opts.mode);
    let per_page = if mode == "scroll" {
        total.max(1)
    } else {
        per_page_from_query(&opts.per_page.to_string())
    };
    let total_pages = if mode == "scroll" {
        1
    } else {
        total.div_ceil(per_page).max(1)
    };
    let page = page_from_query(&opts.page.to_string()).min(total_pages);
    let start = if mode == "scroll" {
        0
    } else {
        (page - 1) * per_page
    };
    let end = if mode == "scroll" {
        total
    } else {
        (start + per_page).min(total)
    };
    let toolbar = store_list_toolbar(mode, per_page.max(4), page, total_pages, total);
    if total == 0 {
        return format!(
            r#"{toolbar}<p class="empty-state">No packages match this search.</p>"#,
            toolbar = toolbar,
        );
    }
    let admin = is_panel_admin(opts.username);
    let mut cards = String::from(r#"<div class="plugin-grid">"#);
    for item in &items[start..end] {
        match item {
            UnifiedItem::Host(status) => {
                cards.push_str(&host_card(status, opts.domain, &apps, admin));
            }
            UnifiedItem::Catalog(entry) => {
                cards.push_str(&render_catalog_card(
                    entry,
                    entries,
                    installed_ids,
                    opts.domain,
                    opts.username,
                ));
            }
        }
    }
    cards.push_str("</div>");
    let scroll_cls = if mode == "scroll" {
        "plugin-grid-scroll is-scroll"
    } else {
        "plugin-grid-scroll"
    };
    format!(
        r#"{toolbar}<div class="{scroll_cls}">{cards}</div>"#,
        toolbar = toolbar,
        scroll_cls = scroll_cls,
        cards = cards,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::{AppId, AppStateKind};

    fn stub(id: AppId) -> AppStatus {
        AppStatus {
            id,
            state: AppStateKind::NotInstalled,
            detail: String::new(),
            warning: None,
        }
    }

    #[test]
    fn host_category_includes_host_apps() {
        let apps = vec![stub(AppId::Mariadb), stub(AppId::Phpmyadmin)];
        let entries: Vec<CatalogEntry> = Vec::new();
        let items = collect_unified(&apps, &entries, "", "Host");
        assert_eq!(items.len(), 2);
    }
}
