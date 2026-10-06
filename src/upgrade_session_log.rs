//! Live installer/upgrade transcript for Version Management (CLI-style tail).
//!
//! Stored under the CPN data dir. Secrets are redacted. Truncated at the start
//! of each maintenance run so the UI shows the current job.

use crate::install_log_redaction::redact_sensitive_line;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

const SESSION_FILE: &str = "upgrade-session.log";
const MAX_TAIL_BYTES: u64 = 96 * 1024;
const MAX_TAIL_LINES: usize = 500;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

pub fn session_log_path() -> PathBuf {
    crate::paths::join_data(SESSION_FILE)
}

pub fn begin_session(kind: &str) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let path = session_log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let header = format!("CPN installer session ({})\n", redact_sensitive_line(kind));
    let _ = fs::write(&path, header);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
}

pub fn append_session(level: &str, message: &str) {
    let Ok(_guard) = WRITE_LOCK.lock() else {
        return;
    };
    let path = session_log_path();
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let safe = redact_sensitive_line(message);
    let line = if level.eq_ignore_ascii_case("error") || level.eq_ignore_ascii_case("err") {
        format!("FAILED {safe}")
    } else if level.eq_ignore_ascii_case("progress") {
        format!("==> {safe}")
    } else {
        safe
    };
    let _ = writeln!(file, "{line}");
    let _ = file.flush();
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

    #[test]
    fn failed_prefix_marks_errors() {
        let dir = std::env::temp_dir().join(format!("cpn-sess-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var("CPN_DATA_DIR", &dir);
        }
        begin_session("upgrade");
        append_session("info", "cargo build --release");
        append_session("error", "marker missing");
        let tail = tail_session();
        assert!(tail.contains("cargo build --release"));
        assert!(tail.contains("FAILED marker missing"));
        assert!(!tail.contains("Bearer "));
        let _ = fs::remove_dir_all(&dir);
    }
}
