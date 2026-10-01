//! Where each Server > Logs viewer reads from.
//!
//! Only fixed allowlisted files, systemd units and the per-site `logs/` jail are ever read. Web
//! access and error logs merge the server-wide file with every site log the viewer may see. FTP
//! combines an FTP daemon log with the jailed SFTP lines from the SSH auth log. ModSecurity audit
//! transactions are condensed to one line each.

use crate::panel_admin::is_panel_admin;
use crate::panel_ops_sftp::SFTP_GROUP;
use crate::panel_server_logs_kind::HostLogKind;
use crate::panel_server_logs_modsec::collect_modsec;
use crate::panel_website_logs::{path_allowed, site_log_path_for_kind};
use crate::site_acl::{SitePerm, can_manage_site};
use crate::sites::{SiteRecord, list_sites};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, Stdio};

/// Trailing bytes read from a host log file.
pub const TAIL_BYTES: u64 = 768_000;
/// Trailing bytes read from one site log when many sites are merged.
const SITE_TAIL_BYTES: u64 = 128_000;
const SITE_TAIL_BYTES_MANY: u64 = 48_000;
/// Most sites merged into one view.
pub const MAX_SITES: usize = 300;
/// Journal lines requested when a unit is the fallback source.
const JOURNAL_LINES: usize = 3_000;

/// One raw log line and the scope it came from (a domain, `server`, `sftp`, ...).
#[derive(Debug, Clone)]
pub struct RawLine {
    pub scope: String,
    pub text: String,
}

/// Everything read for one viewer. `sources` is empty only when nothing exists at all.
#[derive(Debug, Clone, Default)]
pub struct Collected {
    pub sources: Vec<String>,
    /// Shown instead of the generic quiet-state sentence when `lines` is empty.
    pub note: String,
    /// Oldest first within each source.
    pub lines: Vec<RawLine>,
}

impl Collected {
    pub fn push_text(&mut self, scope: &str, text: &str) {
        for line in text.lines() {
            if line.trim().is_empty() || line.starts_with("-- ") {
                continue;
            }
            self.lines.push(RawLine {
                scope: scope.to_string(),
                text: line.to_string(),
            });
        }
    }
}

/// What the viewer may see: the whole host (panel admin) or only the listed sites.
#[derive(Debug, Clone)]
pub enum Scope {
    Host,
    Sites(Vec<SiteRecord>),
}

/// Admin sees the host; everyone else only the sites they own or manage.
pub fn scope_for(username: &str) -> Scope {
    if is_panel_admin(username) {
        return Scope::Host;
    }
    let sites = list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| can_manage_site(username, &s.domain, SitePerm::Enable).unwrap_or(false))
        .collect();
    Scope::Sites(sites)
}

pub fn read_tail(path: &Path, max_bytes: u64) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    file.take(max_bytes).read_to_end(&mut buf).ok()?;
    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if start > 0 {
        // Drop the first, probably partial, line.
        if let Some(pos) = text.find('\n') {
            text = text[pos + 1..].to_string();
        }
    }
    Some(text)
}

