//! Host log viewers for Server > Logs (panel, web access and error, mail, FTP, ModSecurity).
//!
//! Only a fixed allowlist of paths and systemd units is ever read; the request never supplies a
//! path. Each source is read as a bounded tail, redacted, split into time and message (shown as
//! `dd/mm/yyyy HH:MM:SS`), filtered by an optional search term and paginated newest first.

pub use crate::panel_server_logs_kind::{DEFAULT_PER_PAGE, HostLogKind, PAGE_SIZES};
pub use crate::panel_server_logs_sources::{Collected, RawLine, Scope, collect, scope_for};
use crate::panel_server_logs_time::{current_year_month, split_stamp};

/// Hard cap on entries kept in memory per request.
const MAX_LINES: usize = 6_000;
/// Longest line shown in the UI.
const MAX_LINE_CHARS: usize = 700;

/// One log line split into its parts for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// `dd/mm/yyyy HH:MM:SS`, empty when the line carried no timestamp.
    pub stamp: String,
    /// Domain, `server`, `sftp`, ... (empty for single-source logs).
    pub scope: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct LogPage {
    pub kind: HostLogKind,
    /// Where the lines came from; empty when nothing was found at all.
    pub sources: Vec<String>,
    /// Optional explanation for an empty but existing source (for example ModSecurity not enabled).
    pub note: String,
    pub search: String,
    pub page: usize,
    pub per_page: usize,
    pub total_lines: usize,
    pub total_pages: usize,
    /// Newest first.
    pub entries: Vec<LogEntry>,
}

impl LogPage {
    pub fn found(&self) -> bool {
        !self.sources.is_empty()
    }
}

/// Mask values after credential-like keys and cap the line length. Safe for UTF-8 input.
pub fn redact_line(line: &str) -> String {
    const KEYS: [&str; 12] = [
        "password=",
        "password:",
        "passwd=",
        "passphrase=",
        "pass=",
        "pwd=",
        "secret=",
        "token=",
        "api_key=",
        "apikey=",
        "secret_key=",
        "authorization:",
    ];
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let lower = trimmed.to_ascii_lowercase();
    let mut out = String::with_capacity(trimmed.len().min(MAX_LINE_CHARS + 4));
    let mut count = 0usize;
    // 0 = copying, 1 = skipping whitespace before a value, 2 = inside a masked value.
    let mut mode = 0u8;
    let mut skip_to = 0usize;
    for (i, ch) in trimmed.char_indices() {
        if i < skip_to {
            continue;
        }
        if count >= MAX_LINE_CHARS {
            out.push_str("...");
            break;
        }
        match mode {
            1 => {
                if !ch.is_whitespace() {
                    mode = 2;
                } else {
                    out.push(ch);
                    count += 1;
                }
                continue;
            }
            2 => {
                if ch.is_whitespace() {
                    mode = 0;
                    out.push(ch);
                    count += 1;
                }
                continue;
            }
            _ => {}
        }
        if ch.is_ascii()
            && let Some(key) = KEYS.iter().find(|k| lower[i..].starts_with(**k))
        {
            // Only mask at a key boundary so words such as "bypass=" are left alone.
            let at_boundary = trimmed[..i]
                .chars()
                .next_back()
                .is_none_or(|p| !p.is_ascii_alphanumeric());
            if at_boundary {
                out.push_str(&trimmed[i..i + key.len()]);
                out.push_str("***");
                count += key.len() + 3;
                skip_to = i + key.len();
                mode = if key.ends_with(':') { 1 } else { 2 };
                continue;
            }
        }
        out.push(ch);
        count += 1;
    }
    out
}

pub fn clamp_per_page(n: usize) -> usize {
    if PAGE_SIZES.contains(&n) {
        n
    } else {
        DEFAULT_PER_PAGE
    }
}

/// Redact, split off the timestamp and order newest first. Several sources are merged by time;
/// a single source keeps its own order reversed.
fn ordered_entries(lines: &[RawLine]) -> Vec<LogEntry> {
    let now = current_year_month();
    let mut ranked: Vec<(i64, usize, LogEntry)> = Vec::with_capacity(lines.len());
    let mut last_scope: &str = "";
    let mut last_epoch = 0i64;
    for (idx, raw) in lines.iter().enumerate() {
        let redacted = redact_line(&raw.text);
        let (stamp, text, epoch) = match split_stamp(&redacted, now) {
            Some(s) => (s.stamp.display(), s.rest, s.stamp.epoch()),
            None => {
                let inherited = if raw.scope == last_scope { last_epoch } else { 0 };
                (String::new(), redacted, inherited)
            }
        };
        last_scope = &raw.scope;
        last_epoch = epoch;
        ranked.push((
            epoch,
            idx,
            LogEntry {
                stamp,
                scope: raw.scope.clone(),
                text,
            },
        ));
    }
    let multi = lines
        .iter()
        .skip(1)
        .any(|l| l.scope != lines.first().map(|f| f.scope.as_str()).unwrap_or(""));
    if multi {
        ranked.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    } else {
        ranked.reverse();
    }
    ranked.truncate(MAX_LINES);
    ranked.into_iter().map(|(_, _, e)| e).collect()
}

