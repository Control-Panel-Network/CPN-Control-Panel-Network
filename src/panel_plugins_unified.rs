//! One Store catalog: host packages + community plugins in a single filtered grid.

use crate::apps::{list_apps, AppStatus};
use crate::host_packages_catalog::{
    filter_host_packages, host_categories, host_package_is_featured, meta_for,
};
use crate::panel_admin::is_panel_admin;
use crate::panel_apps::host_card;
use crate::panel_plugins_markup::{html_escape, store_scope_query_suffix, urlencoding_simple};
use crate::panel_plugins_spa::{
    list_mode_from_query, page_from_query, per_page_from_query, store_list_toolbar,
};
use crate::panel_plugins_store::{filter_store_entries, render_catalog_card, StoreListOpts};
use crate::plugin_activation::catalog_entry_is_host_scoped;
use crate::plugins::{catalog_entry_is_featured, CatalogEntry};

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
    let id = entry.id.as_str();
    id.eq_ignore_ascii_case("roundcubeWebmail")
        || id.eq_ignore_ascii_case("roundcube")
        || id.eq_ignore_ascii_case("snappymailWebmail")
        || id.eq_ignore_ascii_case("snappymailAdmin")
        || id.eq_ignore_ascii_case("snappymail")
        || id.eq_ignore_ascii_case("tachyon")
}

fn normalize_target(raw: &str) -> &str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "host" => "host",
        "all" | "both" => "all",
        _ => "site",
    }
}

fn include_host_for_category(apps: &[AppStatus], category: &str) -> bool {
    let cat = category.trim().to_ascii_lowercase();
    cat.is_empty()
        || cat == "all"
        || cat == "featured"
        || cat == "paid"
        || cat == "free"
        || host_categories(apps)
            .iter()
            .any(|c| c.eq_ignore_ascii_case(category.trim()))
}

fn collect_unified<'a>(
    apps: &'a [AppStatus],
    entries: &'a [CatalogEntry],
    query: &str,
    category: &str,
    target: &str,
) -> Vec<UnifiedItem<'a>> {
    let cat = category.trim().to_ascii_lowercase();
    let target = normalize_target(target);
    let mut out: Vec<UnifiedItem<'a>> = Vec::new();
    let host_cat = if cat == "host" { "" } else { category };

    if target != "site" && include_host_for_category(apps, host_cat) {
        for status in filter_host_packages(apps, query, host_cat) {
            out.push(UnifiedItem::Host(status));
        }
    }

    if target != "host" {
        for entry in filter_store_entries(entries, query, if cat == "host" { "" } else { category })
        {
            if skip_catalog_duplicate(entry) {
                continue;
            }
            if catalog_entry_is_host_scoped(entry) {
                continue;
            }
            out.push(UnifiedItem::Catalog(entry));
        }
    }

    if target != "site" {
        for entry in filter_store_entries(entries, query, if cat == "host" { "" } else { category })
        {
            if skip_catalog_duplicate(entry) {
                continue;
            }
            if !catalog_entry_is_host_scoped(entry) {
                continue;
            }
            out.push(UnifiedItem::Catalog(entry));
        }
    }

    out.sort_by_key(|a| sort_key(a));
    out
}

