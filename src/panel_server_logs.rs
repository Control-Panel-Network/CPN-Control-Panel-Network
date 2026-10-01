//! Host log sources for Server > Logs (panel, web access and error, mail, FTP, ModSecurity).
//!
//! Only a fixed allowlist of paths and systemd units is ever read; the request never supplies a
//! path. Each source is read as a bounded tail, redacted, filtered by an optional search term and
//! paginated newest first.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, Stdio};

/// Trailing bytes read from a log file.
const TAIL_BYTES: u64 = 768_000;
/// Hard cap on lines kept in memory per request.
const MAX_LINES: usize = 6_000;
/// Longest line shown in the UI.
const MAX_LINE_CHARS: usize = 700;
/// Journal lines requested when a unit is the fallback source.
const JOURNAL_LINES: usize = 3_000;

/// Page sizes offered in the UI.
pub const PAGE_SIZES: [usize; 4] = [25, 50, 100, 200];
pub const DEFAULT_PER_PAGE: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLogKind {
    Panel,
    Access,
    Error,
    Email,
    Ftp,
    ModSec,
}

impl HostLogKind {
    pub const ALL: [HostLogKind; 6] = [
        HostLogKind::Panel,
        HostLogKind::Access,
        HostLogKind::Error,
        HostLogKind::Email,
        HostLogKind::Ftp,
        HostLogKind::ModSec,
    ];

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.slug() == slug)
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Access => "access",
            Self::Error => "error",
            Self::Email => "email",
            Self::Ftp => "ftp",
            Self::ModSec => "modsec",
        }
    }

    /// Viewer route for this log.
    pub fn href(self) -> &'static str {
        match self {
            Self::Panel => "/server/logs/panel",
            Self::Access => "/server/logs/access",
            Self::Error => "/server/logs/error",
            Self::Email => "/server/logs/email",
            Self::Ftp => "/server/logs/ftp",
            Self::ModSec => "/server/logs/modsec",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Panel => "Main Log",
            Self::Access => "Access Logs",
            Self::Error => "Error Logs",
            Self::Email => "Email Log",
            Self::Ftp => "FTP Logs",
            Self::ModSec => "ModSec Audit",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Panel => "CPN panel log",
            Self::Access => "Web server access",
            Self::Error => "Web server errors",
            Self::Email => "Postfix and Dovecot mail log",
            Self::Ftp => "FTP and SFTP transfer log",
            Self::ModSec => "WAF audit log",
        }
    }

    /// Allowlisted log files, first existing and readable wins.
    pub fn files(self) -> &'static [&'static str] {
        match self {
            Self::Panel => &["/var/log/cpn/panel.log", "/var/log/cpn-installer.log"],
            Self::Access => &[
                "/usr/local/lsws/logs/access.log",
                "/var/log/httpd/access_log",
                "/var/log/nginx/access.log",
                "/var/log/apache2/access.log",
            ],
            Self::Error => &[
                "/usr/local/lsws/logs/error.log",
                "/var/log/httpd/error_log",
                "/var/log/nginx/error.log",
                "/var/log/apache2/error.log",
            ],
            Self::Email => &[
                "/var/log/maillog",
                "/var/log/mail.log",
                "/var/log/postfix.log",
            ],
            Self::Ftp => &[
                "/var/log/pure-ftpd/transfer.log",
                "/var/log/pureftpd.log",
                "/var/log/vsftpd.log",
                "/var/log/xferlog",
                "/var/log/proftpd/proftpd.log",
            ],
            Self::ModSec => &[
                "/usr/local/lsws/logs/auditmodsec.log",
                "/usr/local/lsws/logs/modsec_audit.log",
                "/var/log/httpd/modsec_audit.log",
                "/var/log/nginx/modsec_audit.log",
                "/var/log/modsec_audit.log",
            ],
        }
    }

    /// systemd units whose journal is the fallback when no file exists.
    pub fn units(self) -> &'static [&'static str] {
        match self {
            Self::Panel => &["cpn-installer.service"],
            // Access lines never come from a service journal; per-site logs live under each home.
            Self::Access | Self::ModSec => &[],
            Self::Error => &["lshttpd.service", "lsws.service", "httpd.service"],
            Self::Email => &["postfix.service", "dovecot.service"],
            Self::Ftp => &["pure-ftpd.service", "vsftpd.service", "proftpd.service"],
        }
    }

    /// Sentence shown when nothing could be read.
    pub fn empty_hint(self) -> &'static str {
        match self {
            Self::Panel => {
                "No panel journal is readable yet. On the host run: journalctl -u cpn-installer.service -n 100"
            }
            Self::Access => {
                "No server-wide access log found. Per-site access logs are under Websites > Manage > Logs."
            }
            Self::Error => {
                "No server-wide error log found. Per-site error logs are under Websites > Manage > Logs."
            }
            Self::Email => {
                "No mail log found. Install the Email host stack (Postfix and Dovecot) to start logging mail."
            }
            Self::Ftp => {
                "No FTP log found. The FTP server has not logged yet, or no FTP server is installed."
            }
            Self::ModSec => {
                "No ModSecurity audit log found. It appears here once ModSecurity is enabled on the web server and has recorded a transaction."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogPage {
    pub kind: HostLogKind,
    /// Where the lines came from (a path, or `journalctl -u ...`); empty when nothing was found.
    pub source: String,
    pub search: String,
    pub page: usize,
    pub per_page: usize,
    pub total_lines: usize,
    pub total_pages: usize,
    /// Newest first.
    pub lines: Vec<String>,
}

/// Mask values after credential-like keys and cap the line length. Safe for UTF-8 input.
pub fn redact_line(line: &str) -> String {
    const KEYS: [&str; 9] = [
        "password=",
        "password:",
        "passwd=",
        "pass=",
        "pwd=",
        "secret=",
        "token=",
        "api_key=",
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

fn read_tail(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut buf).ok()?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if start > 0 {
        // Drop the first, probably partial, line.
        if let Some(pos) = text.find('\n') {
            text = text[pos + 1..].to_string();
        }
    }
    Some(text)
}

fn read_journal(units: &[&str]) -> Option<String> {
    if units.is_empty() {
        return None;
    }
    let n = JOURNAL_LINES.to_string();
    let mut args: Vec<&str> = vec!["5", "journalctl", "--no-pager", "-o", "short-iso", "-n", &n];
    for unit in units {
        args.push("-u");
        args.push(unit);
    }
    let out = Command::new("timeout")
        .args(&args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let useful = text
        .lines()
        .any(|l| !l.trim().is_empty() && !l.starts_with("-- "));
    useful.then_some(text)
}

/// Raw text plus a label for where it came from.
fn collect(kind: HostLogKind) -> Option<(String, String)> {
    for path in kind.files() {
        let p = Path::new(path);
        if p.is_file()
            && let Some(text) = read_tail(p)
            && !text.trim().is_empty()
        {
            return Some(((*path).to_string(), text));
        }
    }
    let text = read_journal(kind.units())?;
    Some((format!("journalctl -u {}", kind.units().join(" -u ")), text))
}

pub fn clamp_per_page(n: usize) -> usize {
    if PAGE_SIZES.contains(&n) {
        n
    } else {
        DEFAULT_PER_PAGE
    }
}

/// Filter, order newest first and slice one page out of `text`.
pub fn paginate_text(
    kind: HostLogKind,
    source: &str,
    text: &str,
    search: &str,
    page: usize,
    per_page: usize,
) -> LogPage {
    let per_page = clamp_per_page(per_page);
    let needle = search.trim().to_ascii_lowercase();
    let mut lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("-- "))
        .collect();
    if lines.len() > MAX_LINES {
        lines = lines[lines.len() - MAX_LINES..].to_vec();
    }
    let mut kept: Vec<String> = lines
        .into_iter()
        .rev()
        .map(redact_line)
        .filter(|l| needle.is_empty() || l.to_ascii_lowercase().contains(&needle))
        .collect();
    let total_lines = kept.len();
    let total_pages = total_lines.max(1).div_ceil(per_page);
    let page = page.clamp(1, total_pages);
    let start = (page - 1) * per_page;
    let lines = kept
        .drain(start..(start + per_page).min(total_lines))
        .collect();
    LogPage {
        kind,
        source: source.to_string(),
        search: search.trim().chars().take(120).collect(),
        page,
        per_page,
        total_lines,
        total_pages,
        lines,
    }
}

/// Read one allowlisted host log. An unreadable or missing log yields an empty page (never an
/// error), so the UI can show an honest empty state.
pub fn query_host_log(kind: HostLogKind, search: &str, page: usize, per_page: usize) -> LogPage {
    match collect(kind) {
        Some((source, text)) => paginate_text(kind, &source, &text, search, page, per_page),
        None => paginate_text(kind, "", "", search, 1, per_page),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_round_trip_and_reject_unknown() {
        for kind in HostLogKind::ALL {
            assert_eq!(HostLogKind::from_slug(kind.slug()), Some(kind));
            assert_eq!(kind.href(), format!("/server/logs/{}", kind.slug()));
        }
        assert_eq!(HostLogKind::from_slug("../etc/passwd"), None);
        assert_eq!(HostLogKind::from_slug(""), None);
    }

    #[test]
    fn allowlisted_paths_are_absolute_without_traversal() {
        for kind in HostLogKind::ALL {
            for path in kind.files() {
                assert!(path.starts_with("/var/log/") || path.starts_with("/usr/local/lsws/logs/"));
                assert!(!path.contains(".."));
            }
        }
    }

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
    fn paginate_is_newest_first_searchable_and_clamped() {
        let text = (1..=120)
            .map(|i| format!("line {i} {}", if i % 2 == 0 { "even" } else { "odd" }))
            .collect::<Vec<_>>()
            .join("\n");
        let p = paginate_text(HostLogKind::Error, "/x", &text, "", 1, 25);
        assert_eq!(p.total_lines, 120);
        assert_eq!(p.total_pages, 5);
        assert_eq!(p.lines.first().map(String::as_str), Some("line 120 even"));
        assert_eq!(p.lines.len(), 25);

        let last = paginate_text(HostLogKind::Error, "/x", &text, "", 99, 25);
        assert_eq!(last.page, 5);
        assert_eq!(last.lines.len(), 20);

        let even = paginate_text(HostLogKind::Error, "/x", &text, "EVEN", 1, 25);
        assert_eq!(even.total_lines, 60);

        let odd_size = paginate_text(HostLogKind::Error, "/x", &text, "", 1, 7);
        assert_eq!(odd_size.per_page, DEFAULT_PER_PAGE);
    }

    #[test]
    fn empty_input_gives_single_empty_page() {
        let p = paginate_text(HostLogKind::Ftp, "", "", "", 4, 50);
        assert_eq!((p.page, p.total_pages, p.total_lines), (1, 1, 0));
        assert!(p.lines.is_empty());
    }

    #[test]
    fn query_never_panics_on_any_host() {
        for kind in HostLogKind::ALL {
            let _ = query_host_log(kind, "", 1, 25);
        }
    }
}
