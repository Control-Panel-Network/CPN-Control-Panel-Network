//! Apply `/var/lib/cpn/php-default.json` to host Remi/php-fpm and LiteSpeed lsphp.

use crate::litespeed_stack::openlitespeed_installed;
use crate::os_support::detect_guest_os;
use crate::php_defaults::{load_php_default, prepare_and_persist_php, today_ymd};
use std::path::Path;
use std::process::{Command, Stdio};

fn dnf_available() -> bool {
    Path::new("/usr/bin/dnf").exists() || Path::new("/usr/bin/yum").exists()
}

fn run_dnf_install(packages: &[&str]) -> Result<(), String> {
    if !dnf_available() {
        return Err("dnf is not available on this host".into());
    }
    let mut args = vec!["--setopt=lock_timeout=60", "install", "-y"];
    args.extend(packages.iter().copied());
    let out = Command::new("dnf")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run dnf: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!(
            "dnf install failed: {}",
            err.lines().next().unwrap_or("unknown error").trim()
        ))
    }
}

fn branch_to_lsphp_prefix(branch: &str) -> Option<&'static str> {
    match branch.trim() {
        "8.5" => Some("lsphp85"),
        "8.4" => Some("lsphp84"),
        "8.3" => Some("lsphp83"),
        "8.2" => Some("lsphp82"),
        _ => None,
    }
}

/// Persist and apply the host PHP default so CLI, php-fpm (phpMyAdmin), and lsphp align.
pub fn ensure_host_php_default(requested: Option<&str>) -> Result<String, String> {
    let guest = detect_guest_os()?;
    let req = requested
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .or_else(|| load_php_default().map(|r| r.requested))
        .filter(|v| !v.is_empty());
    let record = prepare_and_persist_php(&guest, req.as_deref(), &today_ymd())?;

    if dnf_available() && record.stream != "apt" {
        let _ = run_dnf_install(&[
            "php",
            "php-cli",
            "php-fpm",
            "php-mysqlnd",
            "php-mbstring",
            "php-xml",
            "php-json",
        ]);
        let _ = Command::new("systemctl")
            .args(["restart", "php-fpm"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    if openlitespeed_installed() {
        if let Some(prefix) = branch_to_lsphp_prefix(&record.branch) {
            let pkgs = [
                prefix.to_string(),
                format!("{prefix}-common"),
                format!("{prefix}-mysqlnd"),
                format!("{prefix}-mbstring"),
                format!("{prefix}-xml"),
                format!("{prefix}-gd"),
                format!("{prefix}-process"),
                format!("{prefix}-pdo"),
            ];
            let refs: Vec<&str> = pkgs.iter().map(String::as_str).collect();
            let _ = run_dnf_install(&refs);
        }
    }

    Ok(format!(
        "Host PHP default is {} ({})",
        record.branch, record.message
    ))
}

#[cfg(test)]
mod tests {
    use super::branch_to_lsphp_prefix;

    #[test]
    fn maps_branches() {
        assert_eq!(branch_to_lsphp_prefix("8.5"), Some("lsphp85"));
        assert!(branch_to_lsphp_prefix("9.0").is_none());
    }
}
