//! Per-vhost bandwidth hints from access logs and package quotas.

use crate::packages::{UNLIMITED, format_limit_display, package_for_account};
use crate::panel_website_logs::{candidate_log_paths, first_existing_log, read_log_tail};
use crate::panel_website_resources::format_bytes;
use crate::sites::SiteRecord;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct BandwidthInfo {
    pub label: String,
    pub hint: String,
    pub bytes: Option<u64>,
    pub period: Option<&'static str>,
    pub source: &'static str,
    pub quota_mb: Option<i64>,
}

fn local_day_token() -> Option<String> {
    #[cfg(windows)]
    {
        None
    }
    #[cfg(not(windows))]
    {
        let out = Command::new("date")
            .args(["+%d/%b/%Y"])
            .env("LC_ALL", "C")
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

fn local_month_token() -> Option<String> {
    #[cfg(windows)]
    {
        None
    }
    #[cfg(not(windows))]
    {
        let out = Command::new("date")
            .args(["+%b/%Y"])
            .env("LC_ALL", "C")
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!s.is_empty()).then_some(s)
    }
}

/// Sum response sizes from Common/Combined-style access log lines.
/// Looks for `"...HTTP/x.y" STATUS BYTES` after the request.
pub fn sum_bytes_from_access_log(
    text: &str,
    day: Option<&str>,
    month: Option<&str>,
) -> (u64, u64, usize) {
    let mut today = 0u64;
    let mut month_total = 0u64;
    let mut lines = 0usize;
    for line in text.lines() {
        let Some(bytes) = parse_response_bytes(line) else {
            continue;
        };
        lines += 1;
        let in_month = month.map(|m| line.contains(m)).unwrap_or(false);
        let in_day = day.map(|d| line.contains(d)).unwrap_or(false);
        if in_day {
            today = today.saturating_add(bytes);
        }
        if in_month {
            month_total = month_total.saturating_add(bytes);
        }
    }
    (today, month_total, lines)
}

fn parse_response_bytes(line: &str) -> Option<u64> {
    // Prefer bytes after the request line that ends with HTTP/x.y".
    let after = if let Some(http_at) = line.find("HTTP/") {
        let tail = &line[http_at..];
        let q = tail.find('"')?;
        &tail[q + 1..]
    } else {
        line.rsplit_once("\" ").map(|(_, rest)| rest)?
    };
    let mut parts = after.split_whitespace();
    let status = parts.next()?;
    if !status.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let size = parts.next()?;
    if size == "-" {
        return Some(0);
    }
    size.parse().ok()
}

fn package_quota_mb(owner: &str) -> Option<i64> {
    package_for_account(owner).ok().map(|p| p.bandwidth_mb)
}

/// Resolve bandwidth card text: access-log bytes when possible, else package quota, else Not metered.
pub fn bandwidth_for_site(site: &SiteRecord) -> BandwidthInfo {
    let quota_mb = package_quota_mb(&site.owner);
    let (access, _) = candidate_log_paths(site);
    let day = local_day_token();
    let month = local_month_token();

    if let Some(path) = first_existing_log(site, &access)
        && let Ok(text) = read_log_tail(site, &path, 2_000_000)
    {
        let (today, month_total, lines) =
            sum_bytes_from_access_log(&text, day.as_deref(), month.as_deref());
        if lines > 0 && (today > 0 || month_total > 0 || day.is_some()) {
            let (bytes, period) = if today > 0 || day.is_some() {
                (today, "today")
            } else {
                (month_total, "this month")
            };
            let mut label = format!("{} ({period})", format_bytes(bytes));
            let mut hint = format!(
                "From access log sample ({path}). Host transfer estimate, not package enforcement.",
                path = path.display()
            );
            if let Some(q) = quota_mb {
                hint.push_str(&format!(
                    " Package quota: {}.",
                    format_limit_display(q, "MB")
                ));
                if q != UNLIMITED && q > 0 {
                    label = format!(
                        "{} / {}",
                        format_bytes(bytes),
                        format_limit_display(q, "MB")
                    );
                }
            }
            return BandwidthInfo {
                label,
                hint,
                bytes: Some(bytes),
                period: Some(period),
                source: "access_log",
                quota_mb,
            };
        }
    }

    if let Some(q) = quota_mb {
        if q == UNLIMITED {
            return BandwidthInfo {
                label: "Unlimited quota".into(),
                hint: "Package bandwidth is unlimited; per-vhost transfer counters are not enforced yet."
                    .into(),
                bytes: None,
                period: None,
                source: "package_quota",
                quota_mb: Some(q),
            };
        }
        return BandwidthInfo {
            label: format!("Quota {}", format_limit_display(q, "MB")),
            hint: "Package bandwidth limit is configured; usage from access logs was not available for this site."
                .into(),
            bytes: None,
            period: None,
            source: "package_quota",
            quota_mb: Some(q),
        };
    }

    BandwidthInfo {
        label: "Not metered".into(),
        hint: "No access log bytes and no package bandwidth quota were available for this site."
            .into(),
        bytes: None,
        period: None,
        source: "none",
        quota_mb: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combined_log_bytes() {
        let line = r#"127.0.0.1 - - [13/Sep/2026:21:00:00 +0000] "GET / HTTP/1.1" 200 4096 "https://x/" "ua""#;
        assert_eq!(parse_response_bytes(line), Some(4096));
    }

    #[test]
    fn sums_today_lines() {
        let text = r#"
1.1.1.1 - - [13/Sep/2026:10:00:00 +0000] "GET /a HTTP/1.1" 200 100
1.1.1.1 - - [12/Sep/2026:10:00:00 +0000] "GET /b HTTP/1.1" 200 500
1.1.1.1 - - [13/Sep/2026:11:00:00 +0000] "GET /c HTTP/1.1" 200 50
"#;
        let (today, month, lines) =
            sum_bytes_from_access_log(text, Some("13/Sep/2026"), Some("Sep/2026"));
        assert_eq!(lines, 3);
        assert_eq!(today, 150);
        assert_eq!(month, 650);
    }

    #[test]
    fn dash_size_is_zero() {
        let line = r#"1.1.1.1 - - [13/Sep/2026:10:00:00 +0000] "GET / HTTP/1.1" 304 -"#;
        assert_eq!(parse_response_bytes(line), Some(0));
    }
}
