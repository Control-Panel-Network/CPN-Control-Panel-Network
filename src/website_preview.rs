//! Website preview helpers: public URLs, SSRF guards, and docroot path mapping.
//!
//! Preview mode prefers same-origin files under `/preview/<domain>/content/…`
//! so sites that set X-Frame-Options can still be framed from the panel.
//! Visit links open the real public URL in a new tab.

use crate::sites::normalize_domain;
use std::net::IpAddr;
use std::path::{Component, Path, PathBuf};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// True when common TLS certificate files exist for this domain.
pub fn ssl_material_present(domain: &str) -> bool {
    let Ok(domain) = normalize_domain(domain) else {
        return false;
    };
    let candidates = [
        format!("/etc/letsencrypt/live/{domain}/fullchain.pem"),
        format!("/etc/letsencrypt/live/{domain}/cert.pem"),
        format!("/etc/ssl/cpn/{domain}.crt"),
        format!("/etc/ssl/cpn/{domain}/fullchain.pem"),
        format!("/var/lib/cpn/ssl/{domain}/fullchain.pem"),
    ];
    candidates.iter().any(|path| Path::new(path).is_file())
}

/// Prefer HTTPS when SSL material is known; otherwise HTTP for labs without certs.
pub fn public_site_url(domain_raw: &str) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let scheme = if ssl_material_present(&domain) {
        "https"
    } else {
        "http"
    };
    Ok(format!("{scheme}://{domain}"))
}

/// Pretty preview chrome URL for a managed domain.
pub fn preview_mode_url(domain_raw: &str) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    Ok(format!("/preview/{domain}/"))
}

/// Same-origin content URL served from the site docroot.
pub fn preview_content_url(domain_raw: &str, relative: &str) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let rel = relative.trim().trim_start_matches('/');
    if rel.is_empty() {
        Ok(format!("/preview/{domain}/content/"))
    } else {
        Ok(format!("/preview/{domain}/content/{rel}"))
    }
}

/// Hostnames that must never be fetched server-side (SSRF surface).
pub fn is_blocked_preview_host(host: &str) -> bool {
    let host = host.trim().trim_matches(|c| c == '[' || c == ']');
    if host.is_empty() {
        return true;
    }
    let lower = host.to_ascii_lowercase();
    if lower == "localhost"
        || lower == "localhost."
        || lower.ends_with(".localhost")
        || lower == "metadata.google.internal"
    {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip_is_non_public(ip);
    }
    false
}

fn ip_is_non_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.octets()[0] == 0
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0b1100_0000) == 0b0100_0000)
                || (v4.octets()[0] == 169 && v4.octets()[1] == 254)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| ip_is_non_public(IpAddr::V4(v4)))
        }
    }
}

/// Validate a candidate fetch URL: http(s) only, host equals owned domain, no
/// private IP literals. Callers must already enforce session ACL on `owned_domain`.
pub fn validate_preview_fetch_url(url: &str, owned_domain: &str) -> Result<(), String> {
    let owned = normalize_domain(owned_domain)?;
    let url = url.trim();
    if url.is_empty() {
        return Err("Preview URL is required".into());
    }
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| "Preview URL must include a scheme".to_string())?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err("Preview URL scheme must be http or https".into());
    }
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("").trim();
    if host_port.is_empty() {
        return Err("Preview URL host is required".into());
    }
    let host = host_port
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(host_port);
    let host = if host.starts_with('[') {
        host.trim_matches(|c| c == '[' || c == ']')
            .split('%')
            .next()
            .unwrap_or("")
    } else {
        host.split(':').next().unwrap_or(host)
    };
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return Err("Preview URL host is required".into());
    }
    if is_blocked_preview_host(&host) {
        return Err("Preview URL host is not allowed (private or local)".into());
    }
    if host.parse::<IpAddr>().is_ok() {
        return Err("Preview URL must use the site domain, not a raw IP".into());
    }
    if host != owned {
        return Err("Preview URL host must match the managed domain".into());
    }
    Ok(())
}

