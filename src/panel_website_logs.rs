//! Safe, domain-scoped website log reads for Manage > Logs.
//!
//! Manage and the logs API only jail to `{site_home}/logs/{access|error}.log`.
//! Parent domains never read child subdomain trees (and the reverse).

use crate::sites::{SiteRecord, site_home_from_record};
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Default number of trailing lines loaded into memory for search/pagination.
pub const LOG_TAIL_LINES: usize = 400;
/// Hard cap when scanning for the modal (keeps payloads bounded).
pub const LOG_SCAN_LINES: usize = 5_000;
const LOG_SCAN_BYTES: u64 = 512_000;

/// Preferred access log path under the site home.
pub fn site_access_log_path(site: &SiteRecord) -> PathBuf {
    site_home_from_record(site).join("logs").join("access.log")
}

/// Preferred error log path under the site home.
pub fn site_error_log_path(site: &SiteRecord) -> PathBuf {
    site_home_from_record(site).join("logs").join("error.log")
}

/// Resolve the jailed log file for a kind (`access` or `error`).
pub fn site_log_path_for_kind(site: &SiteRecord, kind: &str) -> Result<PathBuf, String> {
    match kind {
        "access" => Ok(site_access_log_path(site)),
        "error" => Ok(site_error_log_path(site)),
        _ => Err("Log kind must be access or error".into()),
    }
}

/// Candidate access/error log paths (bandwidth helpers may use fallbacks).
/// Manage UI and the logs API use [`site_log_path_for_kind`] only.
pub fn candidate_log_paths(site: &SiteRecord) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let domain = &site.domain;
    let home_logs = site_home_from_record(site).join("logs");
    let access = vec![
        home_logs.join("access.log"),
        home_logs.join("access_log"),
        PathBuf::from(format!("/usr/local/lsws/logs/{domain}.access.log")),
        PathBuf::from(format!("/usr/local/lsws/logs/{domain}.access_log")),
        PathBuf::from(format!("/var/log/httpd/{domain}-access_log")),
        PathBuf::from(format!("/var/log/httpd/{domain}-access.log")),
        PathBuf::from(format!("/var/log/nginx/{domain}.access.log")),
        PathBuf::from(format!("/var/log/apache2/{domain}-access.log")),
    ];
    let error = vec![
        home_logs.join("error.log"),
        home_logs.join("error_log"),
        PathBuf::from(format!("/usr/local/lsws/logs/{domain}.error.log")),
        PathBuf::from(format!("/usr/local/lsws/logs/{domain}.error_log")),
        PathBuf::from(format!("/var/log/httpd/{domain}-error_log")),
        PathBuf::from(format!("/var/log/httpd/{domain}-error.log")),
        PathBuf::from(format!("/var/log/nginx/{domain}.error.log")),
        PathBuf::from(format!("/var/log/apache2/{domain}-error.log")),
    ];
    (access, error)
}

fn file_name_matches_domain(path: &Path, domain: &str) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    // Exact prefix: `{domain}.access.log` or `{domain}-access.log`, not substring of a child FQDN.
    name == format!("{domain}.access.log")
        || name == format!("{domain}.access_log")
        || name == format!("{domain}-access.log")
        || name == format!("{domain}-access_log")
        || name == format!("{domain}.error.log")
        || name == format!("{domain}.error_log")
        || name == format!("{domain}-error.log")
        || name == format!("{domain}-error_log")
}

/// True when `path` is inside this site's `logs/` jail (or an exact domain std log).
pub fn path_allowed(site: &SiteRecord, path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("..") {
        return false;
    }
    let home_logs = site_home_from_record(site).join("logs");
    if let Ok(canon_home) = home_logs.canonicalize() {
        if let Ok(canon_path) = path.canonicalize() {
            if canon_path.starts_with(&canon_home) {
                // Reject nested child trees that somehow share a prefix (should not happen).
                return true;
            }
        } else if path.parent() == Some(home_logs.as_path()) {
            // Preferred path before create / before canonicalize works.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            return matches!(
                name,
                "access.log" | "access_log" | "error.log" | "error_log"
            );
        }
    } else {
        let home_prefix = format!("{}/", home_logs.display());
        if (s.starts_with(&home_prefix) || path.parent() == Some(home_logs.as_path()))
            && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                matches!(n, "access.log" | "access_log" | "error.log" | "error_log")
            })
        {
            return true;
        }
    }

    let in_std = s.starts_with("/usr/local/lsws/logs/")
        || s.starts_with("/var/log/httpd/")
        || s.starts_with("/var/log/nginx/")
        || s.starts_with("/var/log/apache2/");
    in_std && file_name_matches_domain(path, &site.domain)
}

/// First existing allowlisted log path from candidates.
pub fn first_existing_log(site: &SiteRecord, candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|p| path_allowed(site, p) && p.is_file())
        .cloned()
}

