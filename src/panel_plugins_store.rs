//! Plugin Store catalog list: cards, Featured/Paid filters, release/updated dates.

use crate::panel_admin::is_panel_admin;
use crate::panel_plugins_markup::{html_escape, urlencoding_simple};
use crate::panel_plugins_spa::{
    list_mode_from_query, page_from_query, per_page_from_query, store_list_toolbar,
};
use crate::plugin_activation::{catalog_entry_is_host_scoped, host_plugin_installed, is_activated};
use crate::plugins::{CatalogEntry, catalog_entry_is_featured, format_iso_date_eu};

pub(crate) struct StoreListOpts<'a> {
    pub query: &'a str,
    pub category: &'a str,
    pub domain: &'a str,
    pub mode: &'a str,
    pub page: usize,
    pub per_page: usize,
    /// Signed-in panel username (RBAC for Host Install vs Activate).
    pub username: &'a str,
}

fn pricing_is_paid(pricing: &str) -> bool {
    let lower = pricing.to_ascii_lowercase();
    lower.contains("paid") || lower.contains("premium")
}

/// Exact `q=paid` / `q=free` (and `premium`) act as pricing filters, not full-text.
fn exact_pricing_query(query: &str) -> Option<&'static str> {
    match query.trim().to_ascii_lowercase().as_str() {
        "paid" | "premium" => Some("paid"),
        "free" => Some("free"),
        _ => None,
    }
}

fn badge_pricing(pricing: &str) -> String {
    if pricing_is_paid(pricing) {
        r#"<span class="plugin-badge paid">Paid</span>"#.into()
    } else {
        r#"<span class="plugin-badge free">Free</span>"#.into()
    }
}

pub(crate) fn filter_store_entries<'a>(
    entries: &'a [CatalogEntry],
    query: &str,
    category: &str,
) -> Vec<&'a CatalogEntry> {
    let q = query.trim().to_ascii_lowercase();
    let cat = category.trim().to_ascii_lowercase();
    let pricing_q = exact_pricing_query(&q);
    entries
        .iter()
        .filter(|entry| {
            let cat_ok = if cat.is_empty() || cat == "all" {
                true
            } else if cat == "featured" {
                catalog_entry_is_featured(entry, entries)
            } else if cat == "paid" {
                pricing_is_paid(&entry.pricing)
            } else if cat == "free" {
                !pricing_is_paid(&entry.pricing)
            } else {
                entry.category.to_ascii_lowercase() == cat
            };
            if !cat_ok {
                return false;
            }
            if q.is_empty() {
                return true;
            }
            if let Some(want) = pricing_q {
                return match want {
                    "paid" => pricing_is_paid(&entry.pricing),
                    "free" => !pricing_is_paid(&entry.pricing),
                    _ => false,
                };
            }
            entry.name.to_ascii_lowercase().contains(&q)
                || entry.description.to_ascii_lowercase().contains(&q)
                || entry.id.to_ascii_lowercase().contains(&q)
        })
        .collect()
}

