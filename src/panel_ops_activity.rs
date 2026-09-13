//! SSH auth log snapshots and light security analysis for the Activity Board.

use crate::panel_ops_security::{firewall_status, sshd_status};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

const MAX_LINES: usize = 200;
const ANALYZE_LINES: usize = 500;
const MAX_LINE_CHARS: usize = 420;

#[derive(Debug, Clone)]
pub struct ActivityLogRow {
    pub timestamp: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SshSecurityAnalysis {
    pub status_label: String,
    pub logs_analyzed: usize,
    pub failed_logins: usize,
    pub accepted_logins: usize,
    pub firewall_label: String,
    pub tips: Vec<String>,
    pub alert_count: usize,
}

/// Redact credential-like fragments and truncate for safe UI display.
pub fn sanitize_log_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(trimmed.len().min(MAX_LINE_CHARS));
    let lower = trimmed.to_ascii_lowercase();
    let redact_keys = [
        "password=",
        "password:",
        "passwd=",
        "passwd:",
        "pwd=",
        "secret=",
        "token=",
        "authorization:",
        "api_key=",
        "apikey=",
    ];
    let mut skip_until_space = false;
    let mut i = 0usize;
    let bytes = trimmed.as_bytes();
    while i < bytes.len() && out.chars().count() < MAX_LINE_CHARS {
        if skip_until_space {
            if bytes[i].is_ascii_whitespace() {
                skip_until_space = false;
                out.push(bytes[i] as char);
            }
            i += 1;
            continue;
        }
        let rest_lower = &lower[i..];
        let mut matched = None;
        for key in redact_keys {
            if rest_lower.starts_with(key) {
                matched = Some(key.len());
                break;
            }
        }
        if let Some(key_len) = matched {
            out.push_str(&trimmed[i..i + key_len]);
            out.push_str("***");
            i += key_len;
            skip_until_space = true;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    if i < bytes.len() {
        out.push('…');
    }
    out
}

fn split_timestamp(line: &str) -> (String, String) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 4
        && parts[0].len() == 3
        && parts[0]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].contains(':')
    {
        let ts = format!("{} {} {}", parts[0], parts[1], parts[2]);
        let msg = parts[3..].join(" ");
        return (ts, msg);
    }
    if let Some((head, rest)) = line.split_once(' ')
        && (head.contains('T') || (head.len() >= 10 && head.as_bytes().get(4) == Some(&b'-')))
    {
        return (head.to_string(), rest.to_string());
    }
    ("-".into(), line.to_string())
}

fn auth_log_candidates() -> [&'static str; 4] {
    [
        "/var/log/secure",
        "/var/log/auth.log",
        "/var/log/messages",
        "/var/log/syslog",
    ]
}

