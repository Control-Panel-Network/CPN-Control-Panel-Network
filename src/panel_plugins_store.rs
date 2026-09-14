//! Plugin Store catalog list: cards, Featured filter, release/updated dates.

use crate::panel_plugins_markup::{html_escape, urlencoding_simple};
use crate::panel_plugins_spa::{
    list_mode_from_query, page_from_query, per_page_from_query, store_list_toolbar,
};
use crate::plugins::{CatalogEntry, catalog_entry_is_featured, format_iso_date_eu};

pub(crate) struct StoreListOpts<'a> {
    pub query: &'a str,
    pub category: &'a str,
    pub domain: &'a str,
    pub mode: &'a str,
    pub page: usize,
    pub per_page: usize,
}

fn badge_pricing(pricing: &str) -> String {
    let lower = pricing.to_ascii_lowercase();
    if lower.contains("paid") || lower.contains("premium") {
        r#"<span class="plugin-badge paid">Paid</span>"#.into()
    } else {
        r#"<span class="plugin-badge free">Free</span>"#.into()
    }
}

pub(crate) fn store_catalog(
    entries: &[CatalogEntry],
    installed_ids: &[String],
    opts: StoreListOpts<'_>,
) -> String {
    let q = opts.query.trim().to_ascii_lowercase();
    let cat = opts.category.trim().to_ascii_lowercase();
    let filtered: Vec<&CatalogEntry> = entries
        .iter()
        .filter(|entry| {
            let cat_ok = if cat.is_empty() || cat == "all" {
                true
            } else if cat == "featured" {
                catalog_entry_is_featured(entry, entries)
            } else {
                entry.category.to_ascii_lowercase() == cat
            };
            if !cat_ok {
                return false;
            }
            if q.is_empty() {
                return true;
            }
            entry.name.to_ascii_lowercase().contains(&q)
                || entry.description.to_ascii_lowercase().contains(&q)
                || entry.id.to_ascii_lowercase().contains(&q)
        })
        .collect();
    let total = filtered.len();
    if total == 0 {
        return format!(
            r#"{toolbar}<p class="empty-state">No plugins match this search.</p>"#,
            toolbar = store_list_toolbar(opts.mode, opts.per_page, 1, 1, 0),
        );
    }
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
    let page_items = &filtered[start..end];
    let mut cards = String::from(r#"<div class="plugin-grid">"#);
    for entry in page_items {
        cards.push_str(&store_card(entry, entries, installed_ids, opts.domain));
    }
    cards.push_str("</div>");
    let scroll_cls = if mode == "scroll" {
        "plugin-grid-scroll is-scroll"
    } else {
        "plugin-grid-scroll"
    };
    format!(
        r#"{toolbar}<div class="{scroll_cls}">{cards}</div>"#,
        toolbar = store_list_toolbar(mode, opts.per_page.max(4), page, total_pages, total),
        scroll_cls = scroll_cls,
        cards = cards,
    )
}

fn store_card(
    entry: &CatalogEntry,
    all: &[CatalogEntry],
    installed_ids: &[String],
    domain: &str,
) -> String {
    let installed = installed_ids.iter().any(|id| id == &entry.id);
    let featured = catalog_entry_is_featured(entry, all);
    let featured_badge = if featured {
        r#"<span class="plugin-badge featured">Featured</span>"#
    } else {
        ""
    };
    let dates = dates_line(&entry.released_on, &entry.updated_on);
    let action = if installed {
        r#"<span class="plugin-badge installed">Installed</span>"#.to_string()
    } else if domain.is_empty() {
        r#"<span class="muted">Select a domain to install</span>"#.to_string()
    } else {
        format!(
            r#"<form method="post" action="/plugins/install" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-primary">Install</button>
          </form>"#,
            id = html_escape(&entry.id),
            domain = html_escape(domain),
        )
    };
    format!(
        r#"<article class="plugin-card">
          <h3>{name}</h3>
          <div class="plugin-badges">
            <span class="plugin-badge cat">{cat}</span>
            <span class="plugin-badge">v{ver}</span>
            {pricing}
            {featured}
          </div>
          <p class="plugin-desc">{desc}</p>
          <p class="plugin-meta">Author: {author}</p>
          {dates}
          <div class="plugin-actions">{action}</div>
        </article>"#,
        name = html_escape(&entry.name),
        desc = html_escape(&entry.description),
        cat = html_escape(&entry.category),
        ver = html_escape(&entry.version),
        pricing = badge_pricing(&entry.pricing),
        featured = featured_badge,
        author = html_escape(&entry.author),
        dates = dates,
        action = action,
    )
}

fn dates_line(released_on: &str, updated_on: &str) -> String {
    let mut parts = Vec::new();
    if !released_on.trim().is_empty() {
        parts.push(format!(
            "Released: {}",
            format_iso_date_eu(released_on.trim())
        ));
    }
    if !updated_on.trim().is_empty() {
        parts.push(format!(
            "Updated: {}",
            format_iso_date_eu(updated_on.trim())
        ));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!(
        r#"<p class="plugin-dates">{}</p>"#,
        html_escape(&parts.join(" · "))
    )
}

pub(crate) fn category_pills(
    entries: &[CatalogEntry],
    active: &str,
    domain: &str,
    mode: &str,
    per_page: usize,
) -> String {
    let mut cats: Vec<String> = entries.iter().map(|e| e.category.clone()).collect();
    cats.sort_by_key(|c| c.to_ascii_lowercase());
    cats.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    let mut domain_q = format!("&amp;domain={}", urlencoding_simple(domain));
    domain_q.push_str(&format!(
        "&amp;mode={}&amp;per_page={}",
        urlencoding_simple(mode),
        per_page
    ));
    let mut out = String::from(r#"<div class="category-pills">"#);
    out.push_str(&format!(
        r#"<a class="{cls}" href="/plugins?view=store{domain_q}">All categories</a>"#,
        cls = if active.is_empty() || active.eq_ignore_ascii_case("all") {
            "active"
        } else {
            ""
        },
        domain_q = domain_q,
    ));
    out.push_str(&format!(
        r#"<a class="{cls}" href="/plugins?view=store&amp;category=Featured{domain_q}">Featured</a>"#,
        cls = if active.eq_ignore_ascii_case("featured") {
            "active"
        } else {
            ""
        },
        domain_q = domain_q,
    ));
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