fn normalize_relative_segments(relative: &str) -> Result<PathBuf, String> {
    if relative.contains('\0') {
        return Err("Invalid path".into());
    }
    let mut out = PathBuf::new();
    for comp in Path::new(relative).components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(seg) => out.push(seg),
            Component::ParentDir => {
                return Err("Path traversal rejected".into());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("Absolute paths are not allowed in preview content".into());
            }
        }
    }
    Ok(out)
}

/// Map a request path under the site docroot. Rejects `..` and escapes.
pub fn resolve_under_docroot(docroot: &Path, relative: &str) -> Result<PathBuf, String> {
    let raw = relative.trim();
    if raw.starts_with('/') || raw.starts_with('\\') {
        return Err("Absolute paths are not allowed in preview content".into());
    }
    if raw.len() >= 2 && raw.as_bytes()[1] == b':' {
        return Err("Absolute paths are not allowed in preview content".into());
    }
    let rel = raw.trim_start_matches('/');
    let segments = normalize_relative_segments(rel)?;
    let joined = if segments.as_os_str().is_empty() {
        docroot.to_path_buf()
    } else {
        docroot.join(segments)
    };
    let doc_n = normalize_abs(docroot)?;
    let joined_n = normalize_abs(&joined)?;
    if !path_is_under(&joined_n, &doc_n) {
        return Err("Path escapes site document root".into());
    }
    Ok(joined_n)
}

fn normalize_abs(path: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err("Path traversal rejected".into());
                }
            }
            Component::Normal(seg) => out.push(seg),
        }
    }
    Ok(out)
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    let mut path_comps = path.components();
    for root_comp in root.components() {
        match path_comps.next() {
            Some(c) if c == root_comp => {}
            _ => return false,
        }
    }
    true
}

/// Guess a Content-Type for static preview files.
pub fn guess_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "txt" | "log" | "md" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Resolve index file under a directory for preview content.
pub fn resolve_index_file(dir: &Path) -> Option<PathBuf> {
    for name in ["index.html", "index.htm", "index.php"] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Minimal CPN chrome around a same-origin iframe of the site docroot.
pub fn preview_mode_html(
    domain_raw: &str,
    live_url: &str,
    content_src: &str,
) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let manage = format!("/websites/manage?domain={}", urlencoding_simple(&domain));
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Preview · {domain_e} · CPN</title>
  <style>
    :root {{
      --bg:#12141a; --panel:#1a1d26; --ink:#f2f4f7; --muted:#98a2b3;
      --accent:#3b82f6; --hairline:#2a2f3a;
    }}
    * {{ box-sizing:border-box; }}
    html, body {{ margin:0; height:100%; background:var(--bg); color:var(--ink);
      font-family:"Segoe UI", system-ui, sans-serif; }}
    .preview-shell {{ display:grid; grid-template-rows:52px 1fr; height:100%; }}
    .preview-bar {{
      display:flex; align-items:center; gap:12px; padding:0 14px;
      background:var(--panel); border-bottom:1px solid var(--hairline);
    }}
    .preview-back {{
      display:inline-flex; align-items:center; justify-content:center;
      width:34px; height:34px; border-radius:8px; border:1px solid var(--hairline);
      color:var(--ink); text-decoration:none; background:#222633; font-size:18px;
    }}
    .preview-brand {{ display:flex; flex-direction:column; min-width:0; flex:1; }}
    .preview-brand strong {{ font-size:14px; letter-spacing:-.01em; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }}
    .preview-brand span {{ font-size:11px; color:var(--muted); }}
    .preview-actions {{ display:flex; align-items:center; gap:8px; }}
    .preview-visit {{
      display:inline-flex; align-items:center; min-height:34px; padding:0 12px;
      border-radius:999px; background:#222633; color:var(--ink); text-decoration:none;
      font-size:12px; font-weight:700; border:1px solid var(--hairline);
    }}
    .preview-eye {{
      display:inline-flex; align-items:center; justify-content:center;
      width:40px; height:40px; border-radius:999px; border:0; cursor:pointer;
      background:var(--accent); color:#fff; font-size:14px; font-weight:800;
    }}
    .preview-frame-wrap {{
      margin:12px; border-radius:14px; overflow:hidden; border:1px solid var(--hairline);
      background:#0b0d12; min-height:0;
    }}
    .preview-frame-wrap.is-focus {{
      position:fixed; inset:0; margin:0; border-radius:0; border:0; z-index:20;
    }}
    iframe {{ width:100%; height:100%; border:0; background:#fff; display:block; min-height:calc(100vh - 76px); }}
    .preview-frame-wrap.is-focus iframe {{ min-height:100vh; }}
  </style>
</head>
<body>
  <div class="preview-shell">
    <header class="preview-bar">
      <a class="preview-back" href="{manage}" title="Back to Manage" aria-label="Back to Manage">&#8249;</a>
      <div class="preview-brand">
        <strong>{domain_e}</strong>
        <span>CPN Preview Mode</span>
      </div>
      <div class="preview-actions">
        <a class="preview-visit" href="{live}" target="_blank" rel="noopener noreferrer">Visit live site</a>
        <button type="button" class="preview-eye" id="preview-focus" title="Toggle focus" aria-label="Toggle focus frame">Eye</button>
      </div>
    </header>
    <div class="preview-frame-wrap" id="preview-wrap">
      <iframe title="Preview of {domain_e}" src="{content}" referrerpolicy="no-referrer"></iframe>
    </div>
  </div>
  <script>
  (function () {{
    var btn = document.getElementById('preview-focus');
    var wrap = document.getElementById('preview-wrap');
    if (!btn || !wrap) return;
    btn.addEventListener('click', function () {{
      wrap.classList.toggle('is-focus');
    }});
  }})();
  </script>
</body>
</html>"#,
        domain_e = html_escape(&domain),
        manage = html_escape(&manage),
        live = html_escape(live_url),
        content = html_escape(content_src),
    ))
}

