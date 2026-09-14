//! Host/Origin allowlist for `--allow-remote` CSRF-style checks.
//!
//! Loopback spellings (`localhost`, `127.0.0.1`, `::1`) are treated as equivalent
//! for the same port so lab NAT access works from either URL.

use actix_web::HttpRequest;

/// Build Origin/Host allowlist from bind port and server-known addresses.
/// Never trusts the client-supplied `Host` header (issue #1).
pub fn build_allowed_hosts(bind_port: u16, configured_hosts: &[String]) -> Vec<String> {
    let mut allowed = Vec::new();
    push_allowed_authority(&mut allowed, &format!("127.0.0.1:{bind_port}"));
    for host in configured_hosts {
        let raw = host.trim();
        if raw.is_empty() || raw == "0.0.0.0" || raw == "::" || raw == "*" {
            continue;
        }
        let entry = if raw.starts_with('[') {
            if raw.contains("]:") {
                raw.to_string()
            } else {
                format!("{raw}:{bind_port}")
            }
        } else if raw.matches(':').count() >= 2 {
            format!("[{raw}]:{bind_port}")
        } else if let Some((name, port)) = raw.rsplit_once(':') {
            if !name.is_empty() && port.parse::<u16>().is_ok() {
                raw.to_string()
            } else {
                format!("{raw}:{bind_port}")
            }
        } else {
            format!("{raw}:{bind_port}")
        };
        push_allowed_authority(&mut allowed, &entry);
    }
    allowed
}

fn extract_authority(urlish: &str) -> Option<String> {
    let trimmed = urlish.trim();
    let rest = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);
    let authority = rest.split('/').next()?.trim();
    if authority.is_empty() {
        return None;
    }
    Some(authority.to_string())
}

fn is_loopback_hostname(host: &str) -> bool {
    let host = host.trim().trim_matches(|c| c == '[' || c == ']');
    matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1" | "0:0:0:0:0:0:0:1"
    )
}

/// Split `host:port`, `[ipv6]:port`, or bare host into (hostname, optional port).
fn split_authority_host_port(authority: &str) -> Option<(String, Option<u16>)> {
    let authority = authority.trim();
    if authority.is_empty() {
        return None;
    }
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        if after.is_empty() {
            return Some((host.to_string(), None));
        }
        let port = after.strip_prefix(':')?.parse::<u16>().ok()?;
        return Some((host.to_string(), Some(port)));
    }
    if authority.matches(':').count() >= 2 {
        // Bare IPv6 without brackets.
        return Some((authority.to_string(), None));
    }
    if let Some((host, port)) = authority.rsplit_once(':') {
        if !host.is_empty()
            && let Ok(port) = port.parse::<u16>()
        {
            return Some((host.to_string(), Some(port)));
        }
    }
    Some((authority.to_string(), None))
}

fn loopback_authority_aliases(port: Option<u16>) -> Vec<String> {
    let suffix = match port {
        Some(p) => format!(":{p}"),
        None => String::new(),
    };
    vec![
        format!("127.0.0.1{suffix}"),
        format!("localhost{suffix}"),
        format!("[::1]{suffix}"),
        format!("::1{suffix}"),
    ]
}

fn push_allowed_authority(allowed: &mut Vec<String>, authority: &str) {
    let authority = authority.trim();
    if authority.is_empty() {
        return;
    }
    let entries = match split_authority_host_port(authority) {
        Some((host, port)) if is_loopback_hostname(&host) => loopback_authority_aliases(port),
        _ => vec![authority.to_string()],
    };
    for entry in entries {
        if !allowed
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&entry))
        {
            allowed.push(entry);
        }
    }
}

fn authorities_equivalent(left: &str, right: &str) -> bool {
    if left.eq_ignore_ascii_case(right) {
        return true;
    }
    let Some((left_host, left_port)) = split_authority_host_port(left) else {
        return false;
    };
    let Some((right_host, right_port)) = split_authority_host_port(right) else {
        return false;
    };
    if left_port != right_port {
        return false;
    }
    is_loopback_hostname(&left_host) && is_loopback_hostname(&right_host)
}

fn authority_allowed(authority: &str, allowed_hosts: &[String]) -> bool {
    let authority = authority.trim();
    allowed_hosts
        .iter()
        .any(|host| authorities_equivalent(authority, host))
}

