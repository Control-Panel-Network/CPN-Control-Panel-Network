//! Audit export (JSONL/CSV) and optional webhook forward (no secrets in payloads).

use crate::panel_audit::recent_filtered;
use crate::panel_audit_policy::load_audit_policy;
use crate::panel_audit_types::AuditRecord;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static LAST_WEBHOOK_AT: AtomicU64 = AtomicU64::new(0);

/// Build a downloadable JSONL document (newest-first, filtered).
pub fn export_jsonl(
    limit: usize,
    actor_filter: Option<&str>,
    category_filter: Option<&str>,
) -> String {
    let rows = recent_filtered(limit, actor_filter, category_filter, None);
    let mut out = String::new();
    for row in rows {
        if let Ok(line) = serde_json::to_string(&row) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// Build a CSV export with a header row. Fields are escaped; secrets already redacted at write.
pub fn export_csv(
    limit: usize,
    actor_filter: Option<&str>,
    category_filter: Option<&str>,
) -> String {
    let rows = recent_filtered(limit, actor_filter, category_filter, None);
    let mut out =
        String::from("ts,actor,action,target,ok,message,ip,ua,request_id,category,severity\n");
    for row in rows {
        out.push_str(&csv_row(&row));
        out.push('\n');
    }
    out
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn csv_row(row: &AuditRecord) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{},{}",
        row.ts,
        csv_escape(&row.actor),
        csv_escape(&row.action),
        csv_escape(&row.target),
        if row.ok { "1" } else { "0" },
        csv_escape(&row.message),
        csv_escape(&row.ip),
        csv_escape(&row.ua),
        csv_escape(&row.request_id),
        csv_escape(&row.category),
        csv_escape(&row.severity),
    )
}

/// Best-effort webhook POST of a single event. Rate-limited to one call per 2 seconds.
pub fn maybe_forward_webhook(entry: &AuditRecord) {
    let policy = load_audit_policy();
    let url = policy.webhook_url.trim();
    if url.is_empty() || !(url.starts_with("https://") || url.starts_with("http://127.")) {
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let prev = LAST_WEBHOOK_AT.load(Ordering::Relaxed);
    if now.saturating_sub(prev) < 2 {
        return;
    }
    LAST_WEBHOOK_AT.store(now, Ordering::Relaxed);
    let Ok(body) = serde_json::to_string(entry) else {
        return;
    };
    let url_owned = url.to_string();
    std::thread::spawn(move || {
        let _ = std::process::Command::new("curl")
            .args([
                "-sS",
                "-m",
                "3",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
                "-d",
                &body,
                &url_owned,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_audit::record;

    #[test]
    fn export_includes_header_and_row() {
        with_test_data_dir(|| {
            record("alice", "auth.login", "", true, "ok").unwrap();
            let csv = export_csv(10, None, Some("auth"));
            assert!(csv.starts_with("ts,actor,action"));
            assert!(csv.contains("auth.login"));
            let jsonl = export_jsonl(10, Some("alice"), None);
            assert!(jsonl.contains("\"actor\":\"alice\""));
        });
    }
}