fn urlencoding_simple(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn public_url_defaults_to_http_without_certs() {
        let url = public_site_url("cpn-lab-test.example").unwrap();
        assert_eq!(url, "http://cpn-lab-test.example");
    }

    #[test]
    fn rejects_private_ip_hosts() {
        assert!(is_blocked_preview_host("127.0.0.1"));
        assert!(is_blocked_preview_host("10.0.0.5"));
        assert!(is_blocked_preview_host("192.168.1.1"));
        assert!(is_blocked_preview_host("169.254.169.254"));
        assert!(is_blocked_preview_host("localhost"));
        assert!(is_blocked_preview_host("::1"));
        assert!(!is_blocked_preview_host("example.com"));
    }

    #[test]
    fn fetch_url_must_match_owned_domain() {
        assert!(validate_preview_fetch_url("http://example.com/", "example.com").is_ok());
        assert!(validate_preview_fetch_url("https://example.com/path", "example.com").is_ok());
        assert!(validate_preview_fetch_url("http://evil.com/", "example.com").is_err());
        assert!(validate_preview_fetch_url("http://127.0.0.1/", "example.com").is_err());
        assert!(validate_preview_fetch_url("file:///etc/passwd", "example.com").is_err());
        assert!(validate_preview_fetch_url("http://10.1.2.3/", "example.com").is_err());
        assert!(validate_preview_fetch_url("http://example.com:8080/", "example.com").is_ok());
    }

    #[test]
    fn docroot_path_rejects_traversal() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cpn-preview-doc-{stamp}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("public_html")).unwrap();
        fs::write(root.join("public_html/index.html"), b"ok").unwrap();
        let doc = root.join("public_html");
        assert!(resolve_under_docroot(&doc, "index.html").is_ok());
        assert!(resolve_under_docroot(&doc, "../secret").is_err());
        assert!(resolve_under_docroot(&doc, "/etc/passwd").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn preview_mode_html_has_controls() {
        let html = preview_mode_html(
            "docs.example.com",
            "https://docs.example.com",
            "/preview/docs.example.com/content/",
        )
        .unwrap();
        assert!(html.contains("Back to Manage"));
        assert!(html.contains("Visit live site"));
        assert!(html.contains("/preview/docs.example.com/content/"));
        assert!(html.contains("rel=\"noopener noreferrer\""));
        assert!(!html.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn preview_urls_are_pretty() {
        assert_eq!(
            preview_mode_url("Blog.Example.COM").unwrap(),
            "/preview/blog.example.com/"
        );
        assert_eq!(
            preview_content_url("blog.example.com", "css/app.css").unwrap(),
            "/preview/blog.example.com/content/css/app.css"
        );
    }
}
