//! Live installer/upgrade transcript for Version Management (CLI-style tail).
//!
//! Stored under the CPN data dir. Secrets are redacted. Truncated at the start
//! of each maintenance run so the UI shows the current job.
//!
//! Line format: `[dd/mm/yyyy HH:MM:SS] <marker> <message>` (server UTC clock,
//! 24 hour; the Version page shows the same instant in browser-local time).
//! Markers: `==>` progress, `FAILED` error, `note` demoted transient warning,
//! `DONE` final success line, `FAILED (job)` final failure line.

use crate::install_log_redaction::redact_sensitive_line;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

const SESSION_FILE: &str = "upgrade-session.log";
const MAX_TAIL_BYTES: u64 = 96 * 1024;
const MAX_TAIL_LINES: usize = 500;

/// Persistent Main Log every session line is also written to (via `upgrade_tip_log`
/// and the installer transcript). Shown on the Version page as the place to look
/// after the session tail rotates.
pub const PERSISTENT_LOG_PATH: &str = "/var/log/cpn/panel.log";
/// Panel route that renders [`PERSISTENT_LOG_PATH`] with search and pagination.
pub const PERSISTENT_LOG_ROUTE: &str = "/server/logs/panel";

/// Marker written at the start of the final success line.
pub const DONE_MARKER: &str = "DONE";
/// Marker written at the start of the final failure line.
pub const FAILED_JOB_MARKER: &str = "FAILED (job)";

static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn session_log_path() -> PathBuf {
    crate::paths::join_data(SESSION_FILE)
}

/// Login-gate probes log through `upgrade_tip_log` so they reach Main Log and
/// Error logs, but they are not part of an upgrade job. While the panel comes
/// back after the upgrade restart they fire a few times and used to land in the
/// session tail as `FAILED ...`, which read as a failed upgrade.
pub fn is_transient_gate_noise(message: &str) -> bool {
    message.contains("login_service_gate")
}

fn stamp() -> String {
    crate::upgrade_tip_log::now_stamp()
}

fn open_append() -> Option<fs::File> {
    let path = session_log_path();
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return None;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Some(file)
}

pub fn begin_session(kind: &str) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let path = session_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let header = format!(
        "[{}] CPN installer session ({})\n[{}] Persistent copy: {PERSISTENT_LOG_PATH} (Server > Logs > Panel)\n",
        stamp(),
        redact_sensitive_line(kind),
        stamp()
    );
    let _ = fs::write(&path, header);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
}

/// Build the stored line for one message (without the trailing newline).
pub fn format_session_line(level: &str, message: &str, job_in_flight: bool) -> Option<String> {
    let safe = redact_sensitive_line(message);
    let is_error = level.eq_ignore_ascii_case("error") || level.eq_ignore_ascii_case("err");
    if is_transient_gate_noise(&safe) {
        if !job_in_flight {
            // Panel already restarted (or no job at all): keep the finished
            // session's last line meaningful; Main Log still has the probe.
            return None;
        }
        return Some(format!(
            "[{}] note {safe} (transient while the panel restarts; not an upgrade failure)",
            stamp()
        ));
    }
    let body = if is_error {
        format!("FAILED {safe}")
    } else if level.eq_ignore_ascii_case("progress") {
        format!("==> {safe}")
    } else {
        safe
    };
    Some(format!("[{}] {body}", stamp()))
}

pub fn append_session(level: &str, message: &str) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let Some(line) = format_session_line(level, message, crate::upgrade_busy::job_in_flight())
    else {
        return;
    };
    let Some(mut file) = open_append() else {
        return;
    };
    let _ = writeln!(file, "{line}");
    let _ = file.flush();
}

fn append_raw(body: &str) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let Some(mut file) = open_append() else {
        return;
    };
    let _ = writeln!(file, "[{}] {}", stamp(), redact_sensitive_line(body));
    let _ = file.flush();
}

