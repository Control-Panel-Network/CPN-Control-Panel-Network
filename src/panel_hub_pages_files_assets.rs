//! Shared CSS/JS assets for File Manager UI.
//!
//! Served as `/server/files/assets/fm.css` and `fm.js` with a version query so
//! upgrades cache-bust without hashed filenames that 404 after RPM replace.

use crate::http_helpers::VERSION;

/// Bump when File Manager JS/CSS behavior changes (upgrade cache bust).
pub const FM_ASSET_REV: &str = "fm1";

pub fn fm_asset_version() -> String {
    format!("{VERSION}-{FM_ASSET_REV}")
}

pub fn fm_css_url() -> String {
    format!("/server/files/assets/fm.css?v={}", fm_asset_version())
}

pub fn fm_js_url() -> String {
    format!("/server/files/assets/fm.js?v={}", fm_asset_version())
}

pub fn fm_asset_tags() -> String {
    format!(
        r#"<link rel="stylesheet" href="{css}">"#,
        css = fm_css_url()
    )
}

/// Tiny inline boot: load versioned JS; if it 404s after upgrade, hard-reload once.
pub fn fm_inline_boot() -> String {
    let js = serde_json::to_string(&fm_js_url())
        .unwrap_or_else(|_| "\"/server/files/assets/fm.js\"".into());
    format!(
        r#"<script>
(function(){{
  var k='cpn-fm-asset-reload';
  var s=document.createElement('script');
  s.src={js};
  s.async=false;
  s.onerror=function(){{
    try {{
      if(!sessionStorage.getItem(k)){{
        sessionStorage.setItem(k,'1');
        location.reload(true);
      }}
    }} catch(e) {{}}
  }};
  s.onload=function(){{ try {{ sessionStorage.removeItem(k); }} catch(e) {{}} }};
  document.head.appendChild(s);
}})();
</script>"#,
        js = js
    )
}

pub fn fm_css_body() -> &'static str {
    include_str!("../assets/fm.css")
}

pub fn fm_js_body() -> &'static str {
    include_str!("../assets/fm.js")
}
