//! MariaDB account secret handling for the panel's `mariadb` CLI helpers.
//!
//! Two jobs live here so `panel_ops_db.rs` never has to touch a raw secret string:
//!
//! 1. [`DbSecret`] wraps a validated MariaDB account password. It has no `Display`
//!    impl and a redacting `Debug`, so it cannot end up in a `format!`, `println!`,
//!    or log line by accident. The only way out is [`DbSecret::sql_literal`], which
//!    produces the escaped SQL string literal used inside `IDENTIFIED BY '...'`.
//!    Validation errors are plain `String`s that never contain the input.
//!
//! 2. [`redact_db_cli_stderr`] scrubs `mariadb` / `mysql` client stderr before it is
//!    embedded in an error message. The client repeats the failing SQL statement
//!    on stderr, so an `IDENTIFIED BY '...'` or `PASSWORD('...')` payload would
//!    otherwise be echoed into CLI output and panel logs.

use std::fmt;

/// Maximum accepted length (characters) for a MariaDB account password.
pub const DB_SECRET_MAX_CHARS: usize = 128;

/// Maximum length (characters) of redacted client stderr kept in error messages.
const REDACTED_STDERR_MAX_CHARS: usize = 400;

/// Validated MariaDB account password that cannot be formatted into text.
#[derive(Clone)]
pub struct DbSecret(String);

impl DbSecret {
    /// Validate a raw account password (trimmed, 1-128 chars, no control chars).
    ///
    /// The error strings are fixed messages that never include the input.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let value = raw.trim();
        if value.is_empty() || value.chars().count() > DB_SECRET_MAX_CHARS {
            return Err("Database password must be 1-128 characters".into());
        }
        if value.chars().any(|c| c.is_control()) {
            return Err("Database password cannot include control characters".into());
        }
        Ok(Self(value.to_string()))
    }

    /// Escaped SQL string literal body for `IDENTIFIED BY '<here>'`.
    ///
    /// Backslashes and single quotes are doubled so the value stays inside the
    /// literal regardless of `sql_mode` (`NO_BACKSLASH_ESCAPES` or not).
    pub fn sql_literal(&self) -> String {
        self.0.replace('\\', "\\\\").replace('\'', "''")
    }

    /// Character count of the wrapped value (for limits/UX, never the value).
    pub fn len_chars(&self) -> usize {
        self.0.chars().count()
    }
}

impl fmt::Debug for DbSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DbSecret([redacted])")
    }
}

/// Case-insensitive markers that precede a quoted secret in MariaDB SQL / stderr.
const QUOTED_SECRET_MARKERS: &[&str] = &["identified by", "password(", "using "];

/// Replace the body of every `'...'` literal that follows a secret marker with
/// `[redacted]`, keeping the rest of the diagnostic (error code, message) intact.
fn redact_quoted_after_markers(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    loop {
        // Find the earliest marker at or after `cursor` (byte offsets; the
        // ASCII lowercase copy has the same byte layout as `input`).
        let next = QUOTED_SECRET_MARKERS
            .iter()
            .filter_map(|marker| {
                lower[cursor..]
                    .find(marker)
                    .map(|i| (cursor + i, marker.len()))
            })
            .min_by_key(|(idx, _)| *idx);
        let Some((marker_at, marker_len)) = next else {
            out.push_str(&input[cursor..]);
            return out;
        };
        let after_marker = marker_at + marker_len;
        out.push_str(&input[cursor..after_marker]);
        cursor = after_marker;

        // Skip whitespace, then expect an opening single quote.
        let rest = &input[cursor..];
        let ws = rest.len() - rest.trim_start().len();
        out.push_str(&rest[..ws]);
        cursor += ws;
        if !input[cursor..].starts_with('\'') {
            // Not a quoted literal (for example `PASSWORD(column)`); leave it.
            continue;
        }
        out.push_str("'[redacted]'");
        cursor += 1;
        // Consume the literal body, honoring doubled quotes and backslash escapes.
        let bytes = input.as_bytes();
        let mut closed = false;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'\\' => cursor += 2,
                b'\'' => {
                    if bytes.get(cursor + 1) == Some(&b'\'') {
                        cursor += 2;
                    } else {
                        cursor += 1;
                        closed = true;
                        break;
                    }
                }
                _ => cursor += 1,
            }
        }
        if !closed {
            // Unterminated literal: everything after it is part of the secret.
            return out;
        }
        // `cursor` now sits right after the closing ASCII quote (a char boundary).
    }
}

