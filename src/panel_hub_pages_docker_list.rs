//! Search + pagination chrome for Docker Active Containers and Local Images lists.

use crate::panel_ops_docker_inventory::{docker_list_page, docker_list_per_page};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct DockerListQuery {
    pub q: String,
    pub page: usize,
    pub per_page: usize,
}

impl DockerListQuery {
    pub fn from_map(query: &std::collections::HashMap<String, String>) -> Self {
        Self {
            q: query.get("q").cloned().unwrap_or_default(),
            page: docker_list_page(query.get("page").map(String::as_str).unwrap_or("")),
            per_page: docker_list_per_page(query.get("per_page").map(String::as_str).unwrap_or("")),
        }
    }

    pub fn local_filter_from_map(query: &std::collections::HashMap<String, String>) -> Self {
        Self {
            q: query
                .get("lq")
                .cloned()
                .or_else(|| query.get("filter").cloned())
                .unwrap_or_default(),
            page: docker_list_page(query.get("page").map(String::as_str).unwrap_or("")),
            per_page: docker_list_per_page(query.get("per_page").map(String::as_str).unwrap_or("")),
        }
    }
}

pub fn paginate_slice<'a, T>(items: &'a [T], page: usize, per_page: usize) -> (&'a [T], usize) {
    let total = items.len();
    let total_pages = total.div_ceil(per_page).max(1);
    let page = page.min(total_pages).max(1);
    let start = (page - 1) * per_page;
    if start >= total {
        return (&[], total_pages);
    }
    let end = (start + per_page).min(total);
    (&items[start..end], total_pages)
}

pub fn docker_list_toolbar(
    action: &str,
    search_name: &str,
    search_placeholder: &str,
    q: &str,
    page: usize,
    per_page: usize,
    total_pages: usize,
    total_items: usize,
    extra_hidden: &str,
) -> String {
    let sizes = [5usize, 10, 25, 50];
    let mut options = String::new();
    for size in sizes {
        let sel = if size == per_page { " selected" } else { "" };
        options.push_str(&format!(
            r#"<option value="{size}"{sel}>{size}</option>"#,
            size = size,
            sel = sel
        ));
    }
    let prev = if page > 1 { page - 1 } else { 1 };
    let next = if page < total_pages {
        page + 1
    } else {
        total_pages.max(1)
    };
    let q_enc = urlencoding_simple(q);
    let base_q = format!(
        "{search_name}={q}&amp;per_page={per_page}",
        search_name = html_escape(search_name),
        q = html_escape(&q_enc),
        per_page = per_page
    );
    let extra_qs = if extra_hidden.trim().is_empty() {
        String::new()
    } else {
        let cleaned = extra_hidden
            .trim()
            .trim_start_matches('&')
            .replace('&', "&amp;");
        format!("&amp;{cleaned}")
    };
    let prev_href = format!(
        "{action}?{base}&amp;page={prev}{extra}",
        action = html_escape(action),
        base = base_q,
        prev = prev,
        extra = extra_qs
    );
    let next_href = format!(
        "{action}?{base}&amp;page={next}{extra}",
        action = html_escape(action),
        base = base_q,
        next = next,
        extra = extra_qs
    );
    let prev_attr = if page <= 1 {
        r#" aria-disabled="true" class="btn-secondary is-disabled""#.to_string()
    } else {
        format!(r#" class="btn-secondary" href="{prev_href}""#)
    };
    let next_attr = if page >= total_pages.max(1) {
        r#" aria-disabled="true" class="btn-secondary is-disabled""#.to_string()
    } else {
        format!(r#" class="btn-secondary" href="{next_href}""#)
    };
    format!(
        r#"<form method="get" action="{action}" class="docker-list-toolbar" role="search">
      {extra_fields}
      <label class="docker-list-search">Search
        <input type="search" name="{search_name}" value="{q}" maxlength="100" placeholder="{ph}" autocomplete="off">
      </label>
      <label>Show
        <select name="per_page" aria-label="Rows per page" onchange="this.form.submit()">{options}</select>
      </label>
      <button type="submit" class="btn-secondary">Apply</button>
      <span class="muted docker-list-count">{total} matching</span>
      <span class="page-status">Page {page} / {total_pages}</span>
      <a{prev_attr}>Prev</a>
      <a{next_attr}>Next</a>
      <label class="docker-goto">Go to
        <input type="number" name="page" min="1" max="{total_pages}" value="{page}">
      </label>
      <button type="submit" class="btn-primary">Go</button>
    </form>"#,
        action = html_escape(action),
        extra_fields = extra_hidden_inputs(extra_hidden),
        search_name = html_escape(search_name),
        q = html_escape(q),
        ph = html_escape(search_placeholder),
        options = options,
        total = total_items,
        page = page.min(total_pages.max(1)),
        total_pages = total_pages.max(1),
        prev_attr = prev_attr,
        next_attr = next_attr,
    )
}

fn extra_hidden_inputs(extra: &str) -> String {
    // extra is pre-escaped query fragment like &amp;hub_q=nginx; convert to hidden inputs when needed.
    // Callers pass raw pairs as `key=value&key2=value2` without leading &.
    let raw = extra.trim().trim_start_matches('&').replace("&amp;", "&");
    if raw.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for part in raw.split('&') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        if k == "q" || k == "lq" || k == "page" || k == "per_page" || k == "filter" {
            continue;
        }
        out.push_str(&format!(
            r#"<input type="hidden" name="{k}" value="{v}">"#,
            k = html_escape(k),
            v = html_escape(v),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paginate_defaults() {
        let items: Vec<u8> = (0..25).collect();
        let (page1, pages) = paginate_slice(&items, 1, 10);
        assert_eq!(pages, 3);
        assert_eq!(page1.len(), 10);
        let (page3, _) = paginate_slice(&items, 3, 10);
        assert_eq!(page3.len(), 5);
    }

    #[test]
    fn query_defaults() {
        let map = std::collections::HashMap::new();
        let q = DockerListQuery::from_map(&map);
        assert_eq!(q.page, 1);
        assert_eq!(q.per_page, 10);
        assert!(q.q.is_empty());
    }
}
