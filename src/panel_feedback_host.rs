//! Resolve a public-facing Panel host for feedback mail metadata.

use std::cmp::Reverse;
use std::net::IpAddr;

/// Content for the Feedback "Panel host" field (plain + HTML).
pub fn resolve_feedback_panel_host(request_host: Option<&str>, listen_port: u16) -> String {
    let detected = crate::panel_host_info::host_sidebar_info().ip;
    resolve_feedback_panel_host_with(
        request_host,
        listen_port,
        crate::panel_public_url::load_panel_public_url(),
        crate::panel_network::load_panel_hostname(),
        &detected,
    )
}

/// Testable host picker. Preference order among equal quality: public URL, detected IP,
/// request Host, then configured hostname.
pub(crate) fn resolve_feedback_panel_host_with(
    request_host: Option<&str>,
    listen_port: u16,
    panel_public_url: Option<String>,
    panel_hostname: Option<String>,
    detected_ip: &str,
) -> String {
    let mut candidates: Vec<String> = Vec::new();

    if let Some(url) = panel_public_url.as_deref()
        && let Some(host_port) = host_port_from_base_url(url)
    {
        push_unique(&mut candidates, host_port);
    }

    if let Some(ip_host) = format_detected_ip_host(detected_ip, listen_port) {
        push_unique(&mut candidates, ip_host);
    }

    if let Some(host) = sanitize_request_host(request_host) {
        push_unique(&mut candidates, host);
    }

    if let Some(hostname) = panel_hostname
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 253)
    {
        push_unique(&mut candidates, hostname.to_string());
    }

    pick_best_host(&candidates).unwrap_or_else(|| format!("unknown:{listen_port}"))
}

fn push_unique(candidates: &mut Vec<String>, value: String) {
    if !candidates.iter().any(|existing| existing.eq_ignore_ascii_case(&value)) {
        candidates.push(value);
    }
}

fn pick_best_host(candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .enumerate()
        .max_by_key(|(idx, value)| (host_quality(value), Reverse(*idx)))
        .map(|(_, value)| value.clone())
}

/// Higher is better. Public IPs and DNS names beat loopback; loopback beats RFC1918/VBox NAT.
fn host_quality(host_port: &str) -> i32 {
    let host = host_only(host_port);
    if host.is_empty() || host.eq_ignore_ascii_case("unknown") {
        return -1;
    }
    if is_loopback_host(&host) {
        return 1;
    }
    if is_private_or_link_local(&host) {
        return 0;
    }
    3
}

fn host_only(host_port: &str) -> String {
    let trimmed = host_port.trim();
    if trimmed.starts_with('[') {
        if let Some(end) = trimmed.find(']') {
            return trimmed[1..end].to_string();
        }
        return trimmed.trim_matches(|c| c == '[' || c == ']').to_string();
    }
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return ip.to_string();
    }
    match trimmed.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            host.to_string()
        }
        _ => trimmed.to_string(),
    }
}

fn is_loopback_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") || lower == "::1" {
        return true;
    }
    match lower.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_loopback(),
        Ok(IpAddr::V6(v6)) => v6.is_loopback(),
        Err(_) => false,
    }
}

fn is_private_or_link_local(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    if lower == "10.0.2.15" {
        return true;
    }
    match lower.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            v4.is_private() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast()
        }
        Ok(IpAddr::V6(v6)) => {
            v6.is_unique_local() || v6.is_unicast_link_local() || v6.is_unspecified()
        }
        Err(_) => false,
    }
}

fn host_port_from_base_url(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1.trim();
    let host_port = rest.split(['/', '?', '#']).next()?.trim();
    if host_port.is_empty() {
        return None;
    }
    Some(host_port.to_string())
}

fn sanitize_request_host(request_host: Option<&str>) -> Option<String> {
    let value = request_host?.trim();
    if value.is_empty() || value.len() > 253 {
        return None;
    }
    if value
        .chars()
        .any(|ch| ch.is_control() || ch == '/' || ch == '\\' || ch.is_whitespace())
    {
        return None;
    }
    Some(value.to_string())
}

fn format_detected_ip_host(detected_ip: &str, listen_port: u16) -> Option<String> {
    let ip = detected_ip.trim();
    if ip.is_empty() || ip.eq_ignore_ascii_case("Unavailable") {
        return None;
    }
    let parsed: IpAddr = ip.parse().ok()?;
    if parsed.is_unspecified() {
        return None;
    }
    Some(match parsed {
        IpAddr::V4(v4) => format!("{v4}:{listen_port}"),
        IpAddr::V6(v6) => format!("[{v6}]:{listen_port}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_public_ip_over_loopback_public_url() {
        let host = resolve_feedback_panel_host_with(
            Some("127.0.0.1:2087"),
            2087,
            Some("http://127.0.0.1:2087".into()),
            None,
            "207.180.193.210",
        );
        assert_eq!(host, "207.180.193.210:2087");
    }

    #[test]
    fn prefers_configured_public_url_hostname() {
        let host = resolve_feedback_panel_host_with(
            Some("10.0.2.15:2087"),
            2087,
            Some("https://panel.example.com".into()),
            Some("ignored.example.com".into()),
            "10.0.2.15",
        );
        assert_eq!(host, "panel.example.com");
    }

    #[test]
    fn nat_lab_keeps_loopback_public_url_over_vbox_guest_ip() {
        let host = resolve_feedback_panel_host_with(
            Some("10.0.2.15:2087"),
            2087,
            Some("http://127.0.0.1:2090".into()),
            None,
            "10.0.2.15",
        );
        assert_eq!(host, "127.0.0.1:2090");
    }

    #[test]
    fn uses_request_host_when_public() {
        let host = resolve_feedback_panel_host_with(
            Some("cpn.newstargeted.com"),
            2087,
            None,
            None,
            "Unavailable",
        );
        assert_eq!(host, "cpn.newstargeted.com");
    }

    #[test]
    fn falls_back_to_panel_hostname() {
        let host = resolve_feedback_panel_host_with(
            Some("localhost:2087"),
            2087,
            None,
            Some("mail.example.com".into()),
            "Unavailable",
        );
        assert_eq!(host, "mail.example.com");
    }

    #[test]
    fn quality_ranks_public_above_private() {
        assert!(host_quality("207.180.193.210:2087") > host_quality("127.0.0.1:2087"));
        assert!(host_quality("127.0.0.1:2087") > host_quality("10.0.2.15:2087"));
        assert!(host_quality("panel.example.com") > host_quality("192.168.1.10:2087"));
    }
}