pub fn read_journal(units: &[&str]) -> Option<String> {
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

/// A site log is only read when it is a plain file (never a symlink) in a real `logs/` folder,
/// so a site owner cannot point their log at a file outside their home.
fn safe_site_file(path: &Path) -> bool {
    let plain = fs::symlink_metadata(path)
        .map(|m| m.file_type().is_file())
        .unwrap_or(false);
    let parent_ok = path
        .parent()
        .and_then(|p| fs::symlink_metadata(p).ok())
        .map(|m| m.file_type().is_dir())
        .unwrap_or(false);
    plain && parent_ok
}

fn first_host_file(kind: HostLogKind) -> Option<(&'static str, String)> {
    for path in kind.files() {
        let p = Path::new(path);
        if p.is_file()
            && let Some(text) = read_tail(p, TAIL_BYTES)
        {
            return Some((*path, text));
        }
    }
    None
}

fn collect_simple(kind: HostLogKind) -> Option<Collected> {
    let mut out = Collected::default();
    if let Some((path, text)) = first_host_file(kind).filter(|(_, t)| !t.trim().is_empty()) {
        out.sources.push(path.to_string());
        out.push_text("", &text);
        return Some(out);
    }
    let text = read_journal(kind.units())?;
    out.sources
        .push(format!("journalctl -u {}", kind.units().join(" -u ")));
    out.push_text("", &text);
    Some(out)
}

/// Web access or error log: the server-wide file (admin) plus each visible site log.
fn collect_web(kind: HostLogKind, scope: &Scope) -> Option<Collected> {
    let kind_name = if kind == HostLogKind::Access {
        "access"
    } else {
        "error"
    };
    let mut out = Collected::default();
    let sites: Vec<SiteRecord> = match scope {
        Scope::Host => {
            if let Some((path, text)) = first_host_file(kind) {
                out.sources.push(format!("{path} (server-wide)"));
                out.push_text("server", &text);
            }
            list_sites().unwrap_or_default()
        }
        Scope::Sites(list) => list.clone(),
    };
    let per_site = if sites.len() > 20 {
        SITE_TAIL_BYTES_MANY
    } else {
        SITE_TAIL_BYTES
    };
    for site in sites.iter().take(MAX_SITES) {
        let Ok(path) = site_log_path_for_kind(site, kind_name) else {
            continue;
        };
        if !path_allowed(site, &path) || !safe_site_file(&path) {
            continue;
        }
        let Some(text) = read_tail(&path, per_site) else {
            continue;
        };
        out.sources
            .push(format!("{} ({})", site.domain, path.display()));
        out.push_text(&site.domain, &text);
    }
    if out.sources.is_empty() {
        if kind == HostLogKind::Error && matches!(scope, Scope::Host) {
            return collect_simple(kind).map(|mut c| {
                for l in &mut c.lines {
                    l.scope = "server".into();
                }
                c
            });
        }
        return None;
    }
    Some(out)
}

/// Members of the jailed SFTP group: names listed in `/etc/group` plus accounts whose primary
/// group it is (those are only recorded in `/etc/passwd`).
pub fn group_members(group_file: &str, passwd_file: &str, group: &str) -> Vec<String> {
    let mut gid = String::new();
    let mut members: Vec<String> = Vec::new();
    for l in group_file.lines() {
        let mut parts = l.splitn(4, ':');
        if parts.next() != Some(group) {
            continue;
        }
        gid = parts.nth(1).unwrap_or("").trim().to_string();
        members = parts
            .next()
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        break;
    }
    if !gid.is_empty() {
        for l in passwd_file.lines() {
            let fields: Vec<&str> = l.split(':').collect();
            if fields.len() > 3 && fields[3] == gid && !members.iter().any(|m| m == fields[0]) {
                members.push(fields[0].to_string());
            }
        }
    }
    members
}
/// Messages `internal-sftp -l INFO` writes through sshd for file operations and sessions.
const SFTP_OPERATIONS: [&str; 14] = [
    "open \"",
    "opendir \"",
    "close \"",
    "remove \"",
    "rename \"",
    "mkdir \"",
    "rmdir \"",
    "setstat \"",
    "symlink ",
    "readlink \"",
    "realpath \"",
    "session opened for local user",
    "session closed for local user",
    "received client version",
];

/// True for SSH auth log lines that belong to SFTP: sftp-server records, logged file operations
/// and anything sshd says about a jailed SFTP account.
pub fn sftp_line_matches(line: &str, members: &[String]) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("internal-sftp") || lower.contains("sftp-server") {
        return true;
    }
    if !lower.contains("sshd") {
        return false;
    }
    if lower.contains("sftp") {
        return true;
    }
    if let Some((_, message)) = line.split_once("]: ")
        && SFTP_OPERATIONS
            .iter()
            .any(|op| message.trim_start().starts_with(op))
    {
        return true;
    }
    line.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .any(|tok| members.iter().any(|m| m == tok))
}
/// FTP daemon log (if any) plus jailed SFTP lines from the SSH auth log.
fn collect_ftp() -> Option<Collected> {
    let mut out = Collected::default();
    if let Some((path, text)) = first_host_file(HostLogKind::Ftp) {
        out.sources.push(format!("{path} (FTP server)"));
        out.push_text("ftp", &text);
    } else if let Some(text) = read_journal(HostLogKind::Ftp.units()) {
        out.sources.push(format!(
            "journalctl -u {} (FTP server)",
            HostLogKind::Ftp.units().join(" -u ")
        ));
        out.push_text("ftp", &text);
    }
    let members = fs::read_to_string("/etc/group")
        .map(|g| {
            let passwd = fs::read_to_string("/etc/passwd").unwrap_or_default();
            group_members(&g, &passwd, SFTP_GROUP)
        })
        .unwrap_or_default();
    if !members.is_empty() {
        let mut found: Option<(String, String)> = None;
        for path in ["/var/log/secure", "/var/log/auth.log"] {
            let p = Path::new(path);
            if p.is_file()
                && let Some(text) = read_tail(p, TAIL_BYTES)
            {
                found = Some((path.to_string(), text));
                break;
            }
        }
        if found.is_none() {
            found = read_journal(&["sshd.service", "ssh.service"])
                .map(|t| ("journalctl -u sshd.service -u ssh.service".to_string(), t));
        }
        if let Some((src, text)) = found {
            out.sources.push(format!(
                "{src} (SFTP lines for {} jailed account{})",
                members.len(),
                if members.len() == 1 { "" } else { "s" }
            ));
            let kept: String = text
                .lines()
                .filter(|l| sftp_line_matches(l, &members))
                .collect::<Vec<_>>()
                .join("\n");
            out.push_text("sftp", &kept);
        }
    }
    (!out.sources.is_empty()).then_some(out)
}

