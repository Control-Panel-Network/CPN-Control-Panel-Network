//! Bundled panel documentation under repo `docs/` (operator manuals only).
//!
//! Policy lists (`blocked-passwords.txt`, `reserved-usernames.txt`) are excluded.
//! Content is embedded at build time via `rust-embed` so labs work offline.

use rust_embed::Embed;

#[derive(Embed)]
#[folder = "docs/"]
struct PanelDocsAssets;

const SKIP_FILES: &[&str] = &["blocked-passwords.txt", "reserved-usernames.txt"];

/// One browseable Markdown doc shipped with the panel.
#[derive(Debug, Clone)]
pub struct DocEntry {
    pub id: String,
    pub title: String,
    pub filename: String,
    pub body: String,
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn is_markdown_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") && !path.contains("..") && !path.contains('\\')
}

fn is_image_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    (lower.starts_with("images/") || lower.starts_with("images\\"))
        && (lower.ends_with(".png")
            || lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".gif")
            || lower.ends_with(".webp")
            || lower.ends_with(".svg"))
        && !path.contains("..")
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn slug_from_filename(filename: &str) -> String {
    let stem = filename
        .strip_suffix(".md")
        .or_else(|| filename.strip_suffix(".MD"))
        .unwrap_or(filename);
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn title_from_markdown(body: &str, fallback: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let t = rest.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    fallback.replace('-', " ")
}

fn should_skip(path: &str) -> bool {
    let name = basename(path);
    SKIP_FILES.iter().any(|s| name.eq_ignore_ascii_case(s))
}

/// All operator Markdown docs bundled in the binary, sorted by title.
pub fn list_docs() -> Vec<DocEntry> {
    let mut out = Vec::new();
    for path in PanelDocsAssets::iter() {
        let path = path.as_ref();
        if !is_markdown_path(path) || should_skip(path) {
            continue;
        }
        let Some(file) = PanelDocsAssets::get(path) else {
            continue;
        };
        let Ok(body) = std::str::from_utf8(file.data.as_ref()) else {
            continue;
        };
        let filename = basename(path).to_string();
        let id = slug_from_filename(&filename);
        if id.is_empty() {
            continue;
        }
        let title = title_from_markdown(body, &id);
        out.push(DocEntry {
            id,
            title,
            filename,
            body: body.to_string(),
        });
    }
    out.sort_by(|a, b| {
        a.title
            .to_ascii_lowercase()
            .cmp(&b.title.to_ascii_lowercase())
    });
    out
}

pub fn find_doc(page_id: &str) -> Option<DocEntry> {
    let want = page_id.trim().to_ascii_lowercase();
    if want.is_empty() {
        return None;
    }
    list_docs().into_iter().find(|d| d.id == want)
}

/// Case-insensitive search over doc titles and bodies.
pub fn search_docs(query: &str) -> Vec<DocEntry> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return list_docs();
    }
    list_docs()
        .into_iter()
        .filter(|d| {
            d.title.to_ascii_lowercase().contains(&q)
                || d.filename.to_ascii_lowercase().contains(&q)
                || d.id.contains(&q)
                || d.body.to_ascii_lowercase().contains(&q)
        })
        .collect()
}

/// Rewrite relative doc image/link paths so the safe Markdown renderer can use them.
pub fn preprocess_docs_markdown(md: &str) -> String {
    let mut out = md.to_string();
    // Images: ](images/foo.png) or ](./images/foo.png)
    out = out.replace("](./images/", "](/docs/images/");
    out = out.replace("](images/", "](/docs/images/");
    // Relative markdown peers: ](INSTALL.md) -> ](/docs?page=install)
    let docs = list_docs();
    for doc in &docs {
        let needle = format!("]({})", doc.filename);
        let alt = format!("](./{})", doc.filename);
        let href = format!("](/docs?page={})", doc.id);
        out = out.replace(&needle, &href);
        out = out.replace(&alt, &href);
    }
    out
}

/// Safe HTML for a docs page body (escaped Markdown subset + relative `/docs/` asset URLs).
pub fn render_doc_html(md: &str) -> String {
    let prepared = preprocess_docs_markdown(md);
    render_docs_safe_markdown(&prepared)
}

fn safe_docs_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 2_048 {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")
    {
        if trimmed
            .chars()
            .any(|c| c.is_control() || c == '"' || c == '\'' || c == '>')
        {
            return None;
        }
        return Some(html_escape(trimmed));
    }
    // In-panel relative docs assets and deep links only.
    if (trimmed.starts_with("/docs/images/") || trimmed.starts_with("/docs?"))
        && !trimmed.contains("..")
        && !trimmed
            .chars()
            .any(|c| c.is_control() || c == '"' || c == '\'' || c == '>')
    {
        return Some(html_escape(trimmed));
    }
    None
}

