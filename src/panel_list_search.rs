//! Shared GET `q=` list filters for Websites, Sub-domains, and WordPress lists.

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Trim and lowercase a search query (empty when blank).
pub fn normalize_list_q(raw: Option<&str>) -> String {
    raw.unwrap_or("")
        .trim()
        .chars()
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Case-insensitive substring match on domain and optional parent.
pub fn domain_matches_q(domain: &str, parent: Option<&str>, q: &str) -> bool {
    let q = q.trim().to_ascii_lowercase();
    if q.is_empty() {
        return true;
    }
    let domain_l = domain.to_ascii_lowercase();
    if domain_l.contains(&q) {
        return true;
    }
    parent
        .map(|p| p.to_ascii_lowercase().contains(&q))
        .unwrap_or(false)
}

/// Compact mobile-friendly GET search row (reuses plugin-search look).
pub fn list_search_form(action: &str, q: &str, placeholder: &str) -> String {
    list_search_form_with_extras(action, q, placeholder, "")
}

/// Search row with optional hidden fields (for example pagination `per_page` / `mode`).
pub fn list_search_form_with_extras(
    action: &str,
    q: &str,
    placeholder: &str,
    extras_html: &str,
) -> String {
    let clear = if q.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#" <a class="btn-secondary" href="{action}">Clear</a>"#,
            action = html_escape(action)
        )
    };
    format!(
        r#"<style>{css}</style>
<form method="get" action="{action}" class="plugin-search-row cpn-list-search" role="search">
  {extras}
  <label for="cpn-list-q">Search</label>
  <input class="plugin-search" id="cpn-list-q" name="q" type="search" value="{q}" placeholder="{ph}" maxlength="120" autocomplete="off" aria-label="{ph}">
  <button type="submit" class="btn-primary">Search</button>{clear}
</form>"#,
        css = list_search_styles(),
        action = html_escape(action),
        extras = extras_html,
        q = html_escape(q),
        ph = html_escape(placeholder),
        clear = clear,
    )
}

pub fn list_search_styles() -> &'static str {
    r#"
.cpn-list-search.plugin-search-row {
  display:flex; flex-wrap:wrap; gap:10px; align-items:center; margin:12px 0 10px; max-width:none;
}
.cpn-list-search.plugin-search-row label {
  position:absolute; width:1px; height:1px; padding:0; margin:-1px; overflow:hidden;
  clip:rect(0,0,0,0); white-space:nowrap; border:0;
}
.cpn-list-search .plugin-search {
  flex:1 1 220px; min-width:min(100%,180px); max-width:100%; box-sizing:border-box;
  border:1px solid #94a3b8; border-radius:10px; padding:10px 12px; font:inherit; margin:0;
  color:var(--ink); background:var(--canvas);
}
.cpn-list-search .btn-primary,
.cpn-list-search .btn-secondary { min-height:40px; }
@media (max-width:640px) {
  .cpn-list-search.plugin-search-row { flex-direction:column; align-items:stretch; }
  .cpn-list-search .plugin-search { max-width:none; width:100%; }
  .cpn-list-search .btn-primary,
  .cpn-list-search .btn-secondary { width:100%; justify-content:center; }
}
[data-color-mode="dark"] .cpn-list-search .plugin-search {
  background:#1a1d26; border-color:#475569; color:#f1f5f9;
}
"#
}

/// Muted line when a filter is active (for example "Showing 2 of 12 matching blog").
pub fn list_filter_summary(shown: usize, total: usize, q: &str) -> String {
    if q.trim().is_empty() || shown == total {
        return String::new();
    }
    format!(
        r#"<p class="muted" style="margin:0 0 10px;">Showing {shown} of {total} matching <strong>{q}</strong>.</p>"#,
        shown = shown,
        total = total,
        q = html_escape(q.trim()),
    )
}

#[cfg(test)]
mod tests {
    use super::{domain_matches_q, list_search_form, list_search_form_with_extras, normalize_list_q};

    #[test]
    fn normalize_trims_and_lowercases() {
        assert_eq!(normalize_list_q(Some("  Blog.Example  ")), "blog.example");
        assert_eq!(normalize_list_q(Some("   ")), "");
        assert_eq!(normalize_list_q(None), "");
    }

    #[test]
    fn match_domain_and_parent() {
        assert!(domain_matches_q(
            "blog.example.com",
            Some("example.com"),
            "blog"
        ));
        assert!(domain_matches_q(
            "blog.example.com",
            Some("example.com"),
            "EXAMPLE"
        ));
        assert!(domain_matches_q(
            "shop.example.com",
            Some("example.com"),
            "example.com"
        ));
        assert!(!domain_matches_q("a.com", Some("b.com"), "zzz"));
        assert!(domain_matches_q("a.com", None, ""));
    }

    #[test]
    fn form_uses_q_and_clear() {
        let html = list_search_form("/websites", "blog", "Search by domain");
        assert!(html.contains(r#"name="q""#));
        assert!(html.contains(r#"value="blog""#));
        assert!(html.contains("Clear"));
        assert!(html.contains(r#"action="/websites""#));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn form_extras_preserved() {
        let html = list_search_form_with_extras(
            "/websites/list",
            "",
            "Search",
            r#"<input type="hidden" name="per_page" value="5">"#,
        );
        assert!(html.contains(r#"name="per_page""#));
        assert!(html.contains(r#"value="5""#));
    }
}
