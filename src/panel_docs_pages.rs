//! HTML for the in-panel Documentation browser (`/docs`).

use crate::panel_docs::{DocEntry, find_doc, list_docs, render_doc_html, search_docs};

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
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn docs_styles() -> &'static str {
    r#"
.docs-layout {
  display:grid; grid-template-columns:minmax(200px,260px) minmax(0,1fr); gap:20px; align-items:start;
}
.docs-side {
  position:sticky; top:12px; max-height:calc(100vh - 120px); overflow:auto;
  padding:14px; border:1px solid var(--hairline); border-radius:10px; background:var(--canvas);
}
.docs-side h2 { margin:0 0 10px; font-size:14px; }
.docs-search { width:100%; margin-bottom:12px; }
.docs-search input[type=search] {
  width:100%; min-height:40px; padding:8px 12px; border-radius:8px;
  border:1px solid var(--hairline); background:var(--surface); color:var(--ink);
}
.docs-nav { list-style:none; margin:0; padding:0; display:flex; flex-direction:column; gap:4px; }
.docs-nav a {
  display:block; padding:8px 10px; border-radius:8px; font-size:14px; color:var(--ink);
}
.docs-nav a:hover { background:var(--surface); }
.docs-nav a.active { background:#e7f1ff; color:#0066cc; font-weight:600; }
.docs-main {
  min-width:0; padding:18px 20px 28px; border:1px solid var(--hairline);
  border-radius:10px; background:var(--canvas);
}
.docs-main .docs-meta { margin:0 0 14px; color:var(--muted); font-size:13px; }
.docs-body { font-size:15px; line-height:1.55; overflow-wrap:anywhere; }
.docs-body h1 { margin:0 0 12px; font-size:28px; line-height:1.2; }
.docs-body h2 { margin:22px 0 10px; font-size:20px; }
.docs-body h3 { margin:18px 0 8px; font-size:17px; }
.docs-body p, .docs-body ul, .docs-body ol { margin:0 0 12px; }
.docs-body code {
  font-family:ui-monospace,Consolas,monospace; font-size:0.92em;
  padding:1px 5px; border-radius:4px; background:var(--surface);
}
.docs-body pre {
  overflow:auto; padding:12px 14px; border-radius:8px; background:var(--surface);
  border:1px solid var(--hairline); margin:0 0 14px;
}
.docs-body pre code { padding:0; background:transparent; }
.docs-body table { width:100%; border-collapse:collapse; margin:0 0 14px; font-size:14px; }
.docs-body th, .docs-body td {
  border:1px solid var(--hairline); padding:8px 10px; text-align:left; vertical-align:top;
}
.docs-body img { max-width:100%; height:auto; border-radius:8px; margin:8px 0; }
.docs-body a { color:var(--blue); text-decoration:underline; }
.docs-empty { color:var(--muted); }
[data-color-mode="dark"] .docs-nav a.active { background:rgba(0,102,204,.22); color:#8ec8ff; }
[data-color-mode="dark"] .docs-side, [data-color-mode="dark"] .docs-main {
  background:rgba(28,28,32,.92); border-color:rgba(255,255,255,.08);
}
@media (max-width:900px) {
  .docs-layout { grid-template-columns:1fr; }
  .docs-side { position:static; max-height:none; }
}
"#
}

fn nav_list(docs: &[DocEntry], active_id: &str, q: &str) -> String {
    let mut items = String::new();
    for doc in docs {
        let cls = if doc.id == active_id { " active" } else { "" };
        let href = if q.trim().is_empty() {
            format!("/docs?page={}", urlencoding_simple(&doc.id))
        } else {
            format!(
                "/docs?page={}&amp;q={}",
                urlencoding_simple(&doc.id),
                urlencoding_simple(q)
            )
        };
        items.push_str(&format!(
            r#"<li><a class="{cls}" href="{href}">{title}</a></li>"#,
            cls = cls.trim_start(),
            href = href,
            title = html_escape(&doc.title),
        ));
    }
    if items.is_empty() {
        items = r#"<li class="docs-empty">No matching documents.</li>"#.into();
    }
    format!(r#"<ul class="docs-nav">{items}</ul>"#)
}

fn default_page_id(docs: &[DocEntry]) -> String {
    docs.iter()
        .find(|d| d.id == "install")
        .or_else(|| docs.first())
        .map(|d| d.id.clone())
        .unwrap_or_default()
}

/// Full main column HTML for `/docs` (wrapped by `panel_shell` in the route).
pub fn docs_main(page: &str, q: &str) -> String {
    let all = list_docs();
    let filtered = search_docs(q);
    let active = {
        let want = page.trim().to_ascii_lowercase();
        if !want.is_empty() && filtered.iter().any(|d| d.id == want) {
            want
        } else if !want.is_empty() && all.iter().any(|d| d.id == want) {
            want
        } else {
            default_page_id(&filtered)
        }
    };
    let doc = find_doc(&active).or_else(|| filtered.first().cloned());
    let nav = nav_list(&filtered, &active, q);
    let body = match &doc {
        Some(d) => {
            let html = render_doc_html(&d.body);
            format!(
                r#"<p class="docs-meta">Bundled from panel <code>docs/{file}</code>. Source tree: <a href="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/tree/stable/docs" target="_blank" rel="noopener noreferrer">GitHub stable/docs</a>.</p>
        <article class="docs-body">{html}</article>"#,
                file = html_escape(&d.filename),
                html = html,
            )
        }
        None => r#"<p class="docs-empty">No documentation matched this search.</p>"#.into(),
    };
    let title = doc
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Documentation");
    format!(
        r#"<style>{styles}</style>
      <p class="eyebrow">CPN PANEL</p>
      <h1>Documentation</h1>
      <p class="muted">Operator manuals shipped with this panel build. Search titles and body text; images under <code>docs/images</code> load in-panel.</p>
      <div class="docs-layout" style="margin-top:18px;">
        <aside class="docs-side" aria-label="Documentation list">
          <h2>Browse</h2>
          <form class="docs-search" method="get" action="/docs" role="search">
            <input type="hidden" name="page" value="{page}">
            <label class="sr-only" for="docs-q">Search documentation</label>
            <input id="docs-q" type="search" name="q" value="{q}" placeholder="Search docs..." autocomplete="off">
          </form>
          {nav}
        </aside>
        <section class="docs-main" aria-label="{title_esc}">
          {body}
        </section>
      </div>"#,
        styles = docs_styles(),
        page = html_escape(&active),
        q = html_escape(q),
        nav = nav,
        body = body,
        title_esc = html_escape(title),
    )
}

#[cfg(test)]
mod tests {
    use super::docs_main;

    #[test]
    fn page_renders_install_and_search_form() {
        let html = docs_main("install", "");
        assert!(html.contains("Documentation"));
        assert!(html.contains("name=\"q\""));
        assert!(html.contains("/docs?page=install") || html.contains("INSTALL"));
        assert!(!html.to_ascii_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn search_filters_nav() {
        let html = docs_main("cli", "command line");
        assert!(html.contains("CLI") || html.contains("cli"));
    }
}
