//! Monthly bandwidth counter reset helper and systemd timer unit templates.

use crate::package_bandwidth::{MeterPeriod, current_period, period_from_year_month};
use crate::paths::join_data;
use crate::sites::list_sites;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BandwidthResetLog {
    pub last_reset_period: String,
    pub last_reset_unix: u64,
    pub sites_touched: u32,
    pub note: String,
}

fn log_path() -> PathBuf {
    join_data("bandwidth-reset.json")
}

fn ledger_dir() -> PathBuf {
    join_data("bandwidth")
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load_reset_log() -> BandwidthResetLog {
    let Ok(raw) = fs::read_to_string(log_path()) else {
        return BandwidthResetLog::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_reset_log(log: &BandwidthResetLog) -> Result<(), String> {
    if let Some(parent) = log_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(log).map_err(|e| e.to_string())?;
    fs::write(log_path(), raw).map_err(|e| format!("Cannot save bandwidth reset log: {e}"))
}

/// Reset period counters for a previous calendar month (or force current if requested).
/// Ledgers already key by YYYY-MM; this clears stale previous-period files and records the reset.
pub fn reset_monthly_counters(force: bool) -> Result<String, String> {
    let period = current_period();
    let log = load_reset_log();
    if !force && log.last_reset_period == period.key {
        return Ok(format!(
            "Bandwidth period {} already recorded as reset.",
            period.key
        ));
    }
    let dir = ledger_dir();
    let mut touched = 0u32;
    if dir.is_dir() {
        for entry in fs::read_dir(&dir).map_err(|e| format!("Cannot read bandwidth dir: {e}"))? {
            let entry = entry.map_err(|e| format!("Cannot read entry: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            let ledger_period = value
                .get("period")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            // Clear ledgers that are not the current calendar month.
            if !ledger_period.is_empty() && ledger_period != period.key {
                if let Some(obj) = value.as_object_mut() {
                    obj.insert("bytes".into(), serde_json::json!(0));
                    obj.insert("period".into(), serde_json::json!(period.key));
                    obj.insert("offset".into(), serde_json::json!(0));
                }
                let _ = fs::write(&path, serde_json::to_string_pretty(&value).unwrap_or(raw));
                touched += 1;
            }
        }
    }
    // Touch known sites so UI shows the new Norwegian period label immediately.
    let _sites = list_sites().unwrap_or_default();
    let note = format!(
        "Monthly bandwidth reset for period {} (dd/mm style UI). Sites ledger files updated: {touched}.",
        period_label_nb(&period)
    );
    save_reset_log(&BandwidthResetLog {
        last_reset_period: period.key.clone(),
        last_reset_unix: now_unix(),
        sites_touched: touched,
        note: note.clone(),
    })?;
    Ok(note)
}

fn period_label_nb(period: &MeterPeriod) -> String {
    // period.key is YYYY-MM → show mm/yyyy for Norwegian UI hints.
    let parts: Vec<&str> = period.key.split('-').collect();
    if parts.len() == 2 {
        format!("{}/{}", parts[1], parts[0])
    } else {
        period.key.clone()
    }
}

/// systemd timer unit text (operators install under /etc/systemd/system/).
pub fn systemd_timer_unit() -> &'static str {
    "[Unit]\nDescription=CPN monthly bandwidth counter reset\n\n[Timer]\nOnCalendar=*-*-01 00:00:00\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n"
}

pub fn systemd_service_unit() -> &'static str {
    "[Unit]\nDescription=CPN monthly bandwidth counter reset\n\n[Service]\nType=oneshot\nExecStart=/usr/local/bin/cpn bandwidth-reset\nNice=10\n"
}

/// Cron helper line: `0 0 1 * *`
pub fn cron_line() -> &'static str {
    "0 0 1 * * root /usr/local/bin/cpn bandwidth-reset >/var/log/cpn-bandwidth-reset.log 2>&1"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn reset_log_roundtrip() {
        with_test_data_dir(|| {
            let msg = reset_monthly_counters(true).unwrap();
            assert!(msg.to_ascii_lowercase().contains("bandwidth"));
            let log = load_reset_log();
            assert!(!log.last_reset_period.is_empty());
            let p = period_from_year_month(2026, 10);
            assert_eq!(period_label_nb(&p), "10/2026");
        });
    }
}