/// Final line of a successful job. `restart_scheduled` tells the operator why
/// the panel will blink once more after this line.
pub fn finish_session_ok(message: &str, restart_scheduled: bool) {
    let tail = if restart_scheduled {
        " The panel restarts once to load the new binary; this page reloads when it is back."
    } else {
        ""
    };
    append_raw(&format!(
        "{DONE_MARKER} {message}.{tail} Full log: {PERSISTENT_LOG_PATH}"
    ));
}

/// Final line of a failed job.
pub fn finish_session_failed(error: &str) {
    append_raw(&format!(
        "{FAILED_JOB_MARKER} {error} Full log: {PERSISTENT_LOG_PATH}"
    ));
}

/// Last lines of the session log for the Version page (no tokens).
pub fn tail_session() -> String {
    let path = session_log_path();
    let Ok(meta) = fs::metadata(&path) else {
        return String::new();
    };
    let Ok(mut file) = fs::File::open(&path) else {
        return String::new();
    };
    let len = meta.len();
    if len > MAX_TAIL_BYTES {
        let skip = len - MAX_TAIL_BYTES;
        if std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(skip)).is_err() {
            return String::new();
        }
    }
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return String::new();
    }
    let redacted = redact_sensitive_line(&buf);
    let lines: Vec<&str> = redacted.lines().collect();
    if lines.len() <= MAX_TAIL_LINES {
        return redacted.trim_end().to_string();
    }
    lines[lines.len() - MAX_TAIL_LINES..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::DATA_DIR_TEST_LOCK;

    #[test]
    fn failed_prefix_marks_errors() {
        let _guard = DATA_DIR_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "cpn-sess-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::create_dir_all(&dir);
        // SAFETY: tests hold DATA_DIR_TEST_LOCK while mutating CPN_DATA_DIR.
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &dir);
        }
        begin_session("upgrade");
        append_session("info", "cargo build --release");
        append_session("error", "marker missing");
        append_session(
            "error",
            "login_service_gate: /api/login/services timed out; sign-in left open",
        );
        finish_session_ok("Updated to stable @ abc1234 (1.4.0)", true);
        let tail = tail_session();
        assert!(tail.contains("cargo build --release"));
        assert!(tail.contains("FAILED marker missing"));
        assert!(!tail.contains("Bearer "));
        // No job in flight in the test process: gate noise is dropped entirely.
        assert!(!tail.contains("login_service_gate"));
        assert!(tail.contains("DONE Updated to stable @ abc1234 (1.4.0)."));
        assert!(tail.contains(PERSISTENT_LOG_PATH));
        // Every line carries a dd/mm/yyyy HH:MM:SS prefix.
        for line in tail.lines() {
            assert!(
                line.starts_with('[')
                    && line.len() > 22
                    && &line[3..4] == "/"
                    && &line[6..7] == "/",
                "line must start with [dd/mm/yyyy HH:MM:SS]: {line}"
            );
        }
        unsafe {
            std::env::remove_var("CPN_DATA_DIR");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn gate_noise_is_demoted_while_a_job_runs() {
        let line = format_session_line(
            "error",
            "login_service_gate: POST /login gate timed out; sign-in left open",
            true,
        )
        .expect("demoted line");
        assert!(line.contains("] note login_service_gate"));
        assert!(!line.contains("FAILED"));
        assert!(line.contains("not an upgrade failure"));
        assert!(
            format_session_line(
                "error",
                "login_service_gate: evaluate timed out; sign-in left open (fail-open)",
                false
            )
            .is_none()
        );
        let real = format_session_line("error", "cargo build failed", false).expect("error line");
        assert!(real.contains("] FAILED cargo build failed"));
        let progress = format_session_line("progress", "phase=installing", true).expect("progress");
        assert!(progress.contains("] ==> phase=installing"));
        for text in [&line, &real, &progress] {
            assert!(!text.contains('\u{2014}'));
            assert!(!text.contains('\u{2013}'));
        }
    }
}