/// Read the sources for one viewer. `None` means nothing exists (honest empty state).
pub fn collect(kind: HostLogKind, scope: &Scope) -> Option<Collected> {
    if matches!(scope, Scope::Sites(_)) && !kind.per_site() {
        return None;
    }
    match kind {
        HostLogKind::Access | HostLogKind::Error => collect_web(kind, scope),
        HostLogKind::Ftp => collect_ftp(),
        HostLogKind::ModSec => collect_modsec(),
        HostLogKind::Panel | HostLogKind::Email => collect_simple(kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_members_parse() {
        let g = "root:x:0:\ncpn-sftp:x:990:alice,bob,\nother:x:5:carol\n";
        let p = "root:x:0:0::/root:/bin/bash\ndave:x:1001:990::/home/d:/usr/sbin/nologin\nalice:x:1002:990::/home/a:/usr/sbin/nologin\n";
        assert_eq!(group_members(g, "", "cpn-sftp"), vec!["alice", "bob"]);
        // Primary-group accounts are found through /etc/passwd, without duplicating listed ones.
        assert_eq!(
            group_members(g, p, "cpn-sftp"),
            vec!["alice", "bob", "dave"]
        );
        assert!(group_members(g, p, "missing").is_empty());
        assert!(group_members("cpn-sftp:x:990:\n", "", "cpn-sftp").is_empty());
    }

    #[test]
    fn sftp_lines_match_by_keyword_or_jailed_user() {
        let members = vec!["alice".to_string()];
        assert!(sftp_line_matches(
            "sshd[1]: Accepted password for alice from 1.2.3.4",
            &members
        ));
        assert!(sftp_line_matches("internal-sftp[2]: open \"/x\"", &members));
        // File operations arrive as plain sshd records without the account name.
        assert!(sftp_line_matches(
            "Oct  1 15:59:16 h sshd-session[138561]: open \"/public_html/a.txt\" flags WRITE mode 0666 [postauth]",
            &members
        ));
        assert!(sftp_line_matches(
            "Oct  1 15:59:16 h sshd-session[1]: opendir \"/\" [postauth]",
            &members
        ));
        // A sudo command that merely mentions sftp is not an SFTP record.
        assert!(!sftp_line_matches(
            "sudo[1]: cpn : COMMAND=/bin/grep sftp /var/log/secure",
            &members
        ));
        assert!(!sftp_line_matches(
            "sshd[1]: Accepted password for root from 1.2.3.4",
            &members
        ));
        assert!(!sftp_line_matches("sudo: alice : TTY=pts/0", &members));
        assert!(!sftp_line_matches(
            "sshd[1]: Failed for malice from 1.2.3.4",
            &members
        ));
    }

    #[test]
    fn non_admin_scope_never_reads_host_wide_logs() {
        let scope = Scope::Sites(Vec::new());
        for kind in [
            HostLogKind::Panel,
            HostLogKind::Email,
            HostLogKind::Ftp,
            HostLogKind::ModSec,
        ] {
            assert!(collect(kind, &scope).is_none());
        }
        assert!(collect(HostLogKind::Access, &scope).is_none());
    }

    #[test]
    fn site_logs_must_be_plain_files() {
        let dir = std::env::temp_dir().join(format!("cpn-logs-safe-{}", std::process::id()));
        let logs = dir.join("logs");
        fs::create_dir_all(&logs).unwrap();
        let real = logs.join("access.log");
        fs::write(&real, "x\n").unwrap();
        assert!(safe_site_file(&real));
        assert!(!safe_site_file(&logs.join("missing.log")));
        let _ = fs::remove_dir_all(&dir);
    }

    fn temp_site(name: &str) -> (SiteRecord, std::path::PathBuf) {
        let home = std::env::temp_dir().join(format!("cpn-logs-{name}-{}", std::process::id()));
        fs::create_dir_all(home.join("public_html")).unwrap();
        fs::create_dir_all(home.join("logs")).unwrap();
        let site = SiteRecord {
            schema_version: 1,
            domain: "example.com".into(),
            owner: "alice".into(),
            docroot: home.join("public_html").to_string_lossy().into_owned(),
            enabled: true,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
            ssl: Default::default(),
            internal_ip: None,
            owner_suspend_message: String::new(),
            suspended_by: None,
            php_version: None,
            aliases: Vec::new(),
            staging_of: None,
        };
        (site, home)
    }

    #[test]
    fn site_scope_reads_only_the_jailed_site_log_with_domain_badge() {
        let (site, home) = temp_site("scope");
        fs::write(
            home.join("logs/access.log"),
            "1.1.1.1 - - [01/Oct/2026:10:00:00 +0200] \"GET / HTTP/1.1\" 200 1\n",
        )
        .unwrap();
        let got = collect(HostLogKind::Access, &Scope::Sites(vec![site])).unwrap();
        assert_eq!(got.lines.len(), 1);
        assert_eq!(got.lines[0].scope, "example.com");
        assert_eq!(got.sources.len(), 1);
        let _ = fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_site_log_is_never_followed() {
        let (site, home) = temp_site("symlink");
        let secret = home.join("secret.txt");
        fs::write(&secret, "top secret\n").unwrap();
        std::os::unix::fs::symlink(&secret, home.join("logs/access.log")).unwrap();
        let got = collect(HostLogKind::Access, &Scope::Sites(vec![site]));
        assert!(got.is_none());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn collect_never_panics_on_any_host() {
        for kind in HostLogKind::ALL {
            let _ = collect(kind, &Scope::Host);
        }
    }
}