fn pill_link(label: &str, cat_val: &str, active: &str, domain_q: &str) -> String {
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
    format!(
        r#"<a class="{cls}" href="{href}" data-store-cat="{cat}">{label}</a>"#,
        cls = cls,
        href = href,
        cat = html_escape(cat_val),
        label = html_escape(label),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn unified_category_pills(
    apps: &[AppStatus],
    entries: &[CatalogEntry],
    active: &str,
    domain: &str,
    store_target: &str,
    mode: &str,
    per_page: usize,
    q: &str,
) -> String {
    let mut cats: Vec<String> = host_categories(apps)
        .iter()
        .map(|c| (*c).to_string())
        .collect();
    for e in entries {
        if skip_catalog_duplicate(e) {
            continue;
        }
        cats.push(e.category.clone());
    }
    cats.sort_by_key(|c| c.to_ascii_lowercase());
    cats.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

    let domain_q = store_scope_query_suffix(domain, store_target, mode, per_page, q);
    let mut out = String::from(
        r#"<div class="category-pills" role="tablist" aria-label="Store categories">"#,
    );
    for (label, cat_val) in [("All", ""), ("Featured", "Featured"), ("Paid", "Paid")] {
        out.push_str(&pill_link(label, cat_val, active, &domain_q));
    }
    for cat in cats {
        out.push_str(&pill_link(&cat, &cat, active, &domain_q));
    }
    out.push_str("</div>");
    out
}

fn attr_escape(value: &str) -> String {
    html_escape(value).replace('\n', " ").replace('\r', " ")
}

fn with_store_attrs(
    html: &str,
    target: &str,
    category: &str,
    featured: bool,
    paid: bool,
    search: &str,
) -> String {
    html.replacen(
        r#"<article class="plugin-card">"#,
        &format!(
            r#"<article class="plugin-card" data-store-target="{t}" data-cat="{c}" data-featured="{f}" data-paid="{p}" data-search="{s}">"#,
            t = attr_escape(target),
            c = attr_escape(category),
            f = if featured { "1" } else { "0" },
            p = if paid { "1" } else { "0" },
            s = attr_escape(search),
        ),
        1,
    )
}

fn render_item(
    item: &UnifiedItem<'_>,
    apps: &[AppStatus],
    entries: &[CatalogEntry],
    installed_ids: &[String],
    domain: &str,
    username: &str,
    admin: bool,
) -> String {
    match item {
        UnifiedItem::Host(status) => {
            let meta = meta_for(status.id);
            let paid = meta.pricing.to_ascii_lowercase().contains("paid")
                || meta.pricing.to_ascii_lowercase().contains("premium");
            let search = format!(
                "{} {} {} {}",
                status.id.label(),
                status.id.as_str(),
                meta.description,
                meta.category
            );
            with_store_attrs(
                &host_card(status, domain, apps, admin),
                "host",
                meta.category,
                host_package_is_featured(status.id, apps),
                paid,
                &search,
            )
        }
        UnifiedItem::Catalog(entry) => {
            let host = catalog_entry_is_host_scoped(entry);
            let paid = entry.pricing.to_ascii_lowercase().contains("paid")
                || entry.pricing.to_ascii_lowercase().contains("premium");
            let search = format!(
                "{} {} {} {}",
                entry.name, entry.id, entry.description, entry.category
            );
            with_store_attrs(
                &render_catalog_card(entry, entries, installed_ids, domain, username),
                if host { "host" } else { "site" },
                &entry.category,
                catalog_entry_is_featured(entry, entries),
                paid,
                &search,
            )
        }
    }
}

pub(crate) fn unified_store_catalog(
    entries: &[CatalogEntry],
    installed_ids: &[String],
    opts: StoreListOpts<'_>,
) -> String {
    let apps = list_apps();
    let admin = is_panel_admin(opts.username);
    let target = normalize_target(opts.store_target);
    let source_target = if admin { "all" } else { "site" };
    let source_items = collect_unified(&apps, entries, "", "", source_target);
    let items = collect_unified(&apps, entries, opts.query, opts.category, target);
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
    let mut source_html = String::new();
    for item in &source_items {
        source_html.push_str(&render_item(
            item,
            &apps,
            entries,
            installed_ids,
            opts.domain,
            opts.username,
            admin,
        ));
    }
    let mut cards = String::from(r#"<div class="plugin-grid">"#);
    if total > 0 {
        for item in &items[start..end] {
            cards.push_str(&render_item(
                item,
                &apps,
                entries,
                installed_ids,
                opts.domain,
                opts.username,
                admin,
            ));
        }
    }
    cards.push_str("</div>");
    let scroll_cls = if mode == "scroll" {
        "plugin-grid-scroll is-scroll"
    } else {
        "plugin-grid-scroll"
    };
    let empty_hidden = if total == 0 { "" } else { " hidden" };
    let grid_hidden = if total == 0 { " hidden" } else { "" };
    format!(
        r#"{toolbar}<div id="plugin-store-live"><p id="plugin-store-empty" class="empty-state"{empty_hidden}>No packages match this search or install target.</p><div class="{scroll_cls}{grid_hidden}">{cards}</div></div><div id="plugin-store-source" hidden>{source}</div>"#,
        toolbar = toolbar,
        empty_hidden = empty_hidden,
        scroll_cls = scroll_cls,
        grid_hidden = grid_hidden,
        cards = cards,
        source = source_html,
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

    fn site_entry(id: &str, category: &str) -> CatalogEntry {
        CatalogEntry {
            id: id.into(),
            name: id.into(),
            category: category.into(),
            version: "1.0.0".into(),
            description: "site plugin".into(),
            author: "master3395".into(),
            pricing: "free".into(),
            released_on: String::new(),
            updated_on: String::new(),
            install_count: 0,
            featured: false,
            uninstall_impacts: vec![],
            host_scoped: false,
        }
    }

    #[test]
    fn host_target_lists_host_apps_not_site_plugins() {
        let apps = vec![stub(AppId::Mariadb), stub(AppId::Tachyon)];
        let entries = vec![site_entry("bimi", "Email")];
        let items = collect_unified(&apps, &entries, "", "", "host");
        assert!(items
            .iter()
            .any(|i| matches!(i, UnifiedItem::Host(s) if s.id == AppId::Tachyon)));
        assert!(!items.iter().any(|i| matches!(i, UnifiedItem::Catalog(_))));
    }

    #[test]
    fn site_target_lists_site_plugins_not_host_apps() {
        let apps = vec![stub(AppId::Tachyon), stub(AppId::Email)];
        let entries = vec![site_entry("bimi", "Email")];
        let items = collect_unified(&apps, &entries, "", "Email", "site");
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], UnifiedItem::Catalog(e) if e.id == "bimi"));
    }

    #[test]
    fn email_host_includes_tachyon_and_snappymail() {
        let apps = vec![
            stub(AppId::Tachyon),
            stub(AppId::Snappymail),
            stub(AppId::Email),
            stub(AppId::Mariadb),
        ];
        let items = collect_unified(&apps, &[], "", "Email", "host");
        let ids: Vec<_> = items
            .iter()
            .filter_map(|i| match i {
                UnifiedItem::Host(s) => Some(s.id),
                _ => None,
            })
            .collect();
        assert!(ids.contains(&AppId::Tachyon));
        assert!(ids.contains(&AppId::Snappymail));
        assert!(ids.contains(&AppId::Email));
        assert!(!ids.contains(&AppId::Mariadb));
    }

    #[test]
    fn category_pills_omit_host_chip() {
        let apps = vec![stub(AppId::Email)];
        let html = unified_category_pills(&apps, &[], "", "", "site", "page", 4, "mail");
        assert!(html.contains("data-store-cat=\"Email\""));
        assert!(html.contains("data-store-cat=\"Featured\""));
        assert!(!html.contains("data-store-cat=\"Host\""));
        assert!(html.contains("q=mail"));
        assert!(html.contains("target=site"));
    }

    #[test]
    fn skip_community_webmail_duplicates() {
        assert!(skip_catalog_duplicate(&site_entry(
            "snappymailWebmail",
            "Email"
        )));
        assert!(skip_catalog_duplicate(&site_entry("tachyon", "Email")));
        assert!(!skip_catalog_duplicate(&site_entry("bimi", "Email")));
    }
}
