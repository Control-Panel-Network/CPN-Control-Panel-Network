//! Official rspamd.com DNF repo for EL hosts where AppStream/EPEL lack `rspamd`.
//!
//! Tachyon Auth UI SPF/DMARC needs rspamd milter Auth-Results (#714). On EL10 (and
//! often EL9) the package is not in default repos; the same upstream repo NT uses
//! publishes `centos-<major>` RPMs including EL10.

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

const REPO_PATH: &str = "/etc/yum.repos.d/rspamd.repo";

/// Parse major from `/etc/os-release` VERSION_ID (e.g. `10.3` -> 10).
pub fn el_major_from_os_release(contents: &str) -> Option<u32> {
    for line in contents.lines() {
        let t = line.trim();
        let Some(rest) = t
            .strip_prefix("VERSION_ID=")
            .or_else(|| t.strip_prefix("VERSION_ID ="))
        else {
            continue;
        };
        let raw = rest.trim().trim_matches('"').trim_matches('\'');
        let major = raw.split('.').next().unwrap_or("");
        if let Ok(n) = major.parse::<u32>()
            && (8..=10).contains(&n)
        {
            return Some(n);
        }
    }
    None
}

/// Repo file body for the official rspamd stable RPM channel.
pub fn rspamd_repo_file_body(el_major: u32) -> String {
    format!(
        "[rspamd]\n\
         name=Rspamd stable repository\n\
         baseurl=https://rspamd.com/rpm-stable/centos-{el_major}/$basearch\n\
         enabled=1\n\
         gpgcheck=1\n\
         repo_gpgcheck=0\n\
         gpgkey=https://rspamd.com/rpm/rspamd.asc\n"
    )
}

fn dnf_available() -> bool {
    Path::new("/usr/bin/dnf").is_file() || Path::new("/bin/dnf").is_file()
}

fn read_os_release() -> String {
    fs::read_to_string("/etc/os-release").unwrap_or_default()
}

/// Ensure `/etc/yum.repos.d/rspamd.repo` matches the host EL major.
/// No-op on non-DNF hosts. Idempotent; does not print secrets.
pub fn ensure_rspamd_dnf_repo() -> Result<String, String> {
    if !dnf_available() {
        return Ok("rspamd repo skipped (not dnf)".into());
    }
    let major = el_major_from_os_release(&read_os_release()).ok_or_else(|| {
        "cannot determine EL major from /etc/os-release for rspamd repo".to_string()
    })?;
    let want = rspamd_repo_file_body(major);
    let prev = fs::read_to_string(REPO_PATH).unwrap_or_default();
    if prev == want {
        return Ok(format!("rspamd.repo already set for EL{major}"));
    }
    if Path::new(REPO_PATH).exists() {
        let bak = format!(
            "{REPO_PATH}.bak.cpn.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );
        let _ = fs::copy(REPO_PATH, &bak);
    }
    fs::write(REPO_PATH, &want).map_err(|e| format!("write {REPO_PATH}: {e}"))?;
    let _ = Command::new("dnf")
        .args(["clean", "metadata", "-q"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(format!("wrote rspamd.repo for EL{major}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_el_majors() {
        assert_eq!(
            el_major_from_os_release("ID=almalinux\nVERSION_ID=\"10.3\"\n"),
            Some(10)
        );
        assert_eq!(
            el_major_from_os_release("ID=almalinux\nVERSION_ID=\"9.9\"\n"),
            Some(9)
        );
        assert_eq!(
            el_major_from_os_release("ID=rocky\nVERSION_ID=\"8.10\"\n"),
            Some(8)
        );
        assert_eq!(
            el_major_from_os_release("ID=ubuntu\nVERSION_ID=\"24.04\"\n"),
            None
        );
    }

    #[test]
    fn repo_body_uses_major() {
        let body = rspamd_repo_file_body(10);
        assert!(body.contains("centos-10/$basearch"));
        assert!(body.contains("gpgkey=https://rspamd.com/rpm/rspamd.asc"));
        assert!(body.contains("repo_gpgcheck=0"));
    }
}