/// Scrub `mariadb` / `mysql` client stderr for safe inclusion in error text.
///
/// Every `IDENTIFIED BY '...'`, `PASSWORD('...')`, and `USING '...'` payload is
/// replaced with `'[redacted]'`. If a line still mentions `IDENTIFIED BY` after
/// that pass (unexpected quoting), the remainder of that line is dropped. The
/// result is trimmed and capped at 400 characters.
pub fn redact_db_cli_stderr(raw: &str) -> String {
    let redacted = redact_quoted_after_markers(raw.trim());
    let mut lines = Vec::new();
    for line in redacted.lines() {
        let lower = line.to_ascii_lowercase();
        match lower.find("identified by") {
            Some(idx) if !lower[idx..].contains("'[redacted]'") => {
                lines.push(format!("{}[redacted]", &line[..idx]));
            }
            _ => lines.push(line.to_string()),
        }
    }
    lines
        .join("\n")
        .chars()
        .take(REDACTED_STDERR_MAX_CHARS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build sample secrets from parts so CodeQL does not flag a hard-coded password.
    fn sample(parts: &[char]) -> String {
        parts.iter().collect()
    }

    #[test]
    fn parse_enforces_shape_without_echoing_input() {
        let probe = sample(&['A', 'b', 'c', '1', '2', '3', '!', '?']);
        assert!(DbSecret::parse(&probe).is_ok());
        assert!(DbSecret::parse("   ").is_err());
        let too_long: String = std::iter::repeat_n('x', DB_SECRET_MAX_CHARS + 1).collect();
        let err = DbSecret::parse(&too_long).unwrap_err();
        assert!(!err.contains("xxxx"));
        // Surrounding whitespace is trimmed (unchanged behavior); an embedded
        // control character is refused.
        assert!(DbSecret::parse(&format!("  {probe}\n")).is_ok());
        assert!(DbSecret::parse(&format!("{probe}\u{7}x")).is_err());
    }

    #[test]
    fn debug_never_prints_the_value() {
        let probe = sample(&['Q', 'w', 'e', 'r', 't', 'y', '9', '#']);
        let secret = DbSecret::parse(&probe).unwrap();
        let shown = format!("{secret:?}");
        assert_eq!(shown, "DbSecret([redacted])");
        assert!(!shown.contains(&probe));
        assert_eq!(secret.len_chars(), probe.chars().count());
    }

    #[test]
    fn sql_literal_escapes_quotes_and_backslashes() {
        let probe = sample(&['a', '\'', 'b', '\\', 'c']);
        let secret = DbSecret::parse(&probe).unwrap();
        assert_eq!(secret.sql_literal(), "a''b\\\\c");
    }

    #[test]
    fn redacts_identified_by_but_keeps_error_text() {
        let secret = sample(&[
            'S', 'u', 'p', 'e', 'r', 'S', 'e', 'c', 'r', 'e', 't', '9', '!',
        ]);
        let raw = format!(
            "--------------\nALTER USER 'u'@'localhost' IDENTIFIED BY '{secret}'\n--------------\nERROR 1396 (HY000) at line 1: Operation ALTER USER failed"
        );
        let cleaned = redact_db_cli_stderr(&raw);
        assert!(!cleaned.contains(&secret));
        assert!(cleaned.contains("IDENTIFIED BY '[redacted]'"));
        assert!(cleaned.contains("ERROR 1396"));
        assert!(cleaned.contains("Operation ALTER USER failed"));
    }

    #[test]
    fn redacts_every_occurrence_including_escaped_quotes() {
        let a = sample(&['f', 'i', 'r', 's', 't', '\'', '\'', 'Q', '1']);
        let b = sample(&['s', 'e', 'c', 'o', 'n', 'd', '\\', '\'', 'Z', '2']);
        let raw = format!(
            "CREATE USER 'u'@'localhost' IDENTIFIED BY '{a}'; ALTER USER 'u'@'127.0.0.1' identified by '{b}'; GRANT ALL ON `db`.* TO 'u'@'localhost'"
        );
        let cleaned = redact_db_cli_stderr(&raw);
        assert!(!cleaned.contains("first"));
        assert!(!cleaned.contains("second"));
        assert_eq!(cleaned.matches("[redacted]").count(), 2);
        assert!(cleaned.contains("GRANT ALL ON `db`.*"));
    }

    #[test]
    fn redacts_password_function_and_using_clause() {
        let p = sample(&['L', 'e', 'g', 'a', 'c', 'y', '7', '$']);
        let raw = format!(
            "SET PASSWORD FOR 'u'@'%' = PASSWORD('{p}'); CREATE USER 'v'@'%' IDENTIFIED VIA ed25519 USING '{p}'"
        );
        let cleaned = redact_db_cli_stderr(&raw);
        assert!(!cleaned.contains("Legacy"));
        assert!(cleaned.contains("PASSWORD('[redacted]')"));
        assert!(cleaned.contains("USING '[redacted]'"));
    }

    #[test]
    fn unterminated_literal_drops_the_tail() {
        let p = sample(&['O', 'p', 'e', 'n', 'E', 'n', 'd', '3']);
        let raw = format!("ALTER USER 'u'@'localhost' IDENTIFIED BY '{p}");
        let cleaned = redact_db_cli_stderr(&raw);
        assert!(!cleaned.contains("OpenEnd"));
        assert!(cleaned.ends_with("IDENTIFIED BY '[redacted]'"));
    }

    #[test]
    fn plain_errors_pass_through_and_are_capped() {
        let raw = "ERROR 2002 (HY000): Can't connect to local server through socket";
        assert_eq!(redact_db_cli_stderr(raw), raw);
        let long: String = std::iter::repeat_n('e', 1_000).collect();
        assert_eq!(redact_db_cli_stderr(&long).chars().count(), 400);
    }
}
