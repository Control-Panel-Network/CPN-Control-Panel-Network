//! Map one host log line into the card fields used by every Server > Logs viewer:
//! Source, Level (when the line carries one) and a readable Message.
//!
//! The raw line is never lost: `Fields::parsed` tells the UI to keep it behind a Raw line toggle.

use crate::panel_server_logs::LogEntry;
use crate::panel_server_logs_kind::HostLogKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelClass {
    Ok,
    Info,
    Warn,
    Err,
    Debug,
}

impl LevelClass {
    pub fn css(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Err => "err",
            Self::Debug => "debug",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub label: String,
    pub class: LevelClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fields {
    /// Domain, `server`, `sftp`, or the logging program; empty when unknown.
    pub source: String,
    pub level: Option<Level>,
    pub message: String,
    /// True when `message` differs from the raw text (so the raw line is offered separately).
    pub parsed: bool,
}

/// Severity word to a badge class. Accepts Apache style `core:error` by its last segment.
pub fn severity_class(word: &str) -> Option<LevelClass> {
    let w = word.rsplit(':').next().unwrap_or(word).to_ascii_lowercase();
    match w.as_str() {
        "emerg" | "emergency" | "alert" | "crit" | "critical" | "fatal" | "panic" | "error"
        | "err" | "severe" => Some(LevelClass::Err),
        "warn" | "warning" => Some(LevelClass::Warn),
        "notice" | "info" | "information" => Some(LevelClass::Info),
        "debug" | "trace" => Some(LevelClass::Debug),
        _ => None,
    }
}

fn http_level(code: &str) -> Option<Level> {
    let n: u16 = code.parse().ok()?;
    let class = match n {
        200..=299 => LevelClass::Ok,
        300..=399 => LevelClass::Info,
        400..=499 => LevelClass::Warn,
        500..=599 => LevelClass::Err,
        _ => return None,
    };
    Some(Level {
        label: code.to_string(),
        class,
    })
}

/// Common/combined access line: `IP - user "METHOD /path HTTP/1.1" 404 1249 "ref" "ua"`.
fn access_fields(text: &str) -> Option<(String, Level)> {
    let q1 = text.find('"')?;
    let after = &text[q1 + 1..];
    let q2 = after.find('"')?;
    let request = &after[..q2];
    let mut tail = after[q2 + 1..].split_whitespace();
    let status = tail.next()?;
    let bytes = tail.next().unwrap_or("-");
    let level = http_level(status)?;
    let client = text[..q1].split_whitespace().next().unwrap_or("-");
    let mut req = request.split_whitespace();
    let method = req.next()?;
    let path = req.next().unwrap_or("/");
    let proto = req.next().unwrap_or("");
    let proto = if proto.is_empty() {
        String::new()
    } else {
        format!(" ({proto})")
    };
    let size = if bytes == "-" {
        String::new()
    } else {
        format!(", {bytes} bytes")
    };
    Some((format!("{method} {path}{proto} from {client}{size}"), level))
}

/// `[NOTICE] [12853] Server Stopped!` or `[core:error] [pid 1] ...`: the first bracket is the level.
fn bracket_level(text: &str) -> Option<(Level, String)> {
    let rest = text.strip_prefix('[')?;
    let end = rest.find(']')?;
    let class = severity_class(&rest[..end])?;
    Some((
        Level {
            label: rest[..end]
                .rsplit(':')
                .next()
                .unwrap_or(&rest[..end])
                .to_ascii_uppercase(),
            class,
        },
        rest[end + 1..].trim_start().to_string(),
    ))
}

/// `host prog[pid]: message` or `prog[pid]: message`, as written by syslog and journald.
fn syslog_program(text: &str) -> Option<(String, String)> {
    let (head, msg) = text.split_once(": ")?;
    if head.contains('"') || head.split_whitespace().count() > 2 || head.is_empty() {
        return None;
    }
    let token = head.split_whitespace().last()?;
    // A bare word before a colon ("Result: ...") is ordinary text, not a program name.
    let looks_like_program = token.contains('[') || token.contains('/');
    if !looks_like_program {
        return None;
    }
    let prog = token.split('[').next().unwrap_or(token);
    if prog.is_empty()
        || !prog
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '@'))
    {
        return None;
    }
    Some((prog.to_string(), msg.trim_start().to_string()))
}

/// Leading `error:` / `warning:` style marker (Postfix, Dovecot, sshd).
fn prefix_level(text: &str) -> Option<Level> {
    let first = text.split_whitespace().next()?;
    let word = first.strip_suffix(':')?;
    let class = severity_class(word)?;
    Some(Level {
        label: word.to_ascii_uppercase(),
        class,
    })
}

