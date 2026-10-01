//! Append-only panel action log (plugin and host package changes).
//!
//! Feeds the Activity Board "Panel Actions" tab and the Server > Logs page. One JSON object per
//! line in `<data dir>/panel-actions.jsonl` (mode 600). The file is capped: when it grows past
//! [`MAX_BYTES`] only the newest [`KEEP_LINES`] entries are kept. Messages are passed through the
//! same credential redaction as SSH log lines, so secrets never reach the log.

use crate::account::{data_dir, now_unix};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub const LOG_FILE: &str = "panel-actions.jsonl";
const MAX_BYTES: u64 = 1_048_576;
const KEEP_LINES: usize = 2000;
const MAX_FIELD_CHARS: usize = 300;

static WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionRecord {
    /// Unix seconds (UTC).
    pub ts: u64,
    pub actor: String,
    /// Machine key such as `plugin.install-host` (see [`action_label`]).
    pub action: String,
    /// Domain or subdomain the action was scoped to; empty for host-wide actions.
    #[serde(default)]
    pub target: String,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
}

pub fn log_path() -> PathBuf {
    data_dir().join(LOG_FILE)
}

/// Map a POST path to an action key. `None` for paths that are not logged.
pub fn action_for_path(path: &str) -> Option<&'static str> {
    let path = path.split('?').next().unwrap_or(path).trim_end_matches('/');
    match path {
        "/apps/install" => Some("host.install"),
        "/apps/reinstall" => Some("host.reinstall"),
        "/apps/uninstall" => Some("host.uninstall"),
        "/apps/start" => Some("host.start"),
        "/apps/stop" => Some("host.stop"),
        "/apps/activate" => Some("host.attach"),
        "/apps/deactivate" => Some("host.detach"),
        "/plugins/install" => Some("plugin.install"),
        "/plugins/install-host" => Some("plugin.install-host"),
        "/plugins/activate-host" => Some("plugin.activate-host"),
        "/plugins/deactivate-host" => Some("plugin.deactivate-host"),
        "/plugins/uninstall-host" => Some("plugin.uninstall-host"),
        "/plugins/uninstall" => Some("plugin.uninstall"),
        "/plugins/enable" => Some("plugin.activate"),
        "/plugins/disable" => Some("plugin.deactivate"),
        // Theme JSON APIs record via handlers (not PRG Location); keys listed for labels.
        "/api/panel/themes/install" => Some("theme.install"),
        "/api/panel/themes/uninstall" => Some("theme.uninstall"),
        "/api/panel/themes/apply" => Some("theme.apply"),
        "/api/panel/themes/update-all" => Some("theme.update-all"),
        _ => None,
    }
}

/// Human label for an action key.
pub fn action_label(action: &str) -> &'static str {
    match action {
        "host.install" => "Host package: install",
        "host.reinstall" => "Host package: reinstall",
        "host.uninstall" => "Host package: uninstall",
        "host.start" => "Host package: start",
        "host.stop" => "Host package: stop",
        "host.attach" => "Host package: attach to site",
        "host.detach" => "Host package: detach from site",
        "plugin.install" => "Plugin: install",
        "plugin.install-host" => "Plugin: install on host",
        "plugin.activate-host" => "Plugin: activate from host",
        "plugin.deactivate-host" => "Plugin: deactivate (host install)",
        "plugin.uninstall-host" => "Plugin: uninstall from host",
        "plugin.uninstall" => "Plugin: uninstall",
        "plugin.activate" => "Plugin: activate",
        "plugin.deactivate" => "Plugin: deactivate",
        "theme.install" => "Theme: install",
        "theme.update" => "Theme: update",
        "theme.uninstall" => "Theme: uninstall",
        "theme.apply" => "Theme: apply",
        "theme.update-all" => "Theme: update all",
        _ => "Panel action",
    }
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Decode a URL query value (`+` is a space, `%XX` is a byte).
pub fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h * 16 + l);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Outcome parsed from a PRG redirect `Location` such as
/// `/plugins?view=store&domain=a.com&notice=Installed+X`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub ok: bool,
    pub message: String,
    pub domain: String,
}

