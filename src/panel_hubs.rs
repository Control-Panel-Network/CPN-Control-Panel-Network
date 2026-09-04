//! Shared hub tile UI helpers for CPN panel section pages.

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Clone, Copy)]
pub struct HubTile<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub href: &'a str,
    pub live: bool,
}

pub fn section_heading(title: &str, blurb: &str) -> String {
    format!(
        r#"
      <div class="dashboard-heading">
        <div>
          <p class="eyebrow">CPN PANEL</p>
          <h1>{title}</h1>
          <p>{blurb}</p>
        </div>
      </div>"#,
        title = html_escape(title),
        blurb = html_escape(blurb),
    )
}

pub fn notice_block(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{msg}</p>"#,
        msg = html_escape(message)
    )
}

pub fn breadcrumb(parts: &[(&str, Option<&str>)]) -> String {
    let mut out = String::from(r#"<nav class="hub-breadcrumb" aria-label="Breadcrumb">"#);
    for (i, (label, href)) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(r#" <span aria-hidden="true">/</span> "#);
        }
        if let Some(href) = href {
            out.push_str(&format!(
                r#"<a href="{href}">{label}</a>"#,
                href = html_escape(href),
                label = html_escape(label),
            ));
        } else {
            out.push_str(&format!("<strong>{}</strong>", html_escape(label)));
        }
    }
    out.push_str("</nav>");
    out
}

pub fn hub_tiles_grid(section_title: &str, tiles: &[HubTile<'_>]) -> String {
    let mut out = format!(
        r#"<h2 class="hub-section-title">{title}</h2>
      <div class="hub-tile-grid">"#,
        title = html_escape(section_title),
    );
    for tile in tiles {
        let badge = if tile.live {
            r#"<span class="hub-badge live">Live</span>"#
        } else {
            r#"<span class="hub-badge scaffold">Scaffold</span>"#
        };
        out.push_str(&format!(
            r#"<a class="hub-tile" href="{href}">
          <span class="hub-tile-icon" aria-hidden="true"></span>
          <span class="hub-tile-copy">
            <strong>{title}</strong>
            <span>{subtitle}</span>
          </span>
          {badge}
        </a>"#,
            href = html_escape(tile.href),
            title = html_escape(tile.title),
            subtitle = html_escape(tile.subtitle),
            badge = badge,
        ));
    }
    out.push_str("</div>");
    out
}

pub fn feature_shell(
    crumbs: &[(&str, Option<&str>)],
    title: &str,
    subtitle: &str,
    body: &str,
    notice: Option<&str>,
    error: Option<&str>,
) -> String {
    format!(
        r#"{crumbs}
      {heading}
      {ok}
      {err}
      <article class="section-card">{body}</article>"#,
        crumbs = breadcrumb(crumbs),
        heading = section_heading(title, subtitle),
        ok = notice_block("ok", notice),
        err = notice_block("error", error),
        body = body,
    )
}

pub fn not_configured_body(detail: &str, next_step: &str) -> String {
    format!(
        r#"<p><strong>Not configured yet</strong></p>
        <p>{detail}</p>
        <p class="muted">{next}</p>"#,
        detail = html_escape(detail),
        next = html_escape(next_step),
    )
}

pub fn status_kv(rows: &[(&str, &str)]) -> String {
    let mut out = String::from(r#"<ul class="kv-list">"#);
    for (k, v) in rows {
        out.push_str(&format!(
            r#"<li><span>{k}</span><strong>{v}</strong></li>"#,
            k = html_escape(k),
            v = html_escape(v),
        ));
    }
    out.push_str("</ul>");
    out
}

pub fn hub_styles() -> &'static str {
    r#"
.hub-breadcrumb { max-width:1200px; margin:0 auto 18px; color:var(--muted); font-size:13px; }
.hub-breadcrumb a { color:var(--blue); }
.hub-section-title {
  max-width:1200px; margin:28px auto 12px; color:var(--muted); font-size:12px;
  font-weight:700; letter-spacing:.08em; text-transform:uppercase;
}
.hub-tile-grid {
  max-width:1200px; margin:0 auto; display:grid;
  grid-template-columns:repeat(auto-fill,minmax(240px,1fr)); gap:14px;
}
.hub-tile {
  display:flex; align-items:flex-start; gap:12px; min-height:88px; padding:16px 14px;
  border-radius:16px; background:var(--canvas); border:1px solid var(--hairline);
  color:inherit; transition:border-color 120ms ease, box-shadow 120ms ease;
}
.hub-tile:hover { border-color:#b6d0f7; box-shadow:0 6px 18px rgba(0,102,204,.08); }
.hub-tile-icon {
  width:40px; height:40px; flex:0 0 40px; border-radius:12px;
  background:#eef4ff; border:1px solid #d7e6ff;
}
.hub-tile-copy { display:flex; flex-direction:column; gap:4px; min-width:0; flex:1; }
.hub-tile-copy strong { font-size:15px; letter-spacing:-.01em; }
.hub-tile-copy span { color:var(--muted); font-size:12px; line-height:1.35; }
.hub-badge {
  align-self:flex-start; padding:3px 8px; border-radius:999px; font-size:10px;
  font-weight:700; letter-spacing:.04em; text-transform:uppercase;
}
.hub-badge.live { background:#ecfdf3; color:#067647; }
.hub-badge.scaffold { background:#f2f4f7; color:#475467; }
@media (prefers-reduced-motion:reduce) {
  .hub-tile { transition:none; }
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_escape_html() {
        let html = hub_tiles_grid(
            "Test",
            &[HubTile {
                title: "<x>",
                subtitle: "a&b",
                href: "/server",
                live: true,
            }],
        );
        assert!(html.contains("&lt;x&gt;"));
        assert!(html.contains("a&amp;b"));
        assert!(!html.contains("<x>"));
    }
}
