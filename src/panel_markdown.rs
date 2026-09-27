//! Safe Markdown subset for owner-editable panel messages.
//!
//! Renders a limited set of tags; escapes all text. Disallows scripts, event
//! handlers, and non-http(s) / relative-unsafe URLs.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

const MAX_MD_CHARS: usize = 16_000;

fn heading_level_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
    .clamp(1, 3)
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn safe_http_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 2_048 {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:"))
    {
        return None;
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || c == '"' || c == '\'' || c == '>')
    {
        return None;
    }
    Some(html_escape(trimmed))
}

/// Convert Markdown to an HTML fragment using a safe tag allowlist.
pub fn render_safe_markdown(md: &str) -> String {
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
    for event in parser {
        match event {
            Event::Start(tag) => start_tag(&mut out, tag),
            Event::End(tag) => end_tag(&mut out, tag),
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
            Event::FootnoteReference(name) => out.push_str(&html_escape(&name)),
            Event::InlineMath(m) | Event::DisplayMath(m) => out.push_str(&html_escape(&m)),
            Event::TaskListMarker(_) => {}
        }
    }
    out
}

fn start_tag(out: &mut String, tag: Tag<'_>) {
    match tag {
        Tag::Paragraph => out.push_str("<p>"),
        Tag::Heading { level, .. } => {
            let n = heading_level_num(level);
            out.push_str(&format!("<h{n}>"));
        }
        Tag::BlockQuote(_) => out.push_str("<blockquote>"),
        Tag::CodeBlock(_) => out.push_str("<pre><code>"),
        Tag::List(None) => out.push_str("<ul>"),
        Tag::List(Some(1)) => out.push_str("<ol>"),
        Tag::List(Some(_)) => out.push_str("<ol>"),
        Tag::Item => out.push_str("<li>"),
        Tag::Emphasis => out.push_str("<em>"),
        Tag::Strong => out.push_str("<strong>"),
        Tag::Strikethrough => out.push_str("<s>"),
        Tag::Link {
            dest_url, title, ..
        } => {
            if let Some(href) = safe_http_url(&dest_url) {
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
            if let Some(src) = safe_http_url(&dest_url) {
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
                out.push('>');
            }
        }
        Tag::Table(_) => out.push_str("<table>"),
        Tag::TableHead => out.push_str("<thead><tr>"),
        Tag::TableRow => out.push_str("<tr>"),
        Tag::TableCell => out.push_str("<td>"),
        Tag::HtmlBlock => {}
        Tag::FootnoteDefinition(_)
        | Tag::DefinitionList
        | Tag::DefinitionListTitle
        | Tag::DefinitionListDefinition
        | Tag::MetadataBlock(_) => {}
        Tag::Superscript | Tag::Subscript => {}
    }
}

fn end_tag(out: &mut String, tag: TagEnd) {
    match tag {
        TagEnd::Paragraph => out.push_str("</p>\n"),
        TagEnd::Heading(level) => {
            let n = heading_level_num(level);
            out.push_str(&format!("</h{n}>\n"));
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
        TagEnd::HtmlBlock => {}
        TagEnd::FootnoteDefinition
        | TagEnd::DefinitionList
        | TagEnd::DefinitionListTitle
        | TagEnd::DefinitionListDefinition
        | TagEnd::MetadataBlock(_)
        | TagEnd::Superscript
        | TagEnd::Subscript => {}
    }
}

pub use crate::panel_markdown_editor::{
    MARKDOWN_PREVIEW_PATH, html_template_editor_field, markdown_editor_field,
    markdown_editor_field_with_preview, markdown_toolbar_assets,
};

#[cfg(test)]
mod tests {
    use super::render_safe_markdown;

    #[test]
    fn escapes_raw_html_and_renders_bold() {
        let html = render_safe_markdown("Hello **world** <script>x</script>");
        assert!(html.contains("<strong>world</strong>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn rejects_javascript_urls() {
        let html = render_safe_markdown("[x](javascript:alert(1))");
        assert!(!html.contains("javascript:"));
        assert!(html.contains("<span>") || html.contains("x"));
    }

    #[test]
    fn allows_https_links() {
        let html = render_safe_markdown("[docs](https://example.com/a)");
        assert!(html.contains("href=\"https://example.com/a\""));
        assert!(html.contains("rel=\"noopener noreferrer\""));
    }
}
