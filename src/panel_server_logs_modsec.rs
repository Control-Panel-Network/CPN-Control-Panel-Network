//! ModSecurity audit log reader for Server > Logs > ModSec Audit.
//!
//! Serial (`--id-A--` sections) and JSON audit logs are condensed to one line per transaction. The
//! matched request data is never copied, only the rule id and rule message, so request bodies and
//! credentials stay out of the viewer.

use crate::panel_server_logs_kind::HostLogKind;
use crate::panel_server_logs_sources::{Collected, TAIL_BYTES, read_tail};
use crate::panel_ops_security_ssl::modsec_status;
use std::path::Path;
fn quoted(haystack: &str, key: &str) -> Option<String> {
    let start = haystack.find(key)? + key.len();
    let end = haystack[start..].find('"')?;
    Some(haystack[start..start + end].to_string())
}

/// `--c8a4b6d1-A--` gives the section letter `A`.
fn boundary(line: &str) -> Option<char> {
    let inner = line.strip_prefix("--")?.strip_suffix("--")?;
    let (id, section) = inner.rsplit_once('-')?;
    let mut chars = section.chars();
    let c = chars.next()?;
    (chars.next().is_none() && c.is_ascii_uppercase() && !id.is_empty()).then_some(c)
}

#[derive(Default)]
struct Txn {
    section: char,
    seen: usize,
    header: String,
    request: String,
    response: String,
    messages: Vec<String>,
    intercepted: bool,
}

impl Txn {
    fn feed(&mut self, line: &str) {
        if line.trim().is_empty() {
            return;
        }
        self.seen += 1;
        match self.section {
            'A' if self.seen == 1 => self.header = line.to_string(),
            'B' if self.seen == 1 => self.request = line.to_string(),
            'F' if self.seen == 1 => self.response = line.to_string(),
            'H' => {
                if let Some(msg) = line.strip_prefix("Message:") {
                    self.messages.push(msg.trim().to_string());
                } else if line.starts_with("Action: Intercepted") {
                    self.intercepted = true;
                }
            }
            _ => {}
        }
    }

    fn summary(&self) -> Option<String> {
        if self.header.is_empty() {
            return None;
        }
        let (stamp, rest) = match self.header.find(']') {
            Some(i) => (&self.header[..=i], &self.header[i + 1..]),
            None => ("", self.header.as_str()),
        };
        let client = rest.split_whitespace().nth(1).unwrap_or("-");
        let code = self.response.split_whitespace().nth(1).unwrap_or("-");
        let mut rules: Vec<String> = self
            .messages
            .iter()
            .take(3)
            .map(|m| match (quoted(m, "[id \""), quoted(m, "[msg \"")) {
                (Some(id), Some(msg)) => format!("rule {id}: {msg}"),
                (None, Some(msg)) => msg,
                (Some(id), None) => format!("rule {id}"),
                (None, None) => m.chars().take(120).collect(),
            })
            .collect();
        if self.messages.len() > 3 {
            rules.push(format!("+{} more", self.messages.len() - 3));
        }
        let blocked = if self.intercepted { " [blocked]" } else { "" };
        let request = if self.request.is_empty() { "-" } else { &self.request };
        Some(format!(
            "{stamp} {client} {request} -> {code}{blocked} | {}",
            if rules.is_empty() {
                "no rule message".to_string()
            } else {
                rules.join("; ")
            }
        ))
    }
}

