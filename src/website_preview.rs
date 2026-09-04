//! Website preview helpers for Manage and Websites list.
//!
//! Preview uses a sandboxed iframe of the public site URL (no external
//! screenshot APIs). Server-side URL helpers reject private IP literals so any
//! future fetch stays SSRF-safe. Visit links prefer HTTPS when SSL material is
//! present on disk; otherwise HTTP (lab-friendly).

use crate::sites::normalize_domain;
use std::net::IpAddr;
use std::path::Path;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewSize {
    Manage,
    List,
}

/// Thumbnail + Visit Site block (iframe with embedding fallback).
pub fn preview_card_html(domain_raw: &str, size: PreviewSize) -> Result<String, String> {
    let domain = normalize_domain(domain_raw)?;
    let url = public_site_url(&domain)?;
    // Defense in depth: same guards used for any future server fetch.
    validate_preview_fetch_url(&url, &domain)?;
    let domain_e = html_escape(&domain);
    let url_e = html_escape(&url);
    let size_class = match size {
        PreviewSize::Manage => "site-preview--manage",
        PreviewSize::List => "site-preview--list",
    };
    let ssl_badge = if ssl_material_present(&domain) {
        r#"<span class="site-preview-ssl" title="TLS certificate found on this host">SECURE</span>"#
    } else {
        ""
    };
    Ok(format!(
        r#"<div class="site-preview {size_class}" data-site-preview data-preview-url="{url_e}">
  <div class="site-preview-viewport" aria-hidden="false">
    <iframe class="site-preview-frame" title="Preview of {domain_e}" sandbox="allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox" referrerpolicy="no-referrer" loading="lazy" src="{url_e}"></iframe>
    <div class="site-preview-fallback" hidden>
      <div class="site-preview-fallback-inner">
        <strong>{domain_e}</strong>
        <p>Preview unavailable (site blocks embedding).</p>
      </div>
    </div>
  </div>
  <div class="site-preview-actions">
    <a class="site-preview-visit" href="{url_e}" target="_blank" rel="noopener noreferrer">Visit Site</a>
    {ssl_badge}
  </div>
</div>"#
    ))
}

/// Compact expandable preview for the Websites list domain cell.
pub fn list_preview_cell_html(domain_raw: &str) -> String {
    match preview_card_html(domain_raw, PreviewSize::List) {
        Ok(card) => format!(
            r#"<details class="site-preview-details">
  <summary>Preview</summary>
  {card}
</details>"#
        ),
        Err(_) => String::new(),
    }
}

/// CSS for Manage/list website preview cards (injected into panel shell).
pub fn preview_styles() -> &'static str {
    r#"
.site-manage-layout {
  display:grid; grid-template-columns:minmax(220px,280px) minmax(0,1fr); gap:22px; align-items:start; margin-top:14px;
}
.site-preview-col { min-width:0; }
.site-manage-details { min-width:0; }
.site-preview { display:flex; flex-direction:column; gap:10px; }
.site-preview-viewport {
  position:relative; overflow:hidden; border:1px solid var(--hairline); border-radius:12px;
  background:#f8fafc; aspect-ratio:16/10;
}
.site-preview--list .site-preview-viewport { aspect-ratio:16/9; max-width:240px; }
.site-preview-frame {
  position:absolute; inset:0; width:400%; height:400%; border:0;
  transform:scale(0.25); transform-origin:0 0; background:#fff; pointer-events:none;
}
.site-preview-frame.is-blocked { opacity:0; }
.site-preview-fallback {
  position:absolute; inset:0; display:flex; align-items:center; justify-content:center;
  padding:16px; background:linear-gradient(160deg,#f8fafc,#eef2f6); text-align:center;
}
.site-preview-fallback[hidden] { display:none !important; }
.site-preview-fallback-inner strong { display:block; color:var(--ink); font-size:14px; }
.site-preview-fallback-inner p { margin:8px 0 0; color:var(--muted); font-size:12px; max-width:28ch; }
.site-preview-actions { display:flex; flex-wrap:wrap; align-items:center; gap:8px; }
.site-preview-visit {
  display:inline-flex; align-items:center; justify-content:center; min-height:36px; padding:0 12px;
  border-radius:999px; background:#f2f4f7; color:#344054; font-weight:700; font-size:13px; text-decoration:none;
}
.site-preview-visit--inline { min-height:28px; padding:0 10px; font-size:12px; }
.site-preview-ssl {
  display:inline-flex; align-items:center; min-height:24px; padding:0 8px; border-radius:999px;
  background:#ecfdf3; color:#067647; font-size:11px; font-weight:800; letter-spacing:.04em;
}
.site-list-preview-row { display:flex; flex-wrap:wrap; align-items:center; gap:8px; margin-top:8px; }
.site-preview-details { margin-top:6px; }
.site-preview-details > summary {
  cursor:pointer; color:var(--muted); font-size:12px; font-weight:600; list-style:none;
}
.site-preview-details > summary::-webkit-details-marker { display:none; }
.site-preview-details[open] > summary { margin-bottom:8px; }
@media (max-width:1023.98px) {
  .site-manage-layout { grid-template-columns:1fr; }
}
"#
}

/// Client script: show fallback when iframe embed is blocked or unreachable.
pub fn preview_script() -> &'static str {
    r#"
<script>
(function () {
  document.querySelectorAll('[data-site-preview]').forEach(function (root) {
    var iframe = root.querySelector('.site-preview-frame');
    var fallback = root.querySelector('.site-preview-fallback');
    if (!iframe || !fallback) return;
    var settled = false;
    function showFallback(message) {
      if (settled) return;
      settled = true;
      if (message) {
        var copy = fallback.querySelector('p');
        if (copy) copy.textContent = message;
      }
      fallback.hidden = false;
      iframe.classList.add('is-blocked');
    }
    function markOk() {
      if (settled) return;
      settled = true;
      fallback.hidden = true;
      iframe.classList.remove('is-blocked');
    }
    iframe.addEventListener('error', function () {
      showFallback('Preview unavailable (site unreachable).');
    });
    iframe.addEventListener('load', function () {
      try {
        var doc = iframe.contentDocument;
        if (doc && doc.location && String(doc.location.href) === 'about:blank') {
          showFallback('Preview unavailable (site blocks embedding).');
          return;
        }
      } catch (err) {
        // Cross-origin embed loaded; treat as success.
      }
      markOk();
    });
    window.setTimeout(function () {
      if (!settled) {
        showFallback('Preview unavailable (site blocks embedding or did not load).');
      }
    }, 5000);
  });
})();
</script>
"#
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn preview_card_includes_visit_and_iframe() {
        let html = preview_card_html("docs.example.com", PreviewSize::Manage).unwrap();
        assert!(html.contains("Visit Site"));
        assert!(html.contains("rel=\"noopener noreferrer\""));
        assert!(html.contains("target=\"_blank\""));
        assert!(html.contains("sandbox="));
        assert!(html.contains("http://docs.example.com"));
        assert!(html.contains("data-site-preview"));
    }
}