fn tail_file(path: &Path, lines: usize) -> Option<String> {
    let out = Command::new("tail")
        .args(["-n", &lines.to_string()])
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn read_file_tail(path: &Path, lines: usize) -> Option<String> {
    if let Some(t) = tail_file(path, lines) {
        return Some(t);
    }
    let raw = fs::read_to_string(path).ok()?;
    let collected: Vec<&str> = raw.lines().rev().take(lines).collect();
    if collected.is_empty() {
        return None;
    }
    Some(collected.into_iter().rev().collect::<Vec<_>>().join("\n"))
}

fn journal_auth_excerpt(lines: usize) -> Option<String> {
    let out = Command::new("journalctl")
        .args([
            "-n",
            &lines.to_string(),
            "--no-pager",
            "-o",
            "short-iso",
            "_COMM=sshd",
            "+",
            "_COMM=ssh",
            "+",
            "SYSLOG_IDENTIFIER=sshd",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn collect_raw_auth_text(lines: usize) -> (String, usize) {
    for candidate in auth_log_candidates() {
        let path = Path::new(candidate);
        if !path.is_file() {
            continue;
        }
        if let Some(text) = read_file_tail(path, lines) {
            let count = text.lines().filter(|l| !l.trim().is_empty()).count();
            return (text, count);
        }
    }
    if let Some(text) = journal_auth_excerpt(lines) {
        let count = text.lines().filter(|l| !l.trim().is_empty()).count();
        return (text, count);
    }
    (String::new(), 0)
}

fn is_ssh_related(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    l.contains("sshd")
        || l.contains("ssh[")
        || l.contains("accepted")
        || l.contains("failed password")
        || l.contains("invalid user")
        || l.contains("authentication failure")
        || l.contains("connection closed by")
        || l.contains("session opened")
        || l.contains("session closed")
}

fn is_login_success(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    (l.contains("accepted password")
        || l.contains("accepted publickey")
        || l.contains("accepted keyboard-interactive")
        || l.contains("session opened for user"))
        && l.contains("sshd")
}

fn is_login_failure(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    l.contains("failed password")
        || l.contains("authentication failure")
        || l.contains("invalid user")
        || l.contains("failed none")
        || (l.contains("connection closed by authenticating user") && l.contains("sshd"))
}

fn rows_from_text(text: &str, filter: impl Fn(&str) -> bool, limit: usize) -> Vec<ActivityLogRow> {
    let mut rows = Vec::new();
    for line in text.lines().rev() {
        let line = line.trim();
        if line.is_empty() || !filter(line) {
            continue;
        }
        let clean = sanitize_log_line(line);
        if clean.is_empty() {
            continue;
        }
        let (ts, msg) = split_timestamp(&clean);
        rows.push(ActivityLogRow {
            timestamp: if ts.is_empty() { "-".into() } else { ts },
            message: msg,
        });
        if rows.len() >= limit {
            break;
        }
    }
    rows.reverse();
    rows
}

/// Recent successful SSH / session logins.
pub fn recent_ssh_logins(limit: usize) -> Vec<ActivityLogRow> {
    let (text, _) = collect_raw_auth_text(ANALYZE_LINES);
    if text.is_empty() {
        return Vec::new();
    }
    rows_from_text(&text, is_login_success, limit.clamp(1, MAX_LINES))
}

/// Recent SSH-related log lines (broader than logins only).
pub fn recent_ssh_logs(limit: usize) -> (Vec<ActivityLogRow>, usize) {
    let (text, analyzed) = collect_raw_auth_text(ANALYZE_LINES);
    if text.is_empty() {
        return (Vec::new(), 0);
    }
    let rows = rows_from_text(&text, is_ssh_related, limit.clamp(1, MAX_LINES));
    (rows, analyzed)
}

pub fn ssh_security_analysis() -> SshSecurityAnalysis {
    let (text, analyzed) = collect_raw_auth_text(ANALYZE_LINES);
    let mut failed = 0usize;
    let mut accepted = 0usize;
    for line in text.lines() {
        if is_login_failure(line) {
            failed += 1;
        }
        if is_login_success(line) {
            accepted += 1;
        }
    }

    let fw = firewall_status();
    let firewall_label = if fw.backend.is_empty() {
        "Unknown".into()
    } else {
        fw.backend.to_ascii_uppercase()
    };

    let ssh = sshd_status();
    let mut tips = Vec::new();
    let root = ssh.permit_root_login.to_ascii_lowercase();
    if root == "yes" || root.contains("default") {
        tips.push(
            "Disable root login: set PermitRootLogin no (or prohibit-password) in sshd_config."
                .into(),
        );
    }
    let pwd = ssh.password_authentication.to_ascii_lowercase();
    if pwd == "yes" {
        tips.push(
            "Prefer key-based auth: set PasswordAuthentication no after confirming keys work."
                .into(),
        );
    }
    if !fw.active {
        tips.push("Enable a host firewall and allow only the ports you need.".into());
    }
    if tips.is_empty() {
        tips.push("Keep sshd and the firewall updated; review failed logins regularly.".into());
    }

    let alert_count = if failed >= 25 { 1 } else { 0 };
    let status_label = if failed == 0 {
        "No failed logins in sample"
    } else if failed < 25 {
        "Failed logins observed (review recommended)"
    } else {
        "Elevated failed logins (review recommended)"
    };

    SshSecurityAnalysis {
        status_label: status_label.into(),
        logs_analyzed: analyzed,
        failed_logins: failed,
        accepted_logins: accepted,
        firewall_label,
        tips,
        alert_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_redacts_password_values() {
        let with_eq = sanitize_log_line("user login password=supersecret token=abc123 done");
        assert!(with_eq.contains("password=***"));
        assert!(with_eq.contains("token=***"));
        assert!(!with_eq.contains("supersecret"));
        assert!(!with_eq.contains("abc123"));
        let plain = sanitize_log_line("sshd[1]: Failed password for root from 10.0.0.1 port 22");
        assert!(plain.contains("Failed password"));
    }

    #[test]
    fn sanitize_truncates_long_lines() {
        let long = format!("prefix {}", "x".repeat(800));
        let out = sanitize_log_line(&long);
        assert!(out.chars().count() <= MAX_LINE_CHARS + 1);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn login_classifiers() {
        assert!(is_login_success(
            "sshd[12]: Accepted publickey for alice from 1.2.3.4 port 22 ssh2"
        ));
        assert!(is_login_failure(
            "sshd[12]: Failed password for invalid user bob from 1.2.3.4 port 22 ssh2"
        ));
        assert!(is_ssh_related(
            "sshd[12]: Connection closed by 1.2.3.4 port 22"
        ));
    }

    #[test]
    fn collectors_do_not_panic() {
        let _ = recent_ssh_logins(5);
        let _ = recent_ssh_logs(5);
        let _ = ssh_security_analysis();
    }
}
