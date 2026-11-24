//! SQL text helpers for the MariaDB panel-state store.
//!
//! Values never travel as quoted strings: text and binary go out as hex
//! literals, and read-back queries select `HEX(...)`. That removes every
//! escaping edge case (quotes, backslashes, NUL, tabs, newlines) and keeps the
//! client's tab-separated batch output unambiguous.

use data_encoding::HEXLOWER_PERMISSIVE;

/// Binary hex literal (`X'...'`). Empty input yields `X''`.
pub fn hex_lit(bytes: &[u8]) -> String {
    format!("X'{}'", data_encoding::HEXUPPER.encode(bytes))
}

/// UTF-8 text literal as an expression that is safe for any content.
pub fn text_lit(value: &str) -> String {
    format!("CONVERT({} USING utf8mb4)", hex_lit(value.as_bytes()))
}

/// Decode client output produced by `HEX(...)`.
pub fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    HEXLOWER_PERMISSIVE
        .decode(value.trim().as_bytes())
        .map_err(|_| "Corrupt hex value returned by MariaDB".to_string())
}

/// Validate a database or account name used inside backticks or quotes.
pub fn safe_name(name: &str, max_len: usize) -> Result<&str, String> {
    if name.is_empty()
        || name.len() > max_len
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err("Invalid MariaDB name (use letters, digits, underscore)".into());
    }
    Ok(name)
}

/// Backtick-quoted identifier after validation.
pub fn ident(name: &str) -> Result<String, String> {
    Ok(format!("`{}`", safe_name(name, 64)?))
}

/// Validate a generated MariaDB password that is placed in `IDENTIFIED BY '...'`.
pub fn safe_password(value: &str) -> Result<&str, String> {
    if value.len() < 16 || value.len() > 128 || !value.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Panel database password must be 16-128 letters or digits".into());
    }
    Ok(value)
}

/// Split batch output (`--batch --skip-column-names`) into rows of columns.
pub fn parse_rows(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').map(str::to_string).collect())
        .collect()
}

/// Keep only the first error line, cut before any echoed SQL fragment.
pub fn sanitize_sql_error(stderr: &str) -> String {
    let line = stderr
        .lines()
        .find(|l| l.contains("ERROR"))
        .or_else(|| stderr.lines().next())
        .unwrap_or("MariaDB client failed");
    let cut = line.find(" near ").map(|i| &line[..i]).unwrap_or(line);
    cut.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_literals_round_trip_hostile_text() {
        let nasty = "a'b\\c\"d\n\te\0f \u{00e6}\u{00f8}";
        let lit = hex_lit(nasty.as_bytes());
        assert!(lit.starts_with("X'") && lit.ends_with('\''));
        let inner = &lit[2..lit.len() - 1];
        assert_eq!(decode_hex(inner).unwrap(), nasty.as_bytes());
        assert!(!text_lit(nasty).contains('\\'));
    }

    #[test]
    fn names_reject_injection() {
        assert!(safe_name("cpn_panel", 64).is_ok());
        assert!(safe_name("cpn`; DROP", 64).is_err());
        assert!(safe_name("", 64).is_err());
        assert!(safe_password("short").is_err());
        assert!(safe_password("abcdefghijklmnop'q").is_err());
        assert!(safe_password("Abcdefghijklmnop1234").is_ok());
    }

    #[test]
    fn rows_parse_tabs_and_blank_lines() {
        let rows = parse_rows("a\tb\n\nc\td\n");
        assert_eq!(rows, vec![vec!["a", "b"], vec!["c", "d"]]);
    }

    #[test]
    fn error_sanitizer_drops_sql_fragment() {
        let err = "ERROR 1064 (42000) at line 1: You have an error near 'X'414243' at line 1";
        let clean = sanitize_sql_error(err);
        assert!(!clean.contains("414243"));
        assert!(clean.starts_with("ERROR 1064"));
    }
}