fn modsec_json_summary(line: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let t = v.get("transaction")?;
    let s = |p: &serde_json::Value, k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("-").to_string();
    let req = t.get("request").cloned().unwrap_or_default();
    let code = t
        .get("response")
        .and_then(|r| r.get("http_code"))
        .map(|c| c.to_string())
        .unwrap_or_else(|| "-".into());
    let rules: Vec<String> = t
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|a| {
            a.iter()
                .take(3)
                .map(|m| {
                    let id = m
                        .get("details")
                        .and_then(|d| d.get("ruleId"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    let msg = m.get("message").and_then(|x| x.as_str()).unwrap_or("");
                    format!("rule {id}: {msg}")
                })
                .collect()
        })
        .unwrap_or_default();
    Some(format!(
        "{} {} {} {} -> {} | {}",
        s(t, "time_stamp"),
        s(t, "client_ip"),
        s(&req, "method"),
        s(&req, "uri"),
        code,
        if rules.is_empty() {
            "no rule message".to_string()
        } else {
            rules.join("; ")
        }
    ))
}

/// One line per ModSecurity audit transaction (serial or JSON format), oldest first.
pub fn modsec_summaries(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<Txn> = None;
    let flush = |cur: &mut Option<Txn>, out: &mut Vec<String>| {
        if let Some(t) = cur.take()
            && let Some(s) = t.summary()
        {
            out.push(s);
        }
    };
    for line in text.lines() {
        if line.starts_with('{') {
            if let Some(s) = modsec_json_summary(line) {
                out.push(s);
            }
            continue;
        }
        if let Some(section) = boundary(line) {
            match section {
                'A' => {
                    flush(&mut cur, &mut out);
                    cur = Some(Txn::default());
                }
                'Z' => {
                    flush(&mut cur, &mut out);
                    continue;
                }
                _ => {}
            }
            if let Some(t) = cur.as_mut() {
                t.section = section;
                t.seen = 0;
            }
            continue;
        }
        if let Some(t) = cur.as_mut() {
            t.feed(line);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// True when any ModSecurity module (OpenLiteSpeed, Apache, Nginx) is installed.
fn modsec_installed() -> bool {
    Path::new("/usr/local/lsws/modules/mod_security.so").exists() || modsec_status().detected
}

pub fn collect_modsec() -> Option<Collected> {
    let mut out = Collected::default();
    let mut existing: Option<&str> = None;
    for &path in HostLogKind::ModSec.files() {
        let p = Path::new(path);
        if !p.is_file() {
            continue;
        }
        existing.get_or_insert(path);
        if let Some(text) = read_tail(p, TAIL_BYTES)
            && !text.trim().is_empty()
        {
            let lines = modsec_summaries(&text);
            if !lines.is_empty() {
                out.sources.push(format!("{path} (audit log)"));
                out.push_text("modsec", &lines.join("\n"));
                return Some(out);
            }
        }
    }
    // No audit transaction yet: fall back to ModSecurity lines in the web server error log.
    for &path in HostLogKind::Error.files() {
        let p = Path::new(path);
        if let Some(text) = p.is_file().then(|| read_tail(p, TAIL_BYTES)).flatten() {
            let hits: Vec<&str> = text
                .lines()
                .filter(|l| l.to_ascii_lowercase().contains("modsecurity"))
                .collect();
            if !hits.is_empty() {
                out.sources.push(format!("{path} (ModSecurity lines)"));
                out.push_text("error.log", &hits.join("\n"));
                return Some(out);
            }
        }
    }
    if let Some(path) = existing {
        out.sources.push(format!("{path} (audit log, no transactions yet)"));
        return Some(out);
    }
    if modsec_installed() {
        out.sources
            .push("ModSecurity module detected (no audit log written yet)".into());
        return Some(out);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERIAL: &str = "--c8a4b6d1-A--\n[01/Oct/2026:12:00:00 +0200] ZxYw 203.0.113.9 51234 10.0.0.2 80\n--c8a4b6d1-B--\nGET /?id=1%27%20OR%201=1 HTTP/1.1\nHost: example.com\n--c8a4b6d1-F--\nHTTP/1.1 403 Forbidden\n--c8a4b6d1-H--\nMessage: Access denied with code 403 (phase 2). detected SQLi. [file \"/x.conf\"] [id \"942100\"] [msg \"SQL Injection Attack Detected\"] [data \"Matched Data: password=hunter2\"]\nAction: Intercepted (phase 2)\n--c8a4b6d1-Z--\n";

    #[test]
    fn serial_audit_is_condensed_without_leaking_matched_data() {
        let lines = modsec_summaries(SERIAL);
        assert_eq!(lines.len(), 1);
        let l = &lines[0];
        assert!(l.starts_with("[01/Oct/2026:12:00:00 +0200] 203.0.113.9 GET /?id="));
        assert!(l.contains("-> 403 [blocked]"));
        assert!(l.contains("rule 942100: SQL Injection Attack Detected"));
        assert!(!l.contains("hunter2"));
    }

    #[test]
    fn partial_leading_transaction_is_skipped() {
        let text = format!("Host: cut\n--c8a4b6d1-H--\nMessage: stray\n{SERIAL}");
        assert_eq!(modsec_summaries(&text).len(), 1);
    }

    #[test]
    fn json_audit_line_is_condensed() {
        let line = r#"{"transaction":{"time_stamp":"Thu Oct  1 12:00:00 2026","client_ip":"198.51.100.4","request":{"method":"GET","uri":"/admin"},"response":{"http_code":403},"messages":[{"message":"Blocked","details":{"ruleId":"949110"}}]}}"#;
        let lines = modsec_summaries(line);
        assert_eq!(
            lines,
            vec!["Thu Oct  1 12:00:00 2026 198.51.100.4 GET /admin -> 403 | rule 949110: Blocked"]
        );
    }
}
