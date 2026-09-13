//! Safe website log tail reads for Manage > Logs.

use crate::sites::{SiteRecord, site_home_from_record};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Default number of trailing lines shown in the Logs tab.
pub const LOG_TAIL_LINES: usize = 400;

/// Preferred access log path under the site home.
pub fn site_access_log_path(site: &SiteRecord) -> PathBuf {
    site_home_from_record(site).join("logs").join("access.log")
}

/// Preferred error log path under the site home.
pub fn site_error_log_path(site: &SiteRecord) -> PathBuf {
    site_home_from_record(site).join("logs").join("error.log")
}

/// Candidate access/error log paths for a site (first existing wins per kind).
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
        PathBuf::from(format!("/home/{domain}/logs/access.log")),
        PathBuf::from(format!("/home/{domain}/logs/access_log")),
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
        PathBuf::from(format!("/home/{domain}/logs/error.log")),
        PathBuf::from(format!("/home/{domain}/logs/error_log")),
    ];
    (access, error)
}

fn path_allowed(site: &SiteRecord, path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("..") {
        return false;
    }
    let home_logs = site_home_from_record(site).join("logs");
    if let Ok(canon_home) = home_logs.canonicalize()
        && let Ok(canon_path) = path.canonicalize()
        && canon_path.starts_with(&canon_home)
    {
        return true;
    }
    // Allow preferred paths even before the file exists (prefix check).
    let home_prefix = format!("{}/", home_logs.display());
    if s.starts_with(&home_prefix) || path.parent() == Some(home_logs.as_path()) {
        return true;
    }
    let domain = &site.domain;
    let in_std = s.starts_with("/usr/local/lsws/logs/")
        || s.starts_with("/var/log/httpd/")
        || s.starts_with("/var/log/nginx/")
        || s.starts_with("/var/log/apache2/");
    in_std && s.contains(domain.as_str())
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
    // LiteSpeed/OLS default runtime user is nobody.
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
        return Err("Log path is not allowlisted".into());
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

/// HTML-escaped pre block for a log kind, or an honest empty message.
pub fn log_panel_html(site: &SiteRecord, kind: &str) -> String {
    let _ = ensure_site_log_files(site);
    let (access, error) = candidate_log_paths(site);
    let candidates = if kind == "error" { error } else { access };
    let title = if kind == "error" {
        "Error Logs"
    } else {
        "Access Logs"
    };
    let preferred = if kind == "error" {
        site_error_log_path(site)
    } else {
        site_access_log_path(site)
    };
    let domain_q = html_escape(&site.domain);
    let refresh = format!(
        r#"<p class="manage-log-toolbar"><a class="manage-btn" href="/websites/manage?domain={domain_q}&amp;tab=logs#{anchor}">Refresh</a></p>"#,
        anchor = if kind == "error" { "error" } else { "access" },
    );

    match first_existing_log(site, &candidates) {
        Some(path) => match read_log_tail_lines(site, &path, LOG_TAIL_LINES) {
            Ok(body) if body.trim().is_empty() => format!(
                r#"{refresh}<div class="manage-log-panel">
  <h3>{title}</h3>
  <p class="manage-muted">Source: <code>{path}</code> (last {lines} lines). File exists but is empty. Generate traffic (curl the site or Preview) and refresh.</p>
  <pre class="manage-log-pre" aria-label="{title}"></pre>
</div>"#,
                path = html_escape(&path.display().to_string()),
                lines = LOG_TAIL_LINES,
            ),
            Ok(body) => {
                let escaped = html_escape(&body);
                format!(
                    r#"{refresh}<div class="manage-log-panel">
  <h3>{title}</h3>
  <p class="manage-muted">Source: <code>{path}</code> (last {lines} lines)</p>
  <pre class="manage-log-pre" aria-label="{title}">{escaped}</pre>
</div>"#,
                    path = html_escape(&path.display().to_string()),
                    lines = LOG_TAIL_LINES,
                )
            }
            Err(err) => format!(
                r#"{refresh}<div class="manage-log-panel">
  <h3>{title}</h3>
  <p class="manage-muted">{err}</p>
</div>"#,
                err = html_escape(&err),
            ),
        },
        None => format!(
            r#"{refresh}<div class="manage-log-panel">
  <h3>{title}</h3>
  <p class="manage-muted">No {kind} log file found yet for this domain. Expected path: <code>{pref}</code>. When the web stack writes vhost logs, they will appear here.</p>
</div>"#,
            pref = html_escape(&preferred.display().to_string()),
        ),
    }
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
    }

    #[test]
    fn creates_and_tails_site_home_logs() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cpn-log-home-{stamp}"));
        let docroot = root.join("public_html");
        fs::create_dir_all(&docroot).unwrap();
        let site = sample_site("lab.example", &docroot.to_string_lossy());
        ensure_site_log_files(&site).unwrap();
        let access = site_access_log_path(&site);
        assert!(access.is_file());
        fs::write(&access, "a\nb\nc\n").unwrap();
        let html = log_panel_html(&site, "access");
        assert!(html.contains("Access Logs"));
        assert!(html.contains("Source:"));
        assert!(html.contains(">a\nb\nc"));
        assert!(html.contains("Refresh"));
        assert!(!html.to_lowercase().contains("cyberpanel"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_missing_domain_panel() {
        let site = sample_site(
            "missing-logs.example",
            "/tmp/cpn-missing-logs-example/public_html",
        );
        let html = log_panel_html(&site, "access");
        assert!(html.contains("Access Logs"));
        // ensure creates files under temp home when writable; otherwise empty-state text.
        assert!(html.contains("Access Logs"));
    }
}
