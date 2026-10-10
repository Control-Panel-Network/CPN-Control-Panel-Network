//! Tip-upgrade lines for Server > Logs (Main Log and Error logs).
//!
//! Format: timestamp, module, level, message, retry count. Secrets are redacted.

use crate::install_log_redaction::redact_sensitive_line;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MODULE: &str = "upgrade_tip";
const PANEL_LOG: &str = "/var/log/cpn/panel.log";
const INSTALLER_LOG: &str = "/var/log/cpn-installer.log";
const CPN_ERROR_LOG: &str = "/var/log/cpn/error.log";
const OLS_ERROR_LOG: &str = "/usr/local/lsws/logs/error.log";

static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Server clock as `dd/mm/yyyy HH:MM:SS` (UTC, 24 hour). Shared with the
/// Version page session transcript so both logs carry the same stamps.
pub fn now_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let rem = secs % 86400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;
    // Civil date from Unix days (Howard Hinnant). Display as dd/mm/yyyy 24h UTC.
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{d:02}/{m:02}/{year} {hour:02}:{min:02}:{sec:02}")
}

pub fn format_line(level: &str, message: &str, retry: Option<u32>) -> String {
    format_line_for(MODULE, level, message, retry)
}

pub fn format_line_for(module: &str, level: &str, message: &str, retry: Option<u32>) -> String {
    let safe = redact_sensitive_line(message);
    let retry_part = match retry {
        Some(n) => format!(" retry={n}"),
        None => String::new(),
    };
    let mod_name = if module.is_empty() { MODULE } else { module };
    format!(
        "[{}] [{mod_name}] [{level}] {safe}{retry_part}",
        now_stamp()
    )
}

fn append_path(path: &str, line: &str) -> bool {
    let path = Path::new(path);
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return false;
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return false;
    };
    writeln!(file, "{line}").is_ok() && file.flush().is_ok()
}

fn write_main(line: &str) {
    let _ = append_path(PANEL_LOG, line);
    if Path::new(INSTALLER_LOG).is_file() {
        let _ = append_path(INSTALLER_LOG, line);
    }
}

fn write_error(line: &str) {
    let _ = append_path(CPN_ERROR_LOG, line);
    if Path::new(OLS_ERROR_LOG).is_file() {
        let _ = append_path(OLS_ERROR_LOG, line);
    }
}

/// Info and errors go to Main Log (`/var/log/cpn/panel.log`). Errors also go to Error logs.
pub fn log_event(level: &str, message: &str, retry: Option<u32>) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let line = format_line(level, message, retry);
    write_main(&line);
    crate::upgrade_session_log::append_session(level, message);
    if level.eq_ignore_ascii_case("error") || level.eq_ignore_ascii_case("err") {
        write_error(&line);
    }
}

pub fn log_info(message: impl AsRef<str>) {
    log_event("info", message.as_ref(), None);
}

pub fn log_failure(message: impl AsRef<str>, retry: Option<u32>) {
    log_event("error", message.as_ref(), retry);
}

/// Main Log only (no Version page session line). Used by housekeeping that runs
/// outside a maintenance job, such as the panel-start staging sweep, so the
/// finished session transcript keeps its `DONE` line last.
pub fn log_tagged_main_only(module: &str, level: &str, message: &str) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let line = format_line_for(module, level, message, None);
    write_main(&line);
    if level.eq_ignore_ascii_case("error") || level.eq_ignore_ascii_case("err") {
        write_error(&line);
    }
}

/// Main Log line with `[upgrade]`, `[repair]`, or `[upgrade_tip]`.
pub fn log_tagged(module: &str, level: &str, message: &str) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let line = format_line_for(module, level, message, None);
    write_main(&line);
    crate::upgrade_session_log::append_session(level, message);
    if level.eq_ignore_ascii_case("error") || level.eq_ignore_ascii_case("err") {
        write_error(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_includes_module_and_retry() {
        let line = format_line("error", "cargo missing", Some(1));
        assert!(line.contains("[upgrade_tip]"));
        assert!(line.contains("[error]"));
        assert!(line.contains("cargo missing"));
        assert!(line.contains("retry=1"));
        assert!(line.contains('/'));
    }

    #[test]
    fn format_redacts_webadmin_secrets() {
        let generated = ["temporary", "credential"].join("-");
        let line = format_line(
            "error",
            &format!("WebAdmin user/password is admin/{generated}"),
            None,
        );
        assert!(line.contains("[REDACTED]"));
        assert!(!line.contains(&generated));
    }

    #[test]
    fn tagged_lines_use_upgrade_or_repair_module() {
        let upgrade = format_line_for("upgrade", "info", "Scheduled detached reload", None);
        assert!(upgrade.contains("[upgrade]"));
        assert!(upgrade.contains("[info]"));
        let repair = format_line_for("repair", "info", "Scheduled detached reload", None);
        assert!(repair.contains("[repair]"));
        assert!(!repair.contains('\u{2014}'));
    }
}