/// Create `{site_home}/logs` plus empty access/error log files when missing.
pub fn ensure_site_log_files(site: &SiteRecord) -> Result<(), String> {
    let logs_dir = site_home_from_record(site).join("logs");
    fs::create_dir_all(&logs_dir)
        .map_err(|e| format!("Could not create {}: {e}", logs_dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&logs_dir, fs::Permissions::from_mode(0o755));
        chown_web_server(&logs_dir);
    }
    for name in ["access.log", "error.log"] {
        let path = logs_dir.join(name);
        if !path.is_file() {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| format!("Could not create {}: {e}", path.display()))?;
            let _ = file.write_all(b"");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
            chown_web_server(&path);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn chown_web_server(path: &Path) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return;
    };
    let nobody = CString::new("nobody").ok();
    let uid = nobody
        .as_ref()
        .and_then(|n| unsafe {
            let pw = libc::getpwnam(n.as_ptr());
            if pw.is_null() {
                None
            } else {
                Some((*pw).pw_uid)
            }
        })
        .unwrap_or(65534);
    let gid = nobody
        .as_ref()
        .and_then(|n| unsafe {
            let pw = libc::getpwnam(n.as_ptr());
            if pw.is_null() {
                None
            } else {
                Some((*pw).pw_gid)
            }
        })
        .unwrap_or(65534);
    unsafe {
        let _ = libc::chown(c_path.as_ptr(), uid, gid);
    }
}

/// Read the last `max_bytes` of a log file (path must already be allowlisted).
pub fn read_log_tail(site: &SiteRecord, path: &Path, max_bytes: u64) -> Result<String, String> {
    if !path_allowed(site, path) {
        return Err("Log path is not allowlisted for this domain".into());
    }
    if !path.is_file() {
        return Err(format!("Log file not found: {}", path.display()));
    }
    let mut file = File::open(path).map_err(|e| format!("Cannot open log: {e}"))?;
    let len = file
        .metadata()
        .map_err(|e| format!("Cannot stat log: {e}"))?
        .len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| format!("Cannot seek log: {e}"))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("Cannot read log: {e}"))?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Last `max_lines` lines from an allowlisted log (reads a trailing byte window).
pub fn read_log_tail_lines(
    site: &SiteRecord,
    path: &Path,
    max_lines: usize,
) -> Result<String, String> {
    let raw = read_log_tail(site, path, 96_000)?;
    let lines: Vec<&str> = raw.lines().collect();
    if lines.len() <= max_lines {
        return Ok(raw);
    }
    Ok(lines[lines.len() - max_lines..].join("\n"))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Clone, Serialize)]
pub struct LogPagePayload {
    pub ok: bool,
    pub domain: String,
    pub kind: String,
    pub path: String,
    pub search: String,
    pub page: usize,
    pub per_page: usize,
    pub total_lines: usize,
    pub total_pages: usize,
    pub lines: Vec<String>,
    pub empty: bool,
    pub message: String,
}

fn clamp_per_page(n: usize) -> usize {
    match n {
        0..=9 => 10,
        10..=50 => n,
        _ => 50,
    }
}

/// Paginated, searchable view of the jailed site log (no foreign paths).
pub fn query_site_log_page(
    site: &SiteRecord,
    kind: &str,
    search: &str,
    page: usize,
    per_page: usize,
) -> Result<LogPagePayload, String> {
    let _ = ensure_site_log_files(site);
    let path = site_log_path_for_kind(site, kind)?;
    if !path_allowed(site, &path) {
        return Err("Log path is not allowlisted for this domain".into());
    }
    let per_page = clamp_per_page(per_page);
    let page = page.max(1);
    let search_trim = search.trim();
    let search_lc = search_trim.to_ascii_lowercase();

    if !path.is_file() {
        return Ok(LogPagePayload {
            ok: true,
            domain: site.domain.clone(),
            kind: kind.into(),
            path: path.display().to_string(),
            search: search_trim.into(),
            page: 1,
            per_page,
            total_lines: 0,
            total_pages: 1,
            lines: Vec::new(),
            empty: true,
            message: format!("No {kind} log file yet. Expected path: {}", path.display()),
        });
    }

    let raw = read_log_tail(site, &path, LOG_SCAN_BYTES)?;
    let mut lines: Vec<String> = raw.lines().map(|l| l.to_string()).collect();
    if lines.len() > LOG_SCAN_LINES {
        lines = lines[lines.len() - LOG_SCAN_LINES..].to_vec();
    }
    if !search_lc.is_empty() {
        lines.retain(|l| l.to_ascii_lowercase().contains(&search_lc));
    }
    let total_lines = lines.len();
    let total_pages = total_lines.max(1).div_ceil(per_page).max(1);
    let page = page.min(total_pages);
    let start = (page - 1) * per_page;
    let page_lines: Vec<String> = lines.into_iter().skip(start).take(per_page).collect();
    let empty = total_lines == 0;
    let message = if empty && search_trim.is_empty() {
        format!(
            "Source: {} (file exists but is empty). Generate traffic and refresh.",
            path.display()
        )
    } else if empty {
        format!("No lines matched search in {}", path.display())
    } else {
        format!(
            "Source: {} (showing page {page} of {total_pages}, {total_lines} matching lines)",
            path.display()
        )
    };

    Ok(LogPagePayload {
        ok: true,
        domain: site.domain.clone(),
        kind: kind.into(),
        path: path.display().to_string(),
        search: search_trim.into(),
        page,
        per_page,
        total_lines,
        total_pages,
        lines: page_lines,
        empty,
        message,
    })
}