pub fn outcome_from_location(location: &str) -> Option<Outcome> {
    let query = location.split_once('?')?.1;
    let mut notice = None;
    let mut error = None;
    let mut domain = String::new();
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        match key {
            "notice" => notice = Some(percent_decode(value)),
            "error" => error = Some(percent_decode(value)),
            "domain" => domain = percent_decode(value),
            _ => {}
        }
    }
    match (error, notice) {
        (Some(message), _) => Some(Outcome {
            ok: false,
            message,
            domain,
        }),
        (None, Some(message)) => Some(Outcome {
            ok: true,
            message,
            domain,
        }),
        (None, None) => None,
    }
}

fn clean_field(value: &str) -> String {
    let flat: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let redacted = crate::panel_ops_activity::sanitize_log_line(&flat);
    redacted.chars().take(MAX_FIELD_CHARS).collect()
}

fn trim_if_large(path: &PathBuf) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() <= MAX_BYTES {
        return;
    }
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let lines: Vec<&str> = raw.lines().collect();
    let start = lines.len().saturating_sub(KEEP_LINES);
    let mut kept = lines[start..].join("\n");
    kept.push('\n');
    let tmp = path.with_extension("jsonl.tmp");
    if fs::write(&tmp, kept).is_ok() {
        set_private(&tmp);
        let _ = fs::rename(&tmp, path);
    }
}

fn set_private(path: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Append one record. Never panics and never blocks a request on failure; returns the I/O error
/// text so callers can surface it if they care.
pub fn record(
    actor: &str,
    action: &str,
    target: &str,
    ok: bool,
    message: &str,
) -> Result<(), String> {
    let entry = ActionRecord {
        ts: now_unix(),
        actor: clean_field(actor),
        action: clean_field(action),
        target: clean_field(target),
        ok,
        message: clean_field(message),
    };
    let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    let path = log_path();
    let _guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    trim_if_large(&path);
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    writeln!(file, "{line}").map_err(|e| format!("write {}: {e}", path.display()))
}

/// Newest-first records. `actor_filter` limits rows to one account (non-admin viewers).
pub fn recent(limit: usize, actor_filter: Option<&str>) -> Vec<ActionRecord> {
    let Ok(raw) = fs::read_to_string(log_path()) else {
        return Vec::new();
    };
    raw.lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<ActionRecord>(line).ok())
        .filter(|row| match actor_filter {
            Some(actor) => row.actor.eq_ignore_ascii_case(actor),
            None => true,
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn maps_plugin_and_host_paths() {
        assert_eq!(action_for_path("/apps/activate"), Some("host.attach"));
        assert_eq!(
            action_for_path("/plugins/install-host/"),
            Some("plugin.install-host")
        );
        assert_eq!(
            action_for_path("/api/panel/themes/update-all"),
            Some("theme.update-all")
        );
        assert_eq!(action_for_path("/plugins/settings"), None);
        assert_eq!(action_for_path("/dashboard"), None);
        assert_eq!(action_label("theme.install"), "Theme: install");
    }

    #[test]
    fn decodes_location_notice_and_domain() {
        let out = outcome_from_location(
            "/plugins?view=store&category=Host&domain=a.example.com&notice=Installed%20X+on+Host%21",
        )
        .expect("outcome");
        assert!(out.ok);
        assert_eq!(out.domain, "a.example.com");
        assert_eq!(out.message, "Installed X on Host!");
    }

    #[test]
    fn error_wins_and_flags_failure() {
        let out = outcome_from_location("/plugins?error=Nope&notice=x").expect("outcome");
        assert!(!out.ok);
        assert_eq!(out.message, "Nope");
        assert!(outcome_from_location("/plugins?view=store").is_none());
        assert!(outcome_from_location("/plugins").is_none());
    }

    #[test]
    fn percent_decode_handles_truncated_escapes() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
    }

    #[test]
    fn record_round_trips_newest_first_and_redacts() {
        with_test_data_dir(|| {
            record(
                "cpnowner",
                "plugin.install-host",
                "",
                true,
                "Installed MariaDB",
            )
            .unwrap();
            record(
                "cpnowner",
                "host.attach",
                "a.example.com",
                false,
                "failed password=hunter2 now",
            )
            .unwrap();
            let rows = recent(10, None);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].action, "host.attach");
            assert!(!rows[0].message.contains("hunter2"));
            assert_eq!(recent(10, Some("other")).len(), 0);
            assert_eq!(recent(10, Some("CPNOWNER")).len(), 2);
            assert_eq!(recent(1, None).len(), 1);
        });
    }
}