/// Filter and slice one page out of the collected lines.
pub fn paginate(
    kind: HostLogKind,
    collected: &Collected,
    search: &str,
    page: usize,
    per_page: usize,
) -> LogPage {
    let per_page = clamp_per_page(per_page);
    let needle = search.trim().to_ascii_lowercase();
    let mut kept: Vec<LogEntry> = ordered_entries(&collected.lines)
        .into_iter()
        .filter(|e| {
            needle.is_empty()
                || format!("{} {} {}", e.stamp, e.scope, e.text)
                    .to_ascii_lowercase()
                    .contains(&needle)
        })
        .collect();
    let total_lines = kept.len();
    let total_pages = total_lines.max(1).div_ceil(per_page);
    let page = page.clamp(1, total_pages);
    let start = (page - 1) * per_page;
    let entries = kept
        .drain(start..(start + per_page).min(total_lines))
        .collect();
    LogPage {
        kind,
        sources: collected.sources.clone(),
        note: collected.note.clone(),
        search: search.trim().chars().take(120).collect(),
        page,
        per_page,
        total_lines,
        total_pages,
        entries,
    }
}

/// Convenience for plain text from one source (used by tests and simple callers).
pub fn paginate_text(
    kind: HostLogKind,
    source: &str,
    text: &str,
    search: &str,
    page: usize,
    per_page: usize,
) -> LogPage {
    let mut collected = Collected::default();
    if !source.is_empty() {
        collected.sources.push(source.to_string());
    }
    collected.push_text("", text);
    paginate(kind, &collected, search, page, per_page)
}

/// Read the allowlisted sources visible to `scope`. Missing or unreadable logs yield an empty
/// page (never an error), so the UI can show an honest empty state.
pub fn query_host_log_scoped(
    kind: HostLogKind,
    scope: &Scope,
    search: &str,
    page: usize,
    per_page: usize,
) -> LogPage {
    match collect(kind, scope) {
        Some(collected) => paginate(kind, &collected, search, page, per_page),
        None => paginate(kind, &Collected::default(), search, 1, per_page),
    }
}

/// Host-wide read (panel admin).
pub fn query_host_log(kind: HostLogKind, search: &str, page: usize, per_page: usize) -> LogPage {
    query_host_log_scoped(kind, &Scope::Host, search, page, per_page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_masks_credentials_and_keeps_utf8() {
        let out = redact_line("login password=hunter2 user=blåbær token=abc done");
        assert!(out.contains("password=***"));
        assert!(out.contains("token=***"));
        assert!(!out.contains("hunter2"));
        assert!(!out.contains("abc"));
        assert!(out.contains("blåbær"));
        assert!(out.ends_with("done"));
    }

    #[test]
    fn redact_masks_query_string_secrets() {
        let out = redact_line(r#""GET /api?apikey=K123&access_token=T456 HTTP/1.1" 200"#);
        assert!(!out.contains("K123"));
        assert!(!out.contains("T456"));
    }

    #[test]
    fn redact_leaves_words_that_only_end_like_a_key() {
        assert_eq!(redact_line("bypass=1"), "bypass=1");
    }

    #[test]
    fn redact_truncates_long_lines() {
        let out = redact_line(&"x".repeat(3000));
        assert!(out.chars().count() <= MAX_LINE_CHARS + 3);
        assert!(out.ends_with("..."));
    }

    #[test]
    fn paginate_defaults_to_ten_newest_first_searchable_and_clamped() {
        let text = (1..=120)
            .map(|i| format!("line {i} {}", if i % 2 == 0 { "even" } else { "odd" }))
            .collect::<Vec<_>>()
            .join("\n");
        let p = paginate_text(HostLogKind::Error, "/x", &text, "", 1, DEFAULT_PER_PAGE);
        assert_eq!(p.total_lines, 120);
        assert_eq!(p.total_pages, 12);
        assert_eq!(p.entries.first().map(|e| e.text.as_str()), Some("line 120 even"));
        assert_eq!(p.entries.len(), 10);

        let last = paginate_text(HostLogKind::Error, "/x", &text, "", 99, 25);
        assert_eq!(last.page, 5);
        assert_eq!(last.entries.len(), 20);

        let even = paginate_text(HostLogKind::Error, "/x", &text, "EVEN", 1, 25);
        assert_eq!(even.total_lines, 60);

        let odd_size = paginate_text(HostLogKind::Error, "/x", &text, "", 1, 7);
        assert_eq!(odd_size.per_page, DEFAULT_PER_PAGE);
    }

    #[test]
    fn timestamps_are_split_and_searchable_as_displayed() {
        let text = "203.0.113.9 - - [01/Oct/2026:14:05:09 +0200] \"GET / HTTP/1.1\" 200 5";
        let p = paginate_text(HostLogKind::Access, "/x", text, "01/10/2026", 1, 10);
        assert_eq!(p.total_lines, 1);
        assert_eq!(p.entries[0].stamp, "01/10/2026 14:05:09");
        assert!(!p.entries[0].text.contains("Oct"));
    }

    #[test]
    fn several_sources_merge_by_time_newest_first() {
        let mut c = Collected::default();
        c.sources.push("a".into());
        c.push_text("a.example", "[01/Oct/2026:10:00:00 +0000] a1\n[01/Oct/2026:12:00:00 +0000] a2");
        c.push_text("b.example", "[01/Oct/2026:11:00:00 +0000] b1\n[01/Oct/2026:13:00:00 +0000] b2\ncontinuation");
        let p = paginate(HostLogKind::Access, &c, "", 1, 10);
        let order: Vec<&str> = p.entries.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(order, vec!["continuation", "b2", "a2", "b1", "a1"]);
        assert_eq!(p.entries[0].scope, "b.example");
    }

    #[test]
    fn empty_input_gives_single_empty_page() {
        let p = paginate_text(HostLogKind::Ftp, "", "", "", 4, 50);
        assert_eq!((p.page, p.total_pages, p.total_lines), (1, 1, 0));
        assert!(p.entries.is_empty());
        assert!(!p.found());
    }

    #[test]
    fn query_never_panics_on_any_host() {
        for kind in HostLogKind::ALL {
            let _ = query_host_log(kind, "", 1, 25);
        }
    }
}
