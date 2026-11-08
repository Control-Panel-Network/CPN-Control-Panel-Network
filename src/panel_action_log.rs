//! Panel action / audit log facade (legacy name kept for call sites).
//!
//! Storage and schema live in [`crate::panel_audit`]. This module re-exports the
//! record API and PRG helpers so existing callers keep compiling.

use crate::panel_audit;
use crate::panel_audit_paths;
use crate::panel_audit_types::AuditRecord;

pub use crate::panel_audit::{log_path, recent, record, record_full};
pub use crate::panel_audit_paths::{action_for_path, action_label};
pub use crate::panel_audit_types::AuditRecord as ActionRecord;

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

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Outcome parsed from a PRG redirect `Location`.
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

pub type LegacyActionRecord = AuditRecord;

pub fn ensure_store() {
    let _ = panel_audit::apply_retention_now();
}

pub fn is_noisy_read_like(path: &str) -> bool {
    panel_audit_paths::is_noisy_read_like(path)
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