/// Compact hint panel under the log cards (no inline dump).
pub fn log_panel_html(site: &SiteRecord, kind: &str) -> String {
    let _ = ensure_site_log_files(site);
    let title = if kind == "error" {
        "Error Logs"
    } else {
        "Access Logs"
    };
    let preferred = site_log_path_for_kind(site, kind).unwrap_or_else(|_| PathBuf::from(""));
    format!(
        r#"<div class="manage-log-panel manage-log-hint" data-log-kind="{kind}">
  <h3>{title}</h3>
  <p class="manage-muted">Open the card above for a searchable, paginated viewer. Domain-scoped source: <code>{path}</code></p>
</div>"#,
        kind = html_escape(kind),
        path = html_escape(&preferred.display().to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SiteRecord;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_site(domain: &str, docroot: &str) -> SiteRecord {
        SiteRecord {
            schema_version: 1,
            domain: domain.into(),
            owner: "Admin".into(),
            docroot: docroot.into(),
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
        }
    }

    #[test]
    fn rejects_traversal_and_foreign_home() {
        let site = sample_site("example.com", "/home/example.com/public_html");
        let bad = PathBuf::from("/var/log/httpd/../../etc/passwd");
        assert!(!path_allowed(&site, &bad));
        assert!(!path_allowed(
            &site,
            Path::new("/home/other.com/logs/access.log")
        ));
        assert!(path_allowed(
            &site,
            Path::new("/home/example.com/logs/access.log")
        ));
        assert!(path_allowed(
            &site,
            Path::new("/var/log/nginx/example.com.access.log")
        ));
        // Child FQDN must not pass parent substring match on std logs.
        assert!(!path_allowed(
            &site,
            Path::new("/var/log/nginx/test.example.com.access.log")
        ));
    }

    #[test]
    fn parent_cannot_read_child_home_logs() {
        let parent = sample_site("newstargeted.com", "/home/newstargeted.com/public_html");
        let child = sample_site(
            "test2.newstargeted.com",
            "/home/newstargeted.com/test2.newstargeted.com/public_html",
        );
        let parent_log = Path::new("/home/newstargeted.com/logs/access.log");
        let child_log = Path::new("/home/newstargeted.com/test2.newstargeted.com/logs/access.log");
        assert!(path_allowed(&parent, parent_log));
        assert!(!path_allowed(&parent, child_log));
        assert!(path_allowed(&child, child_log));
        assert!(!path_allowed(&child, parent_log));
    }

    #[test]
    fn query_paginates_and_searches() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cpn-log-page-{stamp}"));
        let docroot = root.join("public_html");
        fs::create_dir_all(&docroot).unwrap();
        let site = sample_site("lab.example", &docroot.to_string_lossy());
        ensure_site_log_files(&site).unwrap();
        let access = site_access_log_path(&site);
        let mut body = String::new();
        for i in 1..=25 {
            body.push_str(&format!("line-{i} GET /path-{i}\n"));
        }
        fs::write(&access, body).unwrap();
        let page = query_site_log_page(&site, "access", "", 2, 10).unwrap();
        assert_eq!(page.total_lines, 25);
        assert_eq!(page.total_pages, 3);
        assert_eq!(page.page, 2);
        assert_eq!(page.lines.len(), 10);
        assert!(page.lines[0].contains("line-11"));
        let filtered = query_site_log_page(&site, "access", "path-3", 1, 10).unwrap();
        assert!(filtered.total_lines >= 1);
        assert!(filtered.lines.iter().all(|l| l.contains("path-3")));
        assert!(!page.path.contains("CyberPanel"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn creates_and_hints_without_inline_dump() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cpn-log-home-{stamp}"));
        let docroot = root.join("public_html");
        fs::create_dir_all(&docroot).unwrap();
        let site = sample_site("lab.example", &docroot.to_string_lossy());
        ensure_site_log_files(&site).unwrap();
        fs::write(site_access_log_path(&site), "secret-line\n").unwrap();
        let html = log_panel_html(&site, "access");
        assert!(html.contains("Access Logs"));
        assert!(html.contains("Open the card"));
        assert!(!html.contains("secret-line"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        let _ = fs::remove_dir_all(&root);
    }
}
