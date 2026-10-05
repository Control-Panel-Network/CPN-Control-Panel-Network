//! Fetch the live public origin for Preview when the local docroot is a stub.

use crate::website_preview::{guess_content_type, validate_preview_fetch_url};
use crate::website_preview_stub::is_public_internet_host;
use std::path::Path;
use std::process::{Command, Stdio};

const FETCH_TIMEOUT_SECS: u64 = 8;
const FETCH_MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Public origin used for live Preview and Site preview captures.
pub fn live_public_origin(domain: &str) -> Result<String, String> {
    let domain = crate::sites::normalize_domain(domain)?;
    if is_public_internet_host(&domain) {
        Ok(format!("https://{domain}"))
    } else {
        crate::website_preview::public_site_url(&domain)
    }
}

/// Build a same-host live URL for `relative` under the owned domain.
pub fn live_public_url(domain: &str, relative: &str) -> Result<String, String> {
    let origin = live_public_origin(domain)?;
    let rel = relative.trim().trim_start_matches('/');
    let origin = origin.trim_end_matches('/');
    let url = if rel.is_empty() {
        format!("{origin}/")
    } else {
        format!("{origin}/{rel}")
    };
    validate_preview_fetch_url(&url, domain)?;
    Ok(url)
}

pub struct LiveFetch {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// GET the live public URL with a short timeout (SSRF: owned domain only).
pub fn fetch_live_origin(domain: &str, relative: &str) -> Result<LiveFetch, String> {
    let url = live_public_url(domain, relative)?;
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--max-redirs",
            "4",
            "--max-time",
            &FETCH_TIMEOUT_SECS.to_string(),
            "--max-filesize",
            &FETCH_MAX_BYTES.to_string(),
            "--proto-redir",
            "=https,http",
            "--compressed",
            "-A",
            "cpn-site-preview",
            "-D",
            "-",
            "--output",
            "-",
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("Could not fetch live site: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail: String = stderr.trim().chars().take(120).collect();
        if detail.is_empty() {
            return Err("Could not fetch the live site (timeout or DNS)".into());
        }
        return Err(format!("Could not fetch the live site ({detail})"));
    }
    let (headers, body) = split_curl_headers(&output.stdout)?;
    if body.len() as u64 > FETCH_MAX_BYTES {
        return Err("Live site response exceeds size cap".into());
    }
    let header_ctype = content_type_from_headers(&headers);
    let path_guess = guess_content_type(Path::new(relative));
    let mut ctype = header_ctype.unwrap_or_else(|| {
        if relative.trim().is_empty() || relative.ends_with('/') {
            "text/html; charset=utf-8".into()
        } else {
            path_guess.to_string()
        }
    });
    let mut bytes = body;
    if ctype.to_ascii_lowercase().contains("text/html")
        && let Ok(text) = std::str::from_utf8(&bytes)
    {
        let origin = live_public_origin(domain)?;
        bytes = inject_live_base(text, &origin).into_bytes();
        ctype = "text/html; charset=utf-8".into();
    }
    Ok(LiveFetch {
        bytes,
        content_type: ctype,
    })
}

/// Friendly page when the live origin cannot be loaded (never serve the stub).
pub fn live_fetch_error_html(domain: &str, live_url: &str, err: &str) -> String {
    let domain_e = html_escape(domain);
    let live_e = html_escape(live_url);
    let err_e = html_escape(err);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Live preview unavailable · {domain_e}</title>
  <style>
    body {{ font-family: system-ui, Segoe UI, sans-serif; margin: 32px; color: #101828; background: #fff; }}
    h1 {{ font-size: 1.35rem; }}
    p {{ line-height: 1.5; max-width: 40rem; }}
    a {{ color: #155eef; }}
  </style>
</head>
<body>
  <h1>Live preview unavailable</h1>
  <p>The local document root is still the CPN placeholder or has no index file. Preview tried the public site and could not load it.</p>
  <p>{err_e}</p>
  <p><a href="{live_e}" target="_blank" rel="noopener noreferrer">Open {domain_e} in a new tab</a></p>
</body>
</html>"#
    )
}

fn inject_live_base(html: &str, origin: &str) -> String {
    let href = origin.trim_end_matches('/');
    let tag = format!(r#"<base href="{href}/">"#);
    if html.to_ascii_lowercase().contains("<base ") {
        return html.to_string();
    }
    if let Some(idx) = html.to_ascii_lowercase().find("<head")
        && let Some(end) = html[idx..].find('>')
    {
        let at = idx + end + 1;
        let mut out = String::with_capacity(html.len() + tag.len() + 1);
        out.push_str(&html[..at]);
        out.push_str(&tag);
        out.push_str(&html[at..]);
        return out;
    }
    format!("{tag}{html}")
}

fn split_curl_headers(raw: &[u8]) -> Result<(String, Vec<u8>), String> {
    // curl -D - --output - concatenates headers then body. Take the last HTTP header block.
    let text = String::from_utf8_lossy(raw);
    let mut last_header_start = None;
    let bytes = raw;
    let mut i = 0usize;
    while i + 4 < bytes.len() {
        if bytes[i] == b'H'
            && bytes
                .get(i..i + 5)
                .map(|s| s.eq_ignore_ascii_case(b"HTTP/"))
                == Some(true)
        {
            last_header_start = Some(i);
        }
        i += 1;
        if i > 64 * 1024 {
            break;
        }
    }
    let start = last_header_start.unwrap_or(0);
    let rest = &bytes[start..];
    let sep = rest
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .or_else(|| rest.windows(2).position(|w| w == b"\n\n").map(|p| p + 2));
    let Some(sep) = sep else {
        return Ok((text.into_owned(), Vec::new()));
    };
    let headers = String::from_utf8_lossy(&rest[..sep]).into_owned();
    Ok((headers, rest[sep..].to_vec()))
}

fn content_type_from_headers(headers: &str) -> Option<String> {
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-type:") {
            let original = line.split_once(':').map(|(_, v)| v.trim()).unwrap_or(rest);
            return Some(original.trim().to_string());
        }
    }
    None
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_origin_uses_https() {
        assert_eq!(
            live_public_origin("cmstest.newstargeted.com").unwrap(),
            "https://cmstest.newstargeted.com"
        );
    }

    #[test]
    fn rejects_off_domain_relative_as_url() {
        assert!(live_public_url("example.com", "").is_ok());
        assert!(validate_preview_fetch_url("https://evil.com/", "example.com").is_err());
    }

    #[test]
    fn injects_base_after_head() {
        let html = "<html><head><title>x</title></head><body>ok</body></html>";
        let out = inject_live_base(html, "https://cmstest.newstargeted.com");
        assert!(out.contains(r#"<base href="https://cmstest.newstargeted.com/">"#));
        assert!(out.contains("<title>x</title>"));
    }

    #[test]
    fn error_page_is_not_stub() {
        let html = live_fetch_error_html(
            "cmstest.newstargeted.com",
            "https://cmstest.newstargeted.com/",
            "timeout",
        );
        assert!(!crate::website_preview_stub::html_looks_like_placeholder(
            &html
        ));
        assert!(html.contains("Live preview unavailable"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }
}
