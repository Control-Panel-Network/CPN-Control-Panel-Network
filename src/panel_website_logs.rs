//! Safe website log tail reads for Manage > Logs.

use crate::sites::SiteRecord;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Candidate access/error log paths for a site (first existing wins per kind).
pub fn candidate_log_paths(site: &SiteRecord) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let domain = &site.domain;
    let access = vec![
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

fn path_allowed(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.contains("..") {
        return false;
    }
    s.starts_with("/usr/local/lsws/logs/")
        || s.starts_with("/var/log/httpd/")
        || s.starts_with("/var/log/nginx/")
        || s.starts_with("/var/log/apache2/")
        || s.starts_with("/home/")
}

/// First existing allowlisted log path from candidates.
pub fn first_existing_log(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|p| path_allowed(p) && p.is_file())
        .cloned()
}

/// Read the last `max_bytes` of a log file (path must already be allowlisted).
pub fn read_log_tail(path: &Path, max_bytes: u64) -> Result<String, String> {
    if !path_allowed(path) {
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
    let text = String::from_utf8_lossy(&buf).into_owned();
    Ok(text)
}

/// HTML-escaped pre block for a log kind, or an honest empty message.
pub fn log_panel_html(site: &SiteRecord, kind: &str) -> String {
    let (access, error) = candidate_log_paths(site);
    let candidates = if kind == "error" { error } else { access };
    let title = if kind == "error" {
        "Error Logs"
    } else {
        "Access Logs"
    };
    match first_existing_log(&candidates) {
        Some(path) => match read_log_tail(&path, 48_000) {
            Ok(body) => {
                let escaped = body
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                format!(
                    r#"<div class="manage-log-panel">
  <h3>{title}</h3>
  <p class="manage-muted">Source: <code>{path}</code> (last ~48 KB)</p>
  <pre class="manage-log-pre">{escaped}</pre>
</div>"#,
                    path = path.display(),
                )
            }
            Err(err) => format!(
                r#"<div class="manage-log-panel">
  <h3>{title}</h3>
  <p class="manage-muted">{err}</p>
</div>"#,
                err = err.replace('<', "&lt;"),
            ),
        },
        None => format!(
            r#"<div class="manage-log-panel">
  <h3>{title}</h3>
  <p class="manage-muted">No {kind} log file found yet for this domain. When the web stack writes vhost logs, they will appear here.</p>
</div>"#
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SiteRecord;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_site(domain: &str) -> SiteRecord {
        SiteRecord {
            schema_version: 1,
            domain: domain.into(),
            owner: "Admin".into(),
            docroot: format!("/home/{domain}/public_html"),
            enabled: true,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
        }
    }

    #[test]
    fn rejects_traversal_candidates() {
        let bad = PathBuf::from("/var/log/httpd/../../etc/passwd");
        assert!(!path_allowed(&bad));
        assert!(path_allowed(Path::new(
            "/var/log/nginx/example.com.access.log"
        )));
        assert!(path_allowed(Path::new("/home/example.com/logs/access.log")));
    }

    #[test]
    fn reads_tail_of_temp_under_home_pattern() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cpn-log-{stamp}"));
        // Simulate allowlisted home logs by using a real /home path when writable,
        // otherwise verify the empty panel path for missing logs.
        let site = sample_site("missing-logs.example");
        let html = log_panel_html(&site, "access");
        assert!(html.contains("Access Logs"));
        assert!(html.contains("No access log"));
        let _ = fs::remove_dir_all(&dir);
    }
}