/// Legacy Store catalog markup (hub UI uses [`crate::panel_plugins_unified::unified_store_catalog`]).
#[allow(dead_code)]
pub(crate) fn store_catalog(
    entries: &[CatalogEntry],
    installed_ids: &[String],
    opts: StoreListOpts<'_>,
) -> String {
    let filtered = filter_store_entries(entries, opts.query, opts.category);
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
        cards.push_str(&store_card(
            entry,
            entries,
            installed_ids,
            opts.domain,
            opts.username,
        ));
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

pub(crate) fn render_catalog_card(
    entry: &CatalogEntry,
    all: &[CatalogEntry],
    installed_ids: &[String],
    domain: &str,
    username: &str,
) -> String {
    let installed = installed_ids.iter().any(|id| id == &entry.id);
    let host_scoped = catalog_entry_is_host_scoped(entry);
    let on_host = host_scoped && host_plugin_installed(&entry.id);
    let activated = !domain.is_empty() && is_activated(domain, &entry.id);
    let admin = is_panel_admin(username);
    let featured = catalog_entry_is_featured(entry, all);
    let featured_badge = if featured {
        r#"<span class="plugin-badge featured">Featured</span>"#
    } else {
        ""
    };
    let host_badge = if host_scoped {
        r#"<span class="plugin-badge">Host</span>"#
    } else {
        r#"<span class="plugin-badge cat">Site</span>"#
    };
    let dates = dates_line(&entry.released_on, &entry.updated_on);
    let action = store_action_html(
        entry,
        domain,
        installed,
        host_scoped,
        on_host,
        activated,
        admin,
    );
    format!(
        r#"<article class="plugin-card">
          <h3>{name}</h3>
          <div class="plugin-badges">
            {host}
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
        host = host_badge,
        author = html_escape(&entry.author),
        dates = dates,
        action = action,
    )
}

#[allow(dead_code)] // called from legacy store_catalog
fn store_card(
    entry: &CatalogEntry,
    all: &[CatalogEntry],
    installed_ids: &[String],
    domain: &str,
    username: &str,
) -> String {
    render_catalog_card(entry, all, installed_ids, domain, username)
}

fn store_action_html(
    entry: &CatalogEntry,
    domain: &str,
    installed: bool,
    host_scoped: bool,
    on_host: bool,
    activated: bool,
    admin: bool,
) -> String {
    let id = html_escape(&entry.id);
    let domain_e = html_escape(domain);
    // Roundcube is a Host package (cpn app install); jump to that card in this Store.
    if entry.id.eq_ignore_ascii_case("roundcubeWebmail")
        || entry.id.eq_ignore_ascii_case("roundcube")
    {
        let domain_q = if domain.is_empty() {
            String::new()
        } else {
            format!("&amp;domain={}", urlencoding_simple(domain))
        };
        return format!(
            r#"<span class="muted">Host package</span>
            <a class="btn-primary" href="/plugins?view=store&amp;category=Host{domain_q}&amp;q=roundcube">Show Roundcube</a>"#,
            domain_q = domain_q,
        );
    }
    if host_scoped {
        if on_host {
            if domain.is_empty() {
                if admin {
                    return format!(
                        r#"<span class="plugin-badge installed">Installed on Host</span>
            <form method="post" action="/plugins/uninstall-host" class="inline-form" onsubmit="return confirm('Uninstall host plugin {name}? This removes it for all sites.');">
              <input type="hidden" name="id" value="{id}">
              <button type="submit" class="btn-danger">Uninstall from Host</button>
            </form>"#,
                        name = html_escape(&entry.name),
                        id = id,
                    );
                }
                return r#"<span class="plugin-badge installed">Installed on Host</span>"#.into();
            }
            if activated || installed {
                return format!(
                    r#"<span class="plugin-badge installed">Activated</span>
            <form method="post" action="/plugins/deactivate-host" class="inline-form">
              <input type="hidden" name="id" value="{id}">
              <input type="hidden" name="domain" value="{domain}">
              <button type="submit" class="btn-warn">Deactivate</button>
            </form>"#,
                    id = id,
                    domain = domain_e,
                );
            }
            return format!(
                r#"<form method="post" action="/plugins/activate-host" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-primary">Activate</button>
          </form>"#,
                id = id,
                domain = domain_e,
            );
        }
        // Not on host yet.
        if admin {
            let domain_field = if domain.is_empty() {
                String::new()
            } else {
                format!(r#"<input type="hidden" name="domain" value="{domain}">"#)
            };
            let activate_note = if domain.is_empty() {
                ""
            } else {
                " (then activate for this site)"
            };
            return format!(
                r#"<form method="post" action="/plugins/install-host" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            {domain_field}
            <button type="submit" class="btn-primary">Install on Host{note}</button>
          </form>"#,
                id = id,
                domain_field = domain_field,
                note = activate_note,
            );
        }
        return r#"<span class="muted">Ask the panel admin to Install on Host first</span>"#.into();
    }
    if installed {
        return r#"<span class="plugin-badge installed">Installed</span>"#.into();
    }
    if domain.is_empty() {
        return r#"<span class="muted">Select a domain to install</span>"#.into();
    }
    format!(
        r#"<form method="post" action="/plugins/install" class="inline-form">
            <input type="hidden" name="id" value="{id}">
            <input type="hidden" name="domain" value="{domain}">
            <button type="submit" class="btn-primary">Install</button>
          </form>"#,
        id = id,
        domain = domain_e,
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

/// Legacy category pills (hub UI uses [`crate::panel_plugins_unified::unified_category_pills`]).
#[allow(dead_code)]
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
    out.push_str(&format!(
        r#"<a class="{cls}" href="/plugins?view=store&amp;category=Host{domain_q}">Host</a>"#,
        cls = if active.eq_ignore_ascii_case("host") {
            "active"
        } else {
            ""
        },
        domain_q = domain_q,
    ));
    out.push_str(&format!(
        r#"<a class="{cls}" href="/plugins?view=store&amp;category=Paid{domain_q}">Paid</a>"#,
        cls = if active.eq_ignore_ascii_case("paid") {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, pricing: &str, description: &str) -> CatalogEntry {
        CatalogEntry {
            id: id.into(),
            name: id.into(),
            category: "Security".into(),
            version: "1.0.0".into(),
            description: description.into(),
            author: "master3395".into(),
            pricing: pricing.into(),
            released_on: String::new(),
            updated_on: String::new(),
            install_count: 0,
            featured: false,
            uninstall_impacts: vec![],
            host_scoped: false,
        }
    }

    #[test]
    fn q_paid_filters_by_pricing_not_description() {
        let entries = vec![
            entry(
                "clamav",
                "free",
                "Free malware scanner. No third-party paid brands.",
            ),
            entry("commerce", "paid", "Paid CPN hosting commerce."),
            entry("malwareApi", "paid", "News Targeted malware API."),
        ];
        let paid = filter_store_entries(&entries, "paid", "");
        let ids: Vec<&str> = paid.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["commerce", "malwareApi"]);
        assert!(!ids.contains(&"clamav"));
    }

    #[test]
    fn category_paid_shows_only_paid() {
        let entries = vec![
            entry("clamav", "free", "mentions paid in description"),
            entry("commerce", "paid", "commerce"),
        ];
        let paid = filter_store_entries(&entries, "", "Paid");
        assert_eq!(paid.len(), 1);
        assert_eq!(paid[0].id, "commerce");
    }

    #[test]
    fn q_free_filters_by_pricing() {
        let entries = vec![
            entry("clamav", "free", "scanner"),
            entry("commerce", "paid", "Paid plan"),
        ];
        let free = filter_store_entries(&entries, "FREE", "");
        assert_eq!(free.len(), 1);
        assert_eq!(free[0].id, "clamav");
    }

    #[test]
    fn longer_query_still_full_text() {
        let entries = vec![
            entry("clamav", "free", "No third-party paid brands."),
            entry("commerce", "paid", "Paid CPN hosting commerce."),
        ];
        let hits = filter_store_entries(&entries, "paid brands", "");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "clamav");
    }

    #[test]
    fn paid_pill_present_in_markup() {
        let entries = vec![entry("commerce", "paid", "commerce")];
        let html = category_pills(&entries, "Paid", "example.com", "page", 4);
        assert!(html.contains("category=Paid"));
        assert!(html.contains(">Paid</a>"));
        assert!(html.contains(r#"class="active""#) || html.contains("class=\"active\""));
    }
}
