//! WP-CLI detection, bootstrap, and command runner for CPN WordPress tooling.

use crate::paths;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const WP_CLI_PHAR_URL: &str =
    "https://raw.githubusercontent.com/wp-cli/builds/gh-pages/phar/wp-cli.phar";

#[derive(Debug, Clone)]
pub struct WpCliStatus {
    pub available: bool,
    pub binary: Option<String>,
    pub version: Option<String>,
    pub detail: String,
}

fn which_wp() -> Option<String> {
    for candidate in ["wp", "/usr/local/bin/wp", "/usr/bin/wp"] {
        if let Ok(out) = Command::new(candidate).args(["--info", "--quiet"]).output()
            && out.status.success()
        {
            return Some(candidate.to_string());
        }
        if let Ok(out) = Command::new(candidate).arg("--version").output()
            && out.status.success()
        {
            return Some(candidate.to_string());
        }
    }
    let phar = bundled_phar_path();
    if phar.is_file() {
        return Some(format!("php:{}", phar.display()));
    }
    None
}

pub fn bundled_phar_path() -> PathBuf {
    paths::join_data("bin").join("wp-cli.phar")
}

fn php_bin() -> Option<String> {
    for candidate in [
        "php", "php82", "php83", "php84", "php8.2", "php8.3", "php8.4", "php85", "php8.5",
    ] {
        if let Ok(out) = Command::new(candidate).arg("-v").output()
            && out.status.success()
        {
            return Some(candidate.to_string());
        }
    }
    None
}

pub fn detect_wp_cli() -> WpCliStatus {
    match which_wp() {
        Some(bin) => {
            let version = run_wp_raw(&bin, &["--version"], None)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            WpCliStatus {
                available: true,
                binary: Some(bin),
                version,
                detail: "WP-CLI is available.".into(),
            }
        }
        None => WpCliStatus {
            available: false,
            binary: None,
            version: None,
            detail: "WP-CLI not found. Use Ensure WP-CLI, or place `wp` on PATH. Fallback downloads wp-cli.phar under the CPN data bin directory.".into(),
        },
    }
}

pub fn ensure_wp_cli() -> Result<WpCliStatus, String> {
    if let status @ WpCliStatus {
        available: true, ..
    } = detect_wp_cli()
    {
        return Ok(status);
    }
    let php = php_bin().ok_or_else(|| {
        "PHP CLI not found. Install PHP before using WP-CLI for WordPress installs.".to_string()
    })?;
    let bin_dir = paths::join_data("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Could not create {}: {e}", bin_dir.display()))?;
    let phar = bundled_phar_path();
    if !phar.is_file() {
        download_phar(&phar)?;
    }
    let out = Command::new(&php)
        .arg(&phar)
        .arg("--version")
        .output()
        .map_err(|e| format!("Failed to run WP-CLI phar: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "WP-CLI phar failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(WpCliStatus {
        available: true,
        binary: Some(format!("php:{}", phar.display())),
        version: Some(version),
        detail: format!("WP-CLI phar ready at {} (via {php}).", phar.display()),
    })
}

fn download_phar(dest: &Path) -> Result<(), String> {
    let tmp = dest.with_extension("phar.tmp");
    let tmp_s = tmp.to_str().unwrap_or("/tmp/wp-cli.phar.tmp");
    let curl_ok = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "20",
            "-o",
            tmp_s,
            WP_CLI_PHAR_URL,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !curl_ok {
        let wget = Command::new("wget")
            .args(["-q", "-O", tmp_s, WP_CLI_PHAR_URL])
            .status()
            .map_err(|e| format!("curl/wget missing for WP-CLI download: {e}"))?;
        if !wget.success() {
            return Err("Failed to download wp-cli.phar (curl and wget both failed)".into());
        }
    }
    fs::rename(&tmp, dest).map_err(|e| format!("Could not place wp-cli.phar: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dest, fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

fn run_wp_raw(bin_spec: &str, args: &[&str], path: Option<&Path>) -> Result<String, String> {
    let mut cmd = if let Some(phar) = bin_spec.strip_prefix("php:") {
        let php = php_bin().ok_or_else(|| "PHP CLI not found".to_string())?;
        let mut c = Command::new(php);
        c.arg(phar);
        c
    } else {
        Command::new(bin_spec)
    };
    if let Some(p) = path {
        cmd.arg(format!("--path={}", p.display()));
    }
    cmd.args(args);
    let out = cmd
        .output()
        .map_err(|e| format!("Failed to run WP-CLI: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        Err(format!("WP-CLI failed: {} {}", err.trim(), stdout.trim())
            .trim()
            .to_string())
    }
}

pub fn wp_run(path: &Path, args: &[&str]) -> Result<String, String> {
    let status = ensure_wp_cli()?;
    let bin = status
        .binary
        .ok_or_else(|| "WP-CLI binary missing after ensure".to_string())?;
    run_wp_raw(&bin, args, Some(path))
}

pub fn wp_option_get(path: &Path, key: &str) -> Result<String, String> {
    wp_run(path, &["option", "get", key]).map(|s| s.trim().to_string())
}

pub fn wp_option_update(path: &Path, key: &str, value: &str) -> Result<(), String> {
    wp_run(path, &["option", "update", key, value]).map(|_| ())
}

pub fn is_wordpress_docroot(path: &Path) -> bool {
    path.join("wp-config.php").is_file() || path.join("wp-includes").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic() {
        let status = detect_wp_cli();
        assert!(!status.detail.is_empty());
    }
}
