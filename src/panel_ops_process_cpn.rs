//! Snapshot CPN panel and related panel-owned processes (RSS / CPU).
//!
//! Site apps, MariaDB for websites, RabbitMQ, and LiteSpeed workers are out of
//! scope here. They appear on the full process list instead.

#[cfg(not(windows))]
use std::process::Command;

use crate::panel_ops_process::ProcessRow;
#[cfg(not(windows))]
use crate::panel_ops_process::{format_kb_field, map_state, parse_percent};

/// One CPN-related process row with a friendly role label.
#[derive(Debug, Clone)]
pub struct CpnProcessRow {
    pub role: String,
    pub row: ProcessRow,
}

/// Aggregated CPN panel resource usage.
#[derive(Debug, Clone, Default)]
pub struct CpnUsageSnapshot {
    pub panel_rss_kb: u64,
    pub panel_cpu: f64,
    pub panel_count: u32,
    pub webmail_rss_kb: u64,
    pub webmail_cpu: f64,
    pub webmail_count: u32,
    pub other_rss_kb: u64,
    pub other_cpu: f64,
    pub other_count: u32,
    pub rows: Vec<CpnProcessRow>,
    pub note: String,
}

impl CpnUsageSnapshot {
    pub fn total_rss_kb(&self) -> u64 {
        self.panel_rss_kb
            .saturating_add(self.webmail_rss_kb)
            .saturating_add(self.other_rss_kb)
    }

    pub fn total_cpu(&self) -> f64 {
        self.panel_cpu + self.webmail_cpu + self.other_cpu
    }

    pub fn total_count(&self) -> u32 {
        self.panel_count
            .saturating_add(self.webmail_count)
            .saturating_add(self.other_count)
    }
}

/// Classify a process command / user as a CPN panel-owned role.
pub fn classify_cpn_role(user: &str, command: &str) -> Option<&'static str> {
    let cmd = command.trim();
    let user_l = user.to_ascii_lowercase();
    if cmd.contains("/usr/bin/cpn-installer") {
        return Some("Panel service");
    }
    // Long-lived CLI is rare; still attribute if the binary is clearly CPN.
    if cmd == "/usr/bin/cpn"
        || cmd.starts_with("/usr/bin/cpn ")
        || cmd.starts_with("/usr/bin/cpn\t")
    {
        return Some("Panel CLI");
    }
    if cmd.contains("php-fpm: pool cpn-webmail") || cmd.contains("pool cpn-webmail") {
        return Some("Webmail PHP-FPM");
    }
    if user_l.starts_with("cpn-web") || user_l == "cpn-webmail" {
        return Some("Webmail PHP-FPM");
    }
    None
}

#[cfg(not(windows))]
fn role_bucket(role: &str) -> &'static str {
    match role {
        "Panel service" | "Panel CLI" => "panel",
        "Webmail PHP-FPM" => "webmail",
        _ => "other",
    }
}

/// Collect CPN panel + panel-owned webmail processes (Linux only).
pub fn snapshot_cpn_usage() -> Result<CpnUsageSnapshot, String> {
    #[cfg(windows)]
    {
        Ok(CpnUsageSnapshot {
            note: "CPN usage snapshot is supported on Linux hosts only.".into(),
            ..Default::default()
        })
    }
    #[cfg(not(windows))]
    {
        let out = Command::new("ps")
            .args([
                "-eo",
                "user:32,pid,state,pcpu,pmem,vsz,rss,etime,args",
                "--sort=-rss",
            ])
            .output()
            .map_err(|e| format!("Failed to run ps: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "ps failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut snap = CpnUsageSnapshot {
            note: "Site apps, website PHP, MariaDB for sites, RabbitMQ, and LiteSpeed workers are separate from CPN panel usage.".into(),
            ..Default::default()
        };
        for (i, line) in text.lines().enumerate() {
            if i == 0 {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 {
                continue;
            }
            let user = parts[0];
            let command = parts[8..].join(" ");
            let Some(role) = classify_cpn_role(user, &command) else {
                continue;
            };
            let rss_kb: u64 = parts[6].trim().parse().unwrap_or(0);
            let cpu = parse_percent(parts[3]);
            let row = ProcessRow {
                user: user.to_string(),
                pid: parts[1].to_string(),
                state: map_state(parts[2]),
                cpu: parts[3].to_string(),
                mem: parts[4].to_string(),
                virt: format_kb_field(parts[5]),
                res: format_kb_field(parts[6]),
                time: parts[7].to_string(),
                command,
            };
            match role_bucket(role) {
                "panel" => {
                    snap.panel_rss_kb = snap.panel_rss_kb.saturating_add(rss_kb);
                    snap.panel_cpu += cpu;
                    snap.panel_count = snap.panel_count.saturating_add(1);
                }
                "webmail" => {
                    snap.webmail_rss_kb = snap.webmail_rss_kb.saturating_add(rss_kb);
                    snap.webmail_cpu += cpu;
                    snap.webmail_count = snap.webmail_count.saturating_add(1);
                }
                _ => {
                    snap.other_rss_kb = snap.other_rss_kb.saturating_add(rss_kb);
                    snap.other_cpu += cpu;
                    snap.other_count = snap.other_count.saturating_add(1);
                }
            }
            snap.rows.push(CpnProcessRow {
                role: role.to_string(),
                row,
            });
        }
        Ok(snap)
    }
}

/// Human-readable RSS from kilobytes.
pub fn format_rss_kb(kb: u64) -> String {
    if kb >= 1024 * 1024 {
        format!("{:.1} GB", kb as f64 / (1024.0 * 1024.0))
    } else if kb >= 1024 {
        format!("{:.0} MB", kb as f64 / 1024.0)
    } else {
        format!("{kb} KB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_panel_and_webmail() {
        assert_eq!(
            classify_cpn_role("root", "/usr/bin/cpn-installer --web --port 2086"),
            Some("Panel service")
        );
        assert_eq!(
            classify_cpn_role("cpn-web+", "php-fpm: pool cpn-webmail"),
            Some("Webmail PHP-FPM")
        );
        assert_eq!(
            classify_cpn_role("root", "/usr/bin/cpn panel status"),
            Some("Panel CLI")
        );
        assert_eq!(
            classify_cpn_role("mysql", "/usr/sbin/mariadbd"),
            None
        );
        assert_eq!(
            classify_cpn_role("nobody", "openlitespeed"),
            None
        );
    }

    #[test]
    fn format_rss_scales() {
        assert_eq!(format_rss_kb(512), "512 KB");
        assert_eq!(format_rss_kb(57_176), "56 MB");
        assert!(format_rss_kb(2_097_152).contains("GB"));
    }

    #[test]
    fn snapshot_runs_or_explains() {
        match snapshot_cpn_usage() {
            Ok(snap) => {
                assert_eq!(
                    snap.total_count(),
                    snap.panel_count + snap.webmail_count + snap.other_count
                );
                assert!(!snap.note.is_empty());
            }
            Err(msg) => assert!(!msg.is_empty()),
        }
    }
}
