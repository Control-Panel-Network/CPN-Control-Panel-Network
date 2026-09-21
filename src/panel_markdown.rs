//! Safe Markdown subset for owner-editable panel messages.
//!
//! Renders a limited set of tags; escapes all text. Disallows scripts, event
//! handlers, and non-http(s) / relative-unsafe URLs.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

const MAX_MD_CHARS: usize = 16_000;

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
            let n = u8::from(level).clamp(1, 3);
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
            let n = u8::from(level).clamp(1, 3);
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
        | TagEnd::MetadataBlock
        | TagEnd::Superscript
        | TagEnd::Subscript => {}
    }
}

/// Toolbar + preview script/CSS matching rawdata-style Markdown editors.
pub fn markdown_toolbar_assets() -> &'static str {
    r#"
<style>
.cpn-md-wrap { margin:0 0 12px; }
.cpn-md-label {
  display:block; margin:0 0 6px; font-size:11px; letter-spacing:.04em;
  text-transform:uppercase; color:var(--muted,#94a3b8); font-weight:600;
}
.cpn-md-box {
  border:1px solid var(--hairline,#334155); border-radius:8px; background:var(--canvas,#0f172a);
  overflow:hidden;
}
.cpn-md-toolbar {
  display:flex; flex-wrap:wrap; gap:6px; padding:8px; border-bottom:1px solid var(--hairline,#334155);
  background:rgba(15,23,42,.55);
}
.cpn-md-btn {
  min-width:34px; height:32px; padding:0 8px; border:1px solid #475569; border-radius:6px;
  background:#1e293b; color:#e2e8f0; font-size:12px; font-weight:700; cursor:pointer; line-height:1;
}
.cpn-md-btn:hover { border-color:#38bdf8; color:#7dd3fc; }
.cpn-md-btn.is-active { background:#334155; border-color:#94a3b8; }
.cpn-md-btn.is-italic { font-style:italic; }
.cpn-md-box textarea {
  display:block; width:100%; min-height:140px; box-sizing:border-box; margin:0; padding:12px;
  border:0; resize:vertical; background:transparent; color:var(--ink,#e2e8f0); font:inherit;
}
.cpn-md-preview {
  display:none; min-height:140px; max-height:360px; overflow:auto; padding:12px;
  color:var(--ink,#e2e8f0); font-size:14px; line-height:1.5;
}
.cpn-md-preview.is-open { display:block; }
.cpn-md-preview.is-open + textarea, .cpn-md-box.is-previewing textarea { display:none; }
.cpn-md-actions { display:flex; justify-content:flex-end; margin-top:8px; }
.cpn-md-preview-btn {
  display:inline-flex; align-items:center; gap:6px; padding:8px 14px; border-radius:8px;
  border:1px solid #475569; background:#334155; color:#f1f5f9; font:inherit; font-size:13px;
  font-weight:600; cursor:pointer;
}
.cpn-md-preview-btn:hover { border-color:#94a3b8; }
.cpn-md-preview h1,.cpn-md-preview h2,.cpn-md-preview h3 { margin:0.6em 0 0.35em; }
.cpn-md-preview p { margin:0.4em 0; }
.cpn-md-preview code { font-family:ui-monospace,monospace; font-size:0.92em; }
.cpn-md-preview pre { padding:10px; overflow:auto; border-radius:6px; background:#020617; }
.cpn-md-preview table { border-collapse:collapse; width:100%; margin:0.5em 0; }
.cpn-md-preview td,.cpn-md-preview th { border:1px solid #475569; padding:6px 8px; }
</style>
<script>
(function(){
  function wrap(ta, before, after, ph){
    var s=ta.selectionStart||0, e=ta.selectionEnd||0, v=ta.value||'';
    var sel=v.substring(s,e); var ins=sel.length?sel:(ph||'');
    ta.value=v.substring(0,s)+before+ins+after+v.substring(e);
    ta.focus(); ta.setSelectionRange(s+before.length, s+before.length+ins.length);
  }
  function prefix(ta, pfx){
    var s=ta.selectionStart||0, e=ta.selectionEnd||0, v=ta.value||'';
    var bs=v.lastIndexOf('\n', Math.max(0,s-1))+1;
    var be=v.indexOf('\n', e); if(be<0) be=v.length;
    var block=v.substring(bs,be).split('\n').map(function(l){return pfx+l;}).join('\n');
    ta.value=v.substring(0,bs)+block+v.substring(be);
    ta.focus(); ta.setSelectionRange(bs, bs+block.length);
  }
  function insert(ta, block){
    var s=ta.selectionStart||0, v=ta.value||'';
    ta.value=v.substring(0,s)+block+v.substring(s);
    var p=s+block.length; ta.focus(); ta.setSelectionRange(p,p);
  }
  function togglePreview(box, ta, btn){
    var prev=box.querySelector('.cpn-md-preview');
    if(!prev) return;
    var open=prev.classList.contains('is-open');
    if(open){
      prev.classList.remove('is-open'); box.classList.remove('is-previewing');
      btn.classList.remove('is-active'); btn.setAttribute('aria-pressed','false');
      return;
    }
    var md=ta.value||'';
    fetch('/settings/error-messages/preview', {
      method:'POST',
      headers:{'Content-Type':'application/x-www-form-urlencoded'},
      body:'markdown='+encodeURIComponent(md),
      credentials:'same-origin'
    }).then(function(r){ return r.text(); }).then(function(html){
      prev.innerHTML=html;
      prev.classList.add('is-open'); box.classList.add('is-previewing');
      btn.classList.add('is-active'); btn.setAttribute('aria-pressed','true');
    }).catch(function(){
      prev.textContent=md;
      prev.classList.add('is-open'); box.classList.add('is-previewing');
    });
  }
  function bind(root){
    var ta=root.querySelector('textarea');
    if(!ta) return;
    root.querySelectorAll('[data-md-action]').forEach(function(btn){
      btn.addEventListener('click', function(ev){
        ev.preventDefault();
        var a=btn.getAttribute('data-md-action');
        if(a==='bold') wrap(ta,'**','**','bold');
        else if(a==='italic') wrap(ta,'*','*','italic');
        else if(a==='code') wrap(ta,'`','`','code');
        else if(a==='codeblock') insert(ta,'\n```\ncode\n```\n');
        else if(a==='link'){ var u=prompt('URL (https://...)','https://'); if(u) wrap(ta,'[',']('+u+')','link text'); }
        else if(a==='image'){ var u=prompt('Image URL (https://...)','https://'); if(u) insert(ta,'![alt]('+u+')'); }
        else if(a==='quote') prefix(ta,'> ');
        else if(a==='ul') prefix(ta,'- ');
        else if(a==='ol') prefix(ta,'1. ');
        else if(a==='heading') prefix(ta,'## ');
        else if(a==='table') insert(ta,'| Column | Column |\n| --- | --- |\n| Cell | Cell |\n');
        else if(a==='hr') insert(ta,'\n---\n');
        else if(a==='preview') togglePreview(root.querySelector('.cpn-md-box')||root, ta, btn);
      });
    });
  }
  document.querySelectorAll('[data-cpn-md]').forEach(bind);
})();
</script>
"#
}

/// Build a labeled Markdown editor block for a form field.
pub fn markdown_editor_field(label: &str, name: &str, value: &str, placeholder: &str) -> String {
    format!(
        r#"<div class="cpn-md-wrap" data-cpn-md>
  <span class="cpn-md-label">{label}</span>
  <div class="cpn-md-box">
    <div class="cpn-md-toolbar" role="toolbar" aria-label="Markdown formatting">
      <button type="button" class="cpn-md-btn" data-md-action="bold" title="Bold">B</button>
      <button type="button" class="cpn-md-btn is-italic" data-md-action="italic" title="Italic">I</button>
      <button type="button" class="cpn-md-btn" data-md-action="code" title="Inline code">&lt;/&gt;</button>
      <button type="button" class="cpn-md-btn" data-md-action="codeblock" title="Code block">{{ }}</button>
      <button type="button" class="cpn-md-btn" data-md-action="link" title="Link">Link</button>
      <button type="button" class="cpn-md-btn" data-md-action="image" title="Image">Img</button>
      <button type="button" class="cpn-md-btn" data-md-action="quote" title="Blockquote">&ldquo;</button>
      <button type="button" class="cpn-md-btn" data-md-action="ul" title="Bulleted list">UL</button>
      <button type="button" class="cpn-md-btn" data-md-action="ol" title="Numbered list">OL</button>
      <button type="button" class="cpn-md-btn" data-md-action="heading" title="Heading">H</button>
      <button type="button" class="cpn-md-btn" data-md-action="table" title="Table">Tbl</button>
      <button type="button" class="cpn-md-btn" data-md-action="hr" title="Horizontal rule">-</button>
    </div>
    <div class="cpn-md-preview" aria-live="polite"></div>
    <textarea id="{name}" name="{name}" rows="8" placeholder="{ph}">{value}</textarea>
  </div>
  <div class="cpn-md-actions">
    <button type="button" class="cpn-md-preview-btn" data-md-action="preview" aria-pressed="false">
      <span aria-hidden="true">&#128065;</span> Preview
    </button>
  </div>
</div>"#,
        label = html_escape(label),
        name = html_escape(name),
        value = html_escape(value),
        ph = html_escape(placeholder),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
