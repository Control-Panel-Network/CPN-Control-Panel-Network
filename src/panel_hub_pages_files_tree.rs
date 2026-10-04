//! Sidebar tree for File Manager: ancestors and well-known roots only.
//! Never lists `/home` (or other huge restore trees) on first load.

use crate::panel_hub_http::urlencoding_simple;
use std::path::Path;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn page_href(base_url: &str, query_extra: &str, path: &str) -> String {
    format!(
        "{}?{}path={}",
        base_url,
        query_extra,
        urlencoding_simple(path)
    )
}

fn unixish(path: &str) -> String {
    path.replace('\\', "/")
}

/// Well-known top-level dirs shown under `/` without reading `/home` contents.
const ROOT_SHORTCUTS: &[&str] = &["etc", "home", "opt", "root", "tmp", "usr", "var", "boot"];

pub fn build_tree_html(
    base_url: &str,
    query_extra: &str,
    jail_home: &str,
    current: &str,
) -> String {
    let jail = unixish(jail_home);
    let current_n = unixish(current);
    let mut out = format!(
        r#"<ul class="fm-tree-list"><li><a href="{home}">{label}</a><ul>"#,
        home = page_href(base_url, query_extra, &jail),
        label = html_escape(&jail),
    );
    if jail == "/" {
        for name in ROOT_SHORTCUTS {
            let p = format!("/{name}");
            if !Path::new(&p).is_dir() {
                continue;
            }
            out.push_str(&format!(
                r#"<li><a href="{href}">{name}</a>"#,
                href = page_href(base_url, query_extra, &p),
                name = html_escape(name),
            ));
            if current_n == p || current_n.starts_with(&(p.clone() + "/")) {
                out.push_str(&ancestor_chain(base_url, query_extra, &p, &current_n));
            }
            out.push_str("</li>");
        }
    } else if current_n != jail && current_n.starts_with(&(jail.clone() + "/")) {
        out.push_str(&ancestor_chain(base_url, query_extra, &jail, &current_n));
    }
    out.push_str("</ul></li></ul>");
    out
}

fn ancestor_chain(base_url: &str, query_extra: &str, from: &str, current: &str) -> String {
    let rest = current
        .strip_prefix(from)
        .unwrap_or("")
        .trim_start_matches('/');
    if rest.is_empty() {
        return String::new();
    }
    let mut acc = from.trim_end_matches('/').to_string();
    let mut inner = String::from("<ul>");
    for seg in rest.split('/').filter(|s| !s.is_empty()).take(24) {
        if acc == "/" {
            acc = format!("/{seg}");
        } else {
            acc = format!("{acc}/{seg}");
        }
        inner.push_str(&format!(
            r#"<li><a href="{href}">{name}</a></li>"#,
            href = page_href(base_url, query_extra, &acc),
            name = html_escape(seg),
        ));
    }
    inner.push_str("</ul>");
    inner
}

#[cfg(test)]
mod tests {
    use super::ROOT_SHORTCUTS;

    #[test]
    fn shortcuts_do_not_include_virtual_fs() {
        assert!(!ROOT_SHORTCUTS.contains(&"proc"));
        assert!(!ROOT_SHORTCUTS.contains(&"sys"));
        assert!(ROOT_SHORTCUTS.contains(&"home"));
    }
}
