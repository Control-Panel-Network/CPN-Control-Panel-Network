//! Apache / httpd PHP default (separate from LiteSpeed lsphp host default).
//!
//! Persists at `/var/lib/cpn/php-apache-default.json`. On LiteSpeed hosts that already
//! use `lsphp*`, CPN prefers Remi parallel packages (`php86-php`, …) under `/opt/remi`
//! so Apache PHP can change without fighting `/var/lib/php/opcache` with lsphp.
//! When Remi modular `php` is safe, Apache PHP uses `dnf module switch-to`.

use crate::apps_pkg::{package_installed, rpm_or_dpkg_installed};
use crate::os_support::detect_guest_os;
use crate::panel_ops_php_ext::MANAGED_BRANCHES;
use crate::paths::default_data_dir;
use crate::php_defaults::{
    CPN_PREFERRED_PHP_BRANCH, host_php_cli_branch, normalize_php_request, stream_for_branch,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CONFIG_FILE: &str = "php-apache-default.json";

/// Parallel Remi package prefixes (newest first).
const REMI_PARALLEL: &[(&str, &str)] = &[
    ("8.6", "php86"),
    ("8.5", "php85"),
    ("8.4", "php84"),
    ("8.3", "php83"),
    ("8.2", "php82"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApachePhpRecord {
    pub branch: String,
    /// `php:remi-8.6`, `remi-parallel:php86`, or `system`.
    pub stream: String,
    #[serde(default)]
    pub requested: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApachePhpKind {
    /// Remi / AppStream modular `php` stream (system php-fpm / CLI).
    Modular,
    /// Remi parallel `/opt/remi/phpXX` packages (safe beside lsphp).
    Parallel,
    /// Distro `/usr/bin/php` without a Remi tag.
    System,
}

#[derive(Debug, Clone)]
pub struct ApachePhpVersion {
    pub branch: String,
    pub label: String,
    pub kind: ApachePhpKind,
    pub prefix: String,
    pub installed: bool,
    pub available: bool,
}

fn config_path() -> PathBuf {
    default_data_dir().join(CONFIG_FILE)
}

/// True when Apache httpd is installed (binary, unit file, or RPM/deb).
pub fn httpd_present() -> bool {
    Path::new("/usr/sbin/httpd").is_file()
        || Path::new("/usr/sbin/apache2").is_file()
        || Path::new("/usr/lib/systemd/system/httpd.service").is_file()
        || Path::new("/lib/systemd/system/httpd.service").is_file()
        || Path::new("/usr/lib/systemd/system/apache2.service").is_file()
        || rpm_or_dpkg_installed(&["httpd", "apache2"])
}

fn dnf_available() -> bool {
    Path::new("/usr/bin/dnf").exists() || Path::new("/usr/bin/yum").exists()
}

pub fn remi_parallel_prefix(branch: &str) -> Option<&'static str> {
    REMI_PARALLEL
        .iter()
        .find(|(b, _)| *b == branch.trim())
        .map(|(_, p)| *p)
}

fn parallel_php_bin(prefix: &str) -> PathBuf {
    PathBuf::from(format!("/opt/remi/{prefix}/root/usr/bin/php"))
}

fn parallel_php_ini(prefix: &str) -> PathBuf {
    PathBuf::from(format!("/opt/remi/{prefix}/root/etc/php.ini"))
}

fn parallel_installed(prefix: &str) -> bool {
    parallel_php_bin(prefix).is_file()
        || package_installed(&format!("{prefix}-php-cli"))
        || package_installed(&format!("{prefix}-php"))
}

/// Locate php.ini for Apache-oriented PHP for a branch (never LiteSpeed lsphp).
pub fn resolve_apache_php_ini(branch: &str) -> Result<(PathBuf, String, ApachePhpKind), String> {
    let branch = branch.trim();
    if let Some(prefix) = remi_parallel_prefix(branch) {
        let bin = parallel_php_bin(prefix);
        if bin.is_file() {
            let ini = php_ini_from_binary(bin.to_str().unwrap_or(""))
                .or_else(|| {
                    let p = parallel_php_ini(prefix);
                    p.is_file().then_some(p)
                })
                .ok_or_else(|| {
                    format!("Could not locate php.ini for Remi parallel {prefix} (PHP {branch})")
                })?;
            return Ok((ini, bin.display().to_string(), ApachePhpKind::Parallel));
        }
    }

    let bin = "php".to_string();
    if Path::new("/usr/bin/php").is_file() || which_php() {
        let ini = php_ini_from_binary(&bin)
            .or_else(|| {
                ["/etc/php.ini", "/etc/php/php.ini", "/etc/php8/php.ini"]
                    .iter()
                    .map(PathBuf::from)
                    .find(|p| p.is_file())
            })
            .ok_or_else(|| {
                format!("Could not locate system php.ini for Apache PHP {branch}. Install php-cli first.")
            })?;
        let kind = if package_installed("php-cli") || package_installed("php") {
            ApachePhpKind::Modular
        } else {
            ApachePhpKind::System
        };
        return Ok((ini, bin, kind));
    }

    Err(format!(
        "No Apache-oriented PHP runtime found for {branch}. Install Remi PHP or parallel phpXX-php packages."
    ))
}

fn which_php() -> bool {
    Command::new("php")
        .arg("-v")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn php_ini_from_binary(bin: &str) -> Option<PathBuf> {
    let out = Command::new(bin).args(["-i"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Loaded Configuration File =>") {
            let p = rest.trim();
            if !p.is_empty() && p != "(none)" {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
}

/// Versions offered for Apache PHP selection (prefer 8.6).
pub fn list_apache_php_versions() -> Vec<ApachePhpVersion> {
    let mut out = Vec::new();
    let dnf = dnf_available();
    for branch in MANAGED_BRANCHES {
        if let Some(prefix) = remi_parallel_prefix(branch) {
            let installed = parallel_installed(prefix);
            if installed || dnf {
                out.push(ApachePhpVersion {
                    branch: (*branch).to_string(),
                    label: if installed {
                        format!("PHP {branch} (Remi {prefix}, Apache/httpd)")
                    } else {
                        format!("PHP {branch} (Remi {prefix}, install for Apache)")
                    },
                    kind: ApachePhpKind::Parallel,
                    prefix: prefix.to_string(),
                    installed,
                    available: installed || dnf,
                });
                continue;
            }
        }
        if dnf || Path::new("/usr/bin/php").is_file() {
            let cli = host_php_cli_branch();
            let installed = cli.as_deref() == Some(*branch) || Path::new("/usr/bin/php").is_file();
            out.push(ApachePhpVersion {
                branch: (*branch).to_string(),
                label: format!("PHP {branch} (system Apache handler)"),
                kind: ApachePhpKind::Modular,
                prefix: "php".into(),
                installed,
                available: true,
            });
        }
    }
    if out.is_empty() {
        for branch in MANAGED_BRANCHES {
            out.push(ApachePhpVersion {
                branch: (*branch).to_string(),
                label: format!("PHP {branch} (Apache)"),
                kind: ApachePhpKind::System,
                prefix: "php".into(),
                installed: false,
                available: dnf,
            });
        }
    }
    out
}

pub fn load_apache_php_default() -> Option<ApachePhpRecord> {
    let raw = fs::read_to_string(config_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_apache_php_default(record: &ApachePhpRecord) -> Result<(), String> {
    let dir = default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    let path = config_path();
    let body = serde_json::to_vec_pretty(record)
        .map_err(|e| format!("Cannot encode php-apache-default.json: {e}"))?;
    fs::write(&path, body).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn run_dnf(args: &[&str]) -> Result<(), String> {
    if !dnf_available() {
        return Err("dnf is not available on this host".into());
    }
    let out = Command::new("dnf")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run dnf: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!(
            "dnf failed: {}",
            err.lines().next().unwrap_or("unknown error").trim()
        ))
    }
}

fn try_switch_stream(stream: &str) -> bool {
    let switch = format!("dnf --setopt=lock_timeout=120 -y module switch-to '{stream}'");
    if Command::new("bash")
        .args(["-c", &switch])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return true;
    }
    let enable = format!(
        "dnf -y module reset php >/dev/null 2>&1 || true; dnf --setopt=lock_timeout=120 -y module enable '{stream}'"
    );
    Command::new("bash")
        .args(["-c", &enable])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn restart_unit(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["try-restart", unit])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn apply_parallel(branch: &str, prefix: &str) -> Result<String, String> {
    let pkgs = [
        format!("{prefix}-php"),
        format!("{prefix}-php-cli"),
        format!("{prefix}-php-fpm"),
        format!("{prefix}-php-common"),
        format!("{prefix}-php-mysqlnd"),
        format!("{prefix}-php-mbstring"),
        format!("{prefix}-php-xml"),
    ];
    let mut args = vec![
        "--setopt=lock_timeout=120".to_string(),
        "install".into(),
        "-y".into(),
    ];
    args.extend(pkgs);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_dnf(&arg_refs)?;
    let fpm_unit = format!("{prefix}-php-fpm");
    let _ = Command::new("systemctl")
        .args(["enable", "--now", &fpm_unit])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = restart_unit(&fpm_unit);
    if httpd_present() {
        let _ = restart_unit("httpd");
        let _ = restart_unit("apache2");
    }
    write_httpd_php_hint(branch, prefix)?;
    Ok(format!(
        "Apache PHP default is {branch} via Remi parallel {prefix} (/opt/remi/{prefix})."
    ))
}

fn write_httpd_php_hint(branch: &str, prefix: &str) -> Result<(), String> {
    if !httpd_present() {
        return Ok(());
    }
    let conf_dir = if Path::new("/etc/httpd/conf.d").is_dir() {
        PathBuf::from("/etc/httpd/conf.d")
    } else if Path::new("/etc/apache2/conf-available").is_dir() {
        PathBuf::from("/etc/apache2/conf-available")
    } else {
        return Ok(());
    };
    let path = conf_dir.join("cpn-apache-php.conf");
    let sock = format!("/var/opt/remi/{prefix}/run/php-fpm/www.sock");
    let alt_sock = format!("/run/php-fpm/{prefix}-php-fpm.sock");
    let body = format!(
        "# Managed by CPN: Apache PHP {branch} ({prefix})\n\
         # Prefer php-fpm via proxy when the socket exists. Adjust per-vhost as needed.\n\
         # Primary socket candidates:\n\
         #   {sock}\n\
         #   {alt_sock}\n\
         #   /run/php-fpm/www.sock\n\
         <IfModule proxy_fcgi_module>\n\
         # Example (disabled by default):\n\
         # <FilesMatch \\.php$>\n\
         #   SetHandler \"proxy:unix:{sock}|fcgi://localhost\"\n\
         # </FilesMatch>\n\
         </IfModule>\n"
    );
    fs::write(&path, body).map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
    Ok(())
}

fn apply_modular(branch: &str) -> Result<String, String> {
    let guest = detect_guest_os()?;
    let stream = stream_for_branch(&guest, branch).unwrap_or_else(|| format!("php:remi-{branch}"));
    if !try_switch_stream(&stream) {
        return Err(format!(
            "Could not switch Apache PHP module stream to {stream}"
        ));
    }
    let _ = run_dnf(&[
        "--setopt=lock_timeout=120",
        "install",
        "-y",
        "php",
        "php-cli",
        "php-fpm",
        "php-mysqlnd",
        "php-mbstring",
        "php-xml",
    ]);
    let _ = restart_unit("php-fpm");
    if httpd_present() {
        let _ = restart_unit("httpd");
        let _ = restart_unit("apache2");
    }
    let runtime = host_php_cli_branch().unwrap_or_default();
    if !runtime.is_empty() && runtime != branch {
        return Err(format!(
            "Apache PHP preference {branch} was saved (stream {stream}), but php still reports {runtime}."
        ));
    }
    Ok(format!(
        "Apache PHP default is {branch} via system module {stream}."
    ))
}

/// Persist and apply the Apache / httpd PHP default (does not rewrite lsphp host default).
pub fn ensure_apache_php_default(requested: Option<&str>) -> Result<String, String> {
    let guest = detect_guest_os().ok();
    let requested_norm = normalize_php_request(requested, guest.as_ref());
    let branch = if MANAGED_BRANCHES.contains(&requested_norm.as_str()) {
        requested_norm.clone()
    } else {
        CPN_PREFERRED_PHP_BRANCH.to_string()
    };

    // Prefer parallel Remi beside LiteSpeed lsphp; otherwise modular Remi/system.
    let use_parallel = crate::php_lsphp_host::should_skip_remi_php_packages()
        || remi_parallel_prefix(&branch)
            .map(parallel_installed)
            .unwrap_or(false);

    let (stream, apply_msg) = if use_parallel {
        let prefix = remi_parallel_prefix(&branch)
            .ok_or_else(|| format!("No Remi parallel package map for PHP {branch}"))?;
        let msg = apply_parallel(&branch, prefix)?;
        (format!("remi-parallel:{prefix}"), msg)
    } else {
        let msg = apply_modular(&branch)?;
        let stream = guest
            .as_ref()
            .and_then(|g| stream_for_branch(g, &branch))
            .unwrap_or_else(|| format!("php:remi-{branch}"));
        (stream, msg)
    };

    let record = ApachePhpRecord {
        branch: branch.clone(),
        stream: stream.clone(),
        requested: requested_norm,
        message: apply_msg.clone(),
    };
    save_apache_php_default(&record)?;

    let httpd_note = if httpd_present() {
        " httpd is present; try-restarted when active."
    } else {
        " Apache httpd is not installed yet. Install via System Repair (heal id: httpd), then re-apply."
    };

    Ok(format!(
        "{apply_msg} Stored in {}.{}",
        config_path().display(),
        httpd_note
    ))
}

/// Soft status for the PHP Configurations UI.
pub fn apache_php_ui_status() -> (bool, String, Option<ApachePhpRecord>) {
    let present = httpd_present();
    let rec = load_apache_php_default();
    let detail = if present {
        match &rec {
            Some(r) => format!("{} ({})", r.branch, r.stream),
            None => "not persisted yet".into(),
        }
    } else {
        "Apache httpd is not installed. Use System Repair to install httpd, then set Apache PHP."
            .into()
    };
    (present, detail, rec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn parallel_prefix_maps_86_first() {
        assert_eq!(remi_parallel_prefix("8.6"), Some("php86"));
        assert_eq!(remi_parallel_prefix("8.5"), Some("php85"));
        assert!(remi_parallel_prefix("7.4").is_none());
    }

    #[test]
    fn apache_default_roundtrip() {
        with_test_data_dir(|| {
            let record = ApachePhpRecord {
                branch: "8.6".into(),
                stream: "remi-parallel:php86".into(),
                requested: "8.6".into(),
                message: "test".into(),
            };
            save_apache_php_default(&record).unwrap();
            let loaded = load_apache_php_default().unwrap();
            assert_eq!(loaded.branch, "8.6");
            assert_eq!(loaded.stream, "remi-parallel:php86");
        });
    }

    #[test]
    fn list_includes_managed_branches_when_dnf_or_empty() {
        let versions = list_apache_php_versions();
        assert!(!versions.is_empty());
        assert_eq!(versions[0].branch, "8.6");
        assert!(versions.iter().any(|v| v.branch == "8.5"));
    }

    #[test]
    fn ui_status_when_no_record() {
        with_test_data_dir(|| {
            let (_present, detail, rec) = apache_php_ui_status();
            assert!(rec.is_none());
            assert!(!detail.is_empty());
        });
    }
}