/// ModSecurity summary: `client METHOD uri -> 403 [blocked] | rule ...`.
fn modsec_level(text: &str) -> Option<Level> {
    let after = text.split(" -> ").nth(1)?;
    let code = after.split_whitespace().next()?;
    let mut level = http_level(code)?;
    if after.contains("[blocked]") {
        level.class = LevelClass::Err;
    }
    Some(level)
}

pub fn parse(kind: HostLogKind, entry: &LogEntry) -> Fields {
    let raw = entry.text.as_str();
    let mut source = entry.scope.clone();
    let mut message = raw.to_string();
    let mut level: Option<Level> = None;

    if kind == HostLogKind::Access
        && let Some((msg, lvl)) = access_fields(raw)
    {
        message = msg;
        level = Some(lvl);
    }
    if kind == HostLogKind::ModSec {
        level = modsec_level(raw);
    }
    if level.is_none()
        && let Some((lvl, rest)) = bracket_level(raw)
    {
        level = Some(lvl);
        message = rest;
    }
    if source.is_empty()
        && let Some((prog, rest)) = syslog_program(&message)
    {
        source = prog;
        message = rest;
    }
    if level.is_none() {
        level = prefix_level(&message);
    }
    if message.is_empty() {
        message = raw.to_string();
    }
    let parsed = message != raw;
    Fields {
        source,
        level,
        message,
        parsed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(scope: &str, text: &str) -> LogEntry {
        LogEntry {
            stamp: "01/10/2026 15:46:59".into(),
            scope: scope.into(),
            text: text.into(),
        }
    }

    #[test]
    fn access_line_becomes_request_status_and_client() {
        let f = parse(
            HostLogKind::Access,
            &entry(
                "test2.example.com",
                r#"127.0.0.1 - - "GET /missing-1 HTTP/1.1" 404 1249 "-" "curl/8.12.1""#,
            ),
        );
        assert_eq!(f.source, "test2.example.com");
        let level = f.level.unwrap();
        assert_eq!(
            (level.label.as_str(), level.class),
            ("404", LevelClass::Warn)
        );
        assert_eq!(
            f.message,
            "GET /missing-1 (HTTP/1.1) from 127.0.0.1, 1249 bytes"
        );
        assert!(f.parsed);
    }

    #[test]
    fn server_error_status_is_an_error_level() {
        let f = parse(
            HostLogKind::Access,
            &entry("a", r#"1.2.3.4 - - "POST /x HTTP/2" 502 -"#),
        );
        assert_eq!(f.level.unwrap().class, LevelClass::Err);
        assert_eq!(f.message, "POST /x (HTTP/2) from 1.2.3.4");
    }

    #[test]
    fn error_log_level_is_split_from_the_message() {
        let f = parse(
            HostLogKind::Error,
            &entry("server", "[NOTICE] [12853] Server Stopped!"),
        );
        let level = f.level.unwrap();
        assert_eq!(
            (level.label.as_str(), level.class),
            ("NOTICE", LevelClass::Info)
        );
        assert_eq!(f.message, "[12853] Server Stopped!");
        assert_eq!(f.source, "server");
        let apache = parse(
            HostLogKind::Error,
            &entry("a.example", "[core:error] [pid 1] AH00126: Invalid URI"),
        );
        assert_eq!(apache.level.unwrap().label, "ERROR");
    }

    #[test]
    fn syslog_program_becomes_source_when_no_scope() {
        let f = parse(
            HostLogKind::Email,
            &entry("", "mail postfix/smtpd[881]: warning: hostname mismatch"),
        );
        assert_eq!(f.source, "postfix/smtpd");
        assert_eq!(f.level.unwrap().class, LevelClass::Warn);
        assert_eq!(f.message, "warning: hostname mismatch");
        // A scope from the collector wins and the line is left whole.
        let sftp = parse(
            HostLogKind::Ftp,
            &entry("sftp", "host sshd[1]: open \"/a\" flags READ"),
        );
        assert_eq!(sftp.source, "sftp");
        assert!(!sftp.parsed);
    }

    #[test]
    fn plain_text_is_left_untouched_without_level() {
        let f = parse(HostLogKind::Panel, &entry("", "Started the panel service"));
        assert_eq!(f.message, "Started the panel service");
        assert!(f.level.is_none());
        assert!(f.source.is_empty());
        assert!(!f.parsed);
    }

    #[test]
    fn modsec_status_and_block_are_levelled() {
        let f = parse(
            HostLogKind::ModSec,
            &entry(
                "",
                "203.0.113.9 GET /?id=1 -> 403 [blocked] | rule 942100: SQLi",
            ),
        );
        assert_eq!(f.level.unwrap().class, LevelClass::Err);
    }

    #[test]
    fn colon_in_ordinary_text_is_not_a_program() {
        let f = parse(
            HostLogKind::Panel,
            &entry("", "Result: it worked fine here ok"),
        );
        assert!(f.source.is_empty());
    }
}
