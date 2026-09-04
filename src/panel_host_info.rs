//! Best-effort host IP and uptime for the authenticated panel sidebar.

use std::net::UdpSocket;
use std::process::{Command, Stdio};

/// Snapshot shown in the sidebar host card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSidebarInfo {
    pub ip: String,
    pub uptime: String,
}

/// Resolve primary IPv4 and human uptime for sidebar chrome.
pub fn host_sidebar_info() -> HostSidebarInfo {
    HostSidebarInfo {
        ip: primary_ipv4().unwrap_or_else(|| "Unavailable".into()),
        uptime: format_uptime(uptime_seconds()),
    }
}

fn primary_ipv4() -> Option<String> {
    if let Some(ip) = udp_primary_ip() {
        return Some(ip);
    }
    hostname_first_ipv4()
}

fn udp_primary_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    let text = ip.to_string();
    if text.starts_with("127.") || text.contains(':') {
        return None;
    }
    Some(text)
}

fn hostname_first_ipv4() -> Option<String> {
    #[cfg(windows)]
    {
        let raw = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -notlike '127.*' } | Select-Object -First 1).IPAddress",
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !raw.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&raw.stdout).trim().to_owned();
        (!value.is_empty()).then_some(value)
    }
    #[cfg(not(windows))]
    {
        let raw = Command::new("hostname")
            .arg("-I")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !raw.status.success() {
            return None;
        }
        String::from_utf8_lossy(&raw.stdout)
            .split_whitespace()
            .find(|value| !value.starts_with("127.") && !value.contains(':'))
            .map(str::to_owned)
    }
}

fn uptime_seconds() -> Option<u64> {
    #[cfg(windows)]
    {
        let raw = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[int64]((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).TotalSeconds",
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !raw.status.success() {
            return None;
        }
        String::from_utf8_lossy(&raw.stdout).trim().parse().ok()
    }
    #[cfg(not(windows))]
    {
        let raw = std::fs::read_to_string("/proc/uptime").ok()?;
        let secs_f: f64 = raw.split_whitespace().next()?.parse().ok()?;
        Some(secs_f.floor() as u64)
    }
}

/// Format seconds as `60D, 5H, 18M` style (CyberPanel-inspired, CPN-owned copy).
pub fn format_uptime(seconds: Option<u64>) -> String {
    let Some(total) = seconds else {
        return "Unknown".into();
    };
    let days = total / 86_400;
    let hours = (total % 86_400) / 3_600;
    let mins = (total % 3_600) / 60;
    if days > 0 {
        format!("{days}D, {hours}H, {mins}M")
    } else if hours > 0 {
        format!("{hours}H, {mins}M")
    } else {
        format!("{mins}M")
    }
}

#[cfg(test)]
mod tests {
    use super::format_uptime;

    #[test]
    fn formats_multi_day_uptime() {
        // 60d + 5h + 18m
        let secs = 60 * 86_400 + 5 * 3_600 + 18 * 60;
        assert_eq!(format_uptime(Some(secs)), "60D, 5H, 18M");
    }

    #[test]
    fn formats_short_uptime() {
        assert_eq!(format_uptime(Some(125)), "2M");
        assert_eq!(format_uptime(Some(3_720)), "1H, 2M");
        assert_eq!(format_uptime(None), "Unknown");
    }
}
