//! Owner-editable panel error / forbidden messages (Markdown).
//!
//! Store: `$CPN_DATA_DIR/panel-error-messages.json` (mode 600).

use crate::account::{data_dir, now_unix};
use crate::panel_markdown::render_safe_markdown;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const SCHEMA_VERSION: u32 = 1;
const MAX_CHARS: usize = 12_000;

pub const BUILTIN_FORBIDDEN: &str = "You do not have permission to view this page.";
pub const BUILTIN_NOT_FOUND: &str = "The page you requested was not found.";
pub const BUILTIN_INTERNAL: &str =
    "Something went wrong on the panel. Please try again or contact the server owner.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelErrorMessages {
    pub schema_version: u32,
    pub forbidden_md: String,
    pub not_found_md: String,
    pub internal_md: String,
    pub updated_at_unix: u64,
}

impl Default for PanelErrorMessages {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            forbidden_md: BUILTIN_FORBIDDEN.to_string(),
            not_found_md: BUILTIN_NOT_FOUND.to_string(),
            internal_md: BUILTIN_INTERNAL.to_string(),
            updated_at_unix: 0,
        }
    }
}

fn path() -> PathBuf {
    data_dir().join("panel-error-messages.json")
}

fn write_mode_600(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(contents)
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn sanitize_md(raw: &str, label: &str) -> Result<String, String> {
    if raw.chars().count() > MAX_CHARS {
        return Err(format!("{label} is too long (max {MAX_CHARS} characters)"));
    }
    if raw.chars().any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
        return Err(format!("{label} cannot include control characters"));
    }
    Ok(raw.to_string())
}

pub fn load_messages() -> PanelErrorMessages {
    let Ok(raw) = fs::read_to_string(path()) else {
        return PanelErrorMessages::default();
    };
    let mut loaded: PanelErrorMessages = serde_json::from_str(&raw).unwrap_or_default();
    if loaded.forbidden_md.trim().is_empty() {
        loaded.forbidden_md = BUILTIN_FORBIDDEN.to_string();
    }
    if loaded.not_found_md.trim().is_empty() {
        loaded.not_found_md = BUILTIN_NOT_FOUND.to_string();
    }
    if loaded.internal_md.trim().is_empty() {
        loaded.internal_md = BUILTIN_INTERNAL.to_string();
    }
    loaded.schema_version = SCHEMA_VERSION;
    loaded
}

pub fn save_messages(messages: &PanelErrorMessages) -> Result<(), String> {
    let mut out = messages.clone();
    out.schema_version = SCHEMA_VERSION;
    out.updated_at_unix = now_unix();
    out.forbidden_md = sanitize_md(&out.forbidden_md, "Forbidden message")?;
    out.not_found_md = sanitize_md(&out.not_found_md, "Not found message")?;
    out.internal_md = sanitize_md(&out.internal_md, "Internal error message")?;
    let json = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize error messages: {e}"))?;
    write_mode_600(&path(), json.as_bytes())
}

pub fn restore_forbidden() -> Result<(), String> {
    let mut m = load_messages();
    m.forbidden_md = BUILTIN_FORBIDDEN.to_string();
    save_messages(&m)
}

pub fn restore_not_found() -> Result<(), String> {
    let mut m = load_messages();
    m.not_found_md = BUILTIN_NOT_FOUND.to_string();
    save_messages(&m)
}

pub fn restore_internal() -> Result<(), String> {
    let mut m = load_messages();
    m.internal_md = BUILTIN_INTERNAL.to_string();
    save_messages(&m)
}

pub fn restore_all_builtins() -> Result<(), String> {
    save_messages(&PanelErrorMessages::default())
}

fn shell_error_page(title: &str, status_label: &str, md: &str) -> String {
    let body = render_safe_markdown(md);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <style>
    :root {{ color-scheme: dark light; }}
    body {{
      margin:0; min-height:100vh; display:grid; place-items:center;
      font-family: system-ui, Segoe UI, sans-serif; background:#0b1220; color:#e2e8f0;
    }}
    main {{
      width:min(40rem, 92vw); padding:28px 24px; border-radius:14px;
      border:1px solid #334155; background:#111827;
    }}
    .eyebrow {{ font-size:12px; letter-spacing:.06em; text-transform:uppercase; color:#94a3b8; margin:0 0 8px; }}
    h1 {{ margin:0 0 14px; font-size:1.45rem; }}
    .md :is(p, ul, ol, pre, blockquote, table) {{ margin:0.55em 0; }}
    .md a {{ color:#7dd3fc; }}
    .actions {{ margin-top:18px; }}
    .actions a {{ color:#93c5fd; }}
  </style>
</head>
<body>
  <main>
    <p class="eyebrow">CPN Panel</p>
    <h1>{status}</h1>
    <div class="md">{body}</div>
    <p class="actions"><a href="/dashboard">Back to dashboard</a></p>
  </main>
</body>
</html>"#,
        title = html_escape(title),
        status = html_escape(status_label),
        body = body,
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn forbidden_page_html() -> String {
    let m = load_messages();
    shell_error_page("403 Forbidden", "403 Forbidden", &m.forbidden_md)
}

pub fn not_found_page_html() -> String {
    let m = load_messages();
    shell_error_page("404 Not Found", "404 Not Found", &m.not_found_md)
}

pub fn internal_page_html() -> String {
    let m = load_messages();
    shell_error_page("500 Internal Error", "500 Internal Error", &m.internal_md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn round_trip_and_restore() {
        with_test_data_dir(|| {
            let mut m = load_messages();
            m.forbidden_md = "**Nope**".into();
            save_messages(&m).unwrap();
            assert!(load_messages().forbidden_md.contains("Nope"));
            restore_forbidden().unwrap();
            assert_eq!(load_messages().forbidden_md, BUILTIN_FORBIDDEN);
            let html = forbidden_page_html();
            assert!(html.contains("403 Forbidden"));
            assert!(!html.to_lowercase().contains("cyberpanel"));
            assert!(!html.contains('\u{2014}'));
        });
    }
}