fn render_docs_safe_markdown(md: &str) -> String {
    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    const MAX_MD_CHARS: usize = 500_000;
    let clipped = if md.chars().count() > MAX_MD_CHARS {
        md.chars().take(MAX_MD_CHARS).collect::<String>()
    } else {
        md.to_string()
    };
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&clipped, options);
    let mut out = String::with_capacity(clipped.len().saturating_mul(2));

    let heading_level_num = |level: HeadingLevel| -> u8 {
        match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        }
        .clamp(1, 3)
    };

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => out.push_str("<p>"),
                Tag::Heading { level, .. } => {
                    out.push_str(&format!("<h{}>", heading_level_num(level)));
                }
                Tag::BlockQuote(_) => out.push_str("<blockquote>"),
                Tag::CodeBlock(_) => out.push_str("<pre><code>"),
                Tag::List(None) => out.push_str("<ul>"),
                Tag::List(Some(_)) => out.push_str("<ol>"),
                Tag::Item => out.push_str("<li>"),
                Tag::Emphasis => out.push_str("<em>"),
                Tag::Strong => out.push_str("<strong>"),
                Tag::Strikethrough => out.push_str("<s>"),
                Tag::Link {
                    dest_url, title, ..
                } => {
                    if let Some(href) = safe_docs_url(&dest_url) {
                        out.push_str("<a href=\"");
                        out.push_str(&href);
                        out.push('"');
                        if !title.is_empty() {
                            out.push_str(" title=\"");
                            out.push_str(&html_escape(&title));
                            out.push('"');
                        }
                        out.push_str(" rel=\"noopener noreferrer\">");
                    } else {
                        out.push_str("<span>");
                    }
                }
                Tag::Image {
                    dest_url, title, ..
                } => {
                    if let Some(src) = safe_docs_url(&dest_url) {
                        out.push_str("<img src=\"");
                        out.push_str(&src);
                        out.push('"');
                        if !title.is_empty() {
                            out.push_str(" alt=\"");
                            out.push_str(&html_escape(&title));
                            out.push('"');
                        } else {
                            out.push_str(" alt=\"\"");
                        }
                        out.push_str(" loading=\"lazy\">");
                    }
                }
                Tag::Table(_) => out.push_str("<table>"),
                Tag::TableHead => out.push_str("<thead><tr>"),
                Tag::TableRow => out.push_str("<tr>"),
                Tag::TableCell => out.push_str("<td>"),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => out.push_str("</p>\n"),
                TagEnd::Heading(level) => {
                    out.push_str(&format!("</h{}>\n", heading_level_num(level)));
                }
                TagEnd::BlockQuote(_) => out.push_str("</blockquote>\n"),
                TagEnd::CodeBlock => out.push_str("</code></pre>\n"),
                TagEnd::List(true) => out.push_str("</ol>\n"),
                TagEnd::List(false) => out.push_str("</ul>\n"),
                TagEnd::Item => out.push_str("</li>\n"),
                TagEnd::Emphasis => out.push_str("</em>"),
                TagEnd::Strong => out.push_str("</strong>"),
                TagEnd::Strikethrough => out.push_str("</s>"),
                TagEnd::Link => out.push_str("</a>"),
                TagEnd::Image => {}
                TagEnd::Table => out.push_str("</table>\n"),
                TagEnd::TableHead => out.push_str("</tr></thead>\n"),
                TagEnd::TableRow => out.push_str("</tr>\n"),
                TagEnd::TableCell => out.push_str("</td>"),
                _ => {}
            },
            Event::Text(text) => out.push_str(&html_escape(&text)),
            Event::Code(text) => {
                out.push_str("<code>");
                out.push_str(&html_escape(&text));
                out.push_str("</code>");
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                out.push_str(&html_escape(&html));
            }
            Event::SoftBreak => out.push('\n'),
            Event::HardBreak => out.push_str("<br>\n"),
            Event::Rule => out.push_str("<hr>\n"),
            _ => {}
        }
    }
    out
}

/// Serve a bundled image under `docs/images/`.
pub fn docs_image_bytes(rel: &str) -> Option<(Vec<u8>, &'static str)> {
    let cleaned = rel.trim().trim_start_matches('/');
    let path = if cleaned.starts_with("images/") {
        cleaned.to_string()
    } else {
        format!("images/{cleaned}")
    };
    if !is_image_path(&path) {
        return None;
    }
    let file = PanelDocsAssets::get(&path)?;
    let mime = if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    };
    Some((file.data.as_ref().to_vec(), mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_key_operator_docs() {
        let docs = list_docs();
        assert!(docs.iter().any(|d| d.id == "install"));
        assert!(docs.iter().any(|d| d.id == "cli"));
        assert!(docs.iter().any(|d| d.id == "support"));
        assert!(docs.iter().any(|d| d.id == "plugin-cpn-only-scope"));
        assert!(
            !docs
                .iter()
                .any(|d| d.filename.contains("blocked-passwords"))
        );
        assert!(
            !docs
                .iter()
                .any(|d| d.filename.contains("reserved-usernames"))
        );
    }

    #[test]
    fn search_finds_install_target_copy() {
        let hits = search_docs("User Panel");
        assert!(
            hits.iter().any(|d| d.id == "plugin-cpn-only-scope"),
            "expected PLUGIN-CPN-ONLY-SCOPE in search hits"
        );
    }

    #[test]
    fn render_escapes_scripts_and_allows_docs_images() {
        let html =
            render_doc_html("Hello **world** <script>x</script>\n\n![x](images/cpn-login.png)");
        assert!(html.contains("<strong>world</strong>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("src=\"/docs/images/cpn-login.png\""));
    }

    #[test]
    fn image_asset_loads() {
        let img = docs_image_bytes("cpn-login.png");
        assert!(img.is_some());
        assert!(img.unwrap().0.len() > 10);
    }
}