/// True when Origin/Referer authority matches the server-configured allowlist.
pub fn origin_matches_allowed(candidate: &str, allowed_hosts: &[String]) -> bool {
    let Some(authority) = extract_authority(candidate) else {
        return false;
    };
    authority_allowed(&authority, allowed_hosts)
}

fn request_host_header(request: &HttpRequest) -> Option<String> {
    request
        .headers()
        .get(actix_web::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn host_header_allowed(request: &HttpRequest, allowed_hosts: &[String]) -> bool {
    let Some(host_hdr) = request_host_header(request) else {
        return false;
    };
    authority_allowed(&host_hdr, allowed_hosts)
}

/// True when Origin/Referer matches the request Host (same-origin page that served the form).
fn origin_matches_request_host(candidate: &str, request: &HttpRequest) -> bool {
    let Some(host_hdr) = request_host_header(request) else {
        return false;
    };
    let Some(authority) = extract_authority(candidate) else {
        return false;
    };
    authorities_equivalent(&authority, &host_hdr)
}

fn origin_or_same_host_ok(candidate: &str, request: &HttpRequest, allowed_hosts: &[String]) -> bool {
    origin_matches_allowed(candidate, allowed_hosts)
        || origin_matches_request_host(candidate, request)
}

/// Merge live `panel_public_url` (NAT labs, reverse proxies) into the Host/Origin allowlist.
pub fn extend_allowed_hosts_with_public_url(allowed_hosts: &mut Vec<String>) {
    let Some(url) = crate::panel_public_url::load_panel_public_url() else {
        return;
    };
    let Some(authority) = extract_authority(&url) else {
        return;
    };
    push_allowed_authority(allowed_hosts, &authority);
}

fn effective_allowed_hosts(allowed_hosts: &[String]) -> Vec<String> {
    let mut hosts = allowed_hosts.to_vec();
    extend_allowed_hosts_with_public_url(&mut hosts);
    hosts
}

/// When listening on 0.0.0.0, reject unexpected Host and cross-site Origin/Referer.
pub fn remote_origin_ok(
    request: &HttpRequest,
    allow_remote: bool,
    allowed_hosts: &[String],
) -> bool {
    if !allow_remote {
        return true;
    }
    let allowed = effective_allowed_hosts(allowed_hosts);
    if !host_header_allowed(request, &allowed) {
        return false;
    }
    let method = request.method().as_str();
    if matches!(method, "GET" | "HEAD" | "OPTIONS") {
        return true;
    }
    let origin = request
        .headers()
        .get(actix_web::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let referer = request
        .headers()
        .get(actix_web::http::header::REFERER)
        .and_then(|value| value.to_str().ok());
    let candidate = origin.or(referer);
    let Some(candidate) = candidate else {
        return true;
    };
    origin_or_same_host_ok(candidate, request, &allowed)
}

/// Origin check for WebSocket upgrades when `--allow-remote` is set (issue #1).
pub fn websocket_origin_ok(
    request: &HttpRequest,
    allow_remote: bool,
    allowed_hosts: &[String],
) -> bool {
    if !allow_remote {
        return true;
    }
    let allowed = effective_allowed_hosts(allowed_hosts);
    if !host_header_allowed(request, &allowed) {
        return false;
    }
    let origin = request
        .headers()
        .get(actix_web::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let Some(origin) = origin else {
        return true;
    };
    origin_or_same_host_ok(origin, request, &allowed)
}

#[cfg(test)]
mod tests {
    use super::{
        build_allowed_hosts, origin_matches_allowed, remote_origin_ok, websocket_origin_ok,
    };
    use actix_web::test::TestRequest;

    #[test]
    fn origin_allowlist_accepts_loopback_not_attacker_pair() {
        let allowed = build_allowed_hosts(2087, &["10.0.0.5".into()]);
        assert!(origin_matches_allowed("http://127.0.0.1:2087/", &allowed));
        assert!(origin_matches_allowed("http://10.0.0.5:2087", &allowed));
        assert!(!origin_matches_allowed(
            "http://attacker.example:2087",
            &allowed
        ));
    }

    #[test]
    fn remote_rejects_attacker_host_and_origin() {
        let allowed = build_allowed_hosts(2087, &["192.168.1.10".into()]);
        let req = TestRequest::default()
            .method(actix_web::http::Method::POST)
            .insert_header((actix_web::http::header::HOST, "attacker.example:2087"))
            .insert_header((
                actix_web::http::header::ORIGIN,
                "http://attacker.example:2087",
            ))
            .to_http_request();
        assert!(!remote_origin_ok(&req, true, &allowed));
        assert!(!websocket_origin_ok(&req, true, &allowed));
    }

    #[test]
    fn remote_accepts_configured_host_origin() {
        let allowed = build_allowed_hosts(2087, &["192.168.1.10".into()]);
        let req = TestRequest::default()
            .method(actix_web::http::Method::POST)
            .insert_header((actix_web::http::header::HOST, "192.168.1.10:2087"))
            .insert_header((actix_web::http::header::ORIGIN, "http://192.168.1.10:2087"))
            .to_http_request();
        assert!(remote_origin_ok(&req, true, &allowed));
        assert!(websocket_origin_ok(&req, true, &allowed));
    }

    #[test]
    fn remote_accepts_panel_public_url_nat_host() {
        use crate::account::with_test_data_dir;
        with_test_data_dir(|| {
            crate::panel_public_url::save_panel_public_url("http://127.0.0.1:2090").unwrap();
            // Bind is guest 2087; host NAT uses 2090 via panel_public_url.
            let allowed = build_allowed_hosts(2087, &[]);
            let req = TestRequest::default()
                .method(actix_web::http::Method::POST)
                .insert_header((actix_web::http::header::HOST, "127.0.0.1:2090"))
                .insert_header((actix_web::http::header::ORIGIN, "http://127.0.0.1:2090"))
                .to_http_request();
            assert!(remote_origin_ok(&req, true, &allowed));
            crate::panel_public_url::clear_panel_public_url().unwrap();
        });
    }

    #[test]
    fn remote_accepts_localhost_when_public_url_is_127() {
        use crate::account::with_test_data_dir;
        with_test_data_dir(|| {
            crate::panel_public_url::save_panel_public_url("http://127.0.0.1:2090").unwrap();
            let allowed = build_allowed_hosts(2087, &[]);
            let req = TestRequest::default()
                .method(actix_web::http::Method::POST)
                .insert_header((actix_web::http::header::HOST, "localhost:2090"))
                .insert_header((actix_web::http::header::ORIGIN, "http://localhost:2090"))
                .to_http_request();
            assert!(remote_origin_ok(&req, true, &allowed));
            assert!(websocket_origin_ok(&req, true, &allowed));
            crate::panel_public_url::clear_panel_public_url().unwrap();
        });
    }

    #[test]
    fn remote_accepts_mixed_loopback_host_and_origin() {
        use crate::account::with_test_data_dir;
        with_test_data_dir(|| {
            crate::panel_public_url::save_panel_public_url("http://localhost:2090").unwrap();
            let allowed = build_allowed_hosts(2087, &[]);
            let req = TestRequest::default()
                .method(actix_web::http::Method::POST)
                .insert_header((actix_web::http::header::HOST, "127.0.0.1:2090"))
                .insert_header((actix_web::http::header::ORIGIN, "http://localhost:2090"))
                .to_http_request();
            assert!(remote_origin_ok(&req, true, &allowed));
            crate::panel_public_url::clear_panel_public_url().unwrap();
        });
    }

    #[test]
    fn loopback_aliases_share_nat_port_not_bind_port() {
        let allowed = build_allowed_hosts(2087, &[]);
        assert!(origin_matches_allowed("http://localhost:2087", &allowed));
        assert!(!origin_matches_allowed("http://localhost:2090", &allowed));
        assert!(!origin_matches_allowed("http://127.0.0.1:2090", &allowed));
    }

    #[test]
    fn origin_matching_request_host_passes_when_host_allowed() {
        let allowed = build_allowed_hosts(2087, &["panel.example".into()]);
        let req = TestRequest::default()
            .method(actix_web::http::Method::POST)
            .insert_header((actix_web::http::header::HOST, "panel.example:2087"))
            .insert_header((
                actix_web::http::header::ORIGIN,
                "http://panel.example:2087",
            ))
            .to_http_request();
        assert!(remote_origin_ok(&req, true, &allowed));
    }
}
