//! Registry update column HTML for Docker Active Containers.

use crate::panel_ops_docker_image_updates::{ImageUpdateSnapshot, UpdateCheckState, CACHE_TTL_SECS};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn update_status_cell(snap: &ImageUpdateSnapshot) -> String {
    let (label, class) = match snap.state {
        UpdateCheckState::UpToDate => ("Up to date", "docker-update-ok"),
        UpdateCheckState::UpdateAvailable => ("Update available", "docker-update-warn"),
        UpdateCheckState::Unknown => ("Unknown", "docker-update-unknown"),
        UpdateCheckState::NotChecked => ("Not checked", "docker-update-muted"),
    };
    let title = html_escape(&snap.detail);
    format!(
        r#"<span class="docker-update-pill {class}" title="{title}">{label}</span>"#,
        class = class,
        title = title,
        label = label,
    )
}

pub fn update_styles() -> &'static str {
    r#"
.docker-update-pill{display:inline-block;padding:2px 8px;border-radius:999px;font-size:12px;font-weight:600;line-height:1.4;border:1px solid transparent;}
.docker-update-ok{color:#067647;background:#ecfdf3;border-color:#abefc6;}
.docker-update-warn{color:#b54708;background:#fffaeb;border-color:#fedf89;}
.docker-update-unknown{color:#344054;background:#f2f4f7;border-color:#e4e7ec;}
.docker-update-muted{color:#667085;background:transparent;border-color:#e4e7ec;font-weight:500;}
"#
}

pub fn check_updates_button_href(
    base_action: &str,
    search_name: &str,
    q: &str,
    page: usize,
    per_page: usize,
) -> String {
    let parts = [
        "check_updates=1".to_string(),
        format!("{search_name}={}", urlencoding_simple(q)),
        format!("page={page}"),
        format!("per_page={per_page}"),
    ];
    format!("{base_action}?{}", parts.join("&amp;"))
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

pub fn cache_help_text() -> String {
    format!(
        "Image update status compares local and registry digests. Results cache for about {} hours; use <strong>Check for updates</strong> to refresh. Pull images from <a href=\"/docker/images\">Manage Images</a> or recreate stacks under <a href=\"/docker/stacks\">Compose Stacks</a>.",
        CACHE_TTL_SECS / 3600
    )
}