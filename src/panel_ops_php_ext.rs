//! PHP extension package discovery and install/uninstall (LiteSpeed lsphp preferred).

use crate::litespeed_stack::openlitespeed_installed;
use crate::panel_session::session_secret;
use crate::php_defaults::{
    CPN_PHP_FALLBACK_ORDER, CPN_PREFERRED_PHP_BRANCH, default_php_branch_for_sites,
    load_php_default,
};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// Supported major.minor branches for the extensions manager.
pub const MANAGED_BRANCHES: &[&str] = &["8.5", "8.4", "8.3", "8.2"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhpPackageFamily {
    /// OpenLiteSpeed `lsphp85-*` packages from the LiteSpeed repo.
    LiteSpeed,
    /// Remi / AppStream modular `php-*` packages (system php-fpm / CLI).
    Modular,
}

#[derive(Debug, Clone)]
pub struct PhpVersionOption {
    pub branch: String,
    pub label: String,
    pub family: PhpPackageFamily,
    pub prefix: String,
    pub installed: bool,
    pub available: bool,
}

#[derive(Debug, Clone)]
pub struct PhpExtensionRow {
    pub id: usize,
    pub branch: String,
    pub package: String,
    pub name: String,
    pub description: String,
    pub installed: bool,
    pub protected: bool,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// CSRF token for PHP extension POST actions (bound to user + hour bucket).
pub fn php_ext_csrf_token(username: &str) -> String {
    let secret = session_secret(None);
    let hour = now_unix() / 3600;
    let payload = format!("php-ext|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(&secret, &payload))
}

pub fn verify_php_ext_csrf(username: &str, token: &str) -> bool {
    let secret = session_secret(None);
    let Some((hour_s, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(hour) = hour_s.parse::<u64>() else {
        return false;
    };
    let current = now_unix() / 3600;
    if hour + 2 < current || hour > current + 1 {
        return false;
    }
    let payload = format!("php-ext|{username}|{hour}");
    let expected = hmac_hex(&secret, &payload);
    expected == sig && verify_hmac_hex_compat(&expected, sig)
}

fn verify_hmac_hex_compat(a: &str, b: &str) -> bool {
    // Constant-time-ish compare via existing helper when available.
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn branch_to_lsphp_prefix(branch: &str) -> Option<String> {
    let cleaned = branch.trim();
    match cleaned {
        "8.5" => Some("lsphp85".into()),
        "8.4" => Some("lsphp84".into()),
        "8.3" => Some("lsphp83".into()),
        "8.2" => Some("lsphp82".into()),
        _ => None,
    }
}

fn lsphp_dir_present(prefix: &str) -> bool {
    Path::new(&format!("/usr/local/lsws/{prefix}/bin/lsphp")).is_file()
}

fn dnf_available() -> bool {
    Path::new("/usr/bin/dnf").exists() || Path::new("/usr/bin/yum").exists()
}

fn run_capture(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run {program}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        Err(format!(
            "{program} failed ({}): {}",
            out.status.code().unwrap_or(-1),
            stderr.lines().next().unwrap_or("no details").trim()
        ))
    }
}

fn package_names_from_dnf_list(raw: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("Installed")
            || line.starts_with("Available")
            || line.starts_with("Last metadata")
        {
            continue;
        }
        let pkg = line.split_whitespace().next().unwrap_or("");
        let base = pkg.split('.').next().unwrap_or(pkg);
        if !base.is_empty() {
            names.insert(base.to_string());
        }
    }
    names
}

fn dnf_list_names(pattern: &str, installed: bool) -> BTreeSet<String> {
    if !dnf_available() {
        return BTreeSet::new();
    }
    let mode = if installed { "installed" } else { "available" };
    match run_capture("dnf", &["-q", "list", mode, pattern]) {
        Ok(raw) => package_names_from_dnf_list(&raw),
        Err(_) => BTreeSet::new(),
    }
}

/// Default branch for the dropdown (php-default.json, else preferred 8.5 / EL8 8.2).
pub fn selected_default_branch() -> String {
    if let Some(rec) = load_php_default() {
        if MANAGED_BRANCHES.contains(&rec.branch.as_str()) {
            return rec.branch;
        }
    }
    let preferred = default_php_branch_for_sites();
    if MANAGED_BRANCHES.contains(&preferred.as_str()) {
        preferred
    } else {
        CPN_PREFERRED_PHP_BRANCH.to_string()
    }
}

/// Discover PHP versions available on this host for extension management.
pub fn list_php_versions() -> Vec<PhpVersionOption> {
    let prefer_ls = openlitespeed_installed() && dnf_available();
    let mut out = Vec::new();
    for branch in MANAGED_BRANCHES {
        if prefer_ls {
            if let Some(prefix) = branch_to_lsphp_prefix(branch) {
                let installed = lsphp_dir_present(&prefix)
                    || !dnf_list_names(&format!("{prefix}"), true).is_empty()
                    || !dnf_list_names(&format!("{prefix}-*"), true).is_empty();
                let available = installed
                    || !dnf_list_names(&format!("{prefix}"), false).is_empty()
                    || !dnf_list_names(&format!("{prefix}-*"), false).is_empty();
                if available || *branch == CPN_PREFERRED_PHP_BRANCH || *branch == "8.3" {
                    out.push(PhpVersionOption {
                        branch: (*branch).to_string(),
                        label: format!("PHP {branch} (LiteSpeed {prefix})"),
                        family: PhpPackageFamily::LiteSpeed,
                        prefix,
                        installed,
                        available: available || installed,
                    });
                    continue;
                }
            }
        }
        // Modular Remi / AppStream path when OLS lsphp is not the package surface.
        out.push(PhpVersionOption {
            branch: (*branch).to_string(),
            label: format!("PHP {branch} (system module)"),
            family: PhpPackageFamily::Modular,
            prefix: "php".into(),
            installed: Path::new("/usr/bin/php").exists(),
            available: dnf_available(),
        });
    }
    if out.is_empty() {
        for branch in CPN_PHP_FALLBACK_ORDER {
            out.push(PhpVersionOption {
                branch: (*branch).to_string(),
                label: format!("PHP {branch}"),
                family: PhpPackageFamily::Modular,
                prefix: "php".into(),
                installed: false,
                available: dnf_available(),
            });
        }
    }
    out
}

fn resolve_version(branch: &str) -> Result<PhpVersionOption, String> {
    let branch = branch.trim();
    list_php_versions()
        .into_iter()
        .find(|v| v.branch == branch)
        .ok_or_else(|| format!("PHP {branch} is not available on this host"))
}

fn display_name_for_package(prefix: &str, package: &str) -> String {
    if package == prefix {
        return format!("{prefix} (runtime)");
    }
    let stripped = package
        .strip_prefix(&format!("{prefix}-"))
        .unwrap_or(package);
    stripped.to_string()
}

fn is_protected_package(family: PhpPackageFamily, prefix: &str, package: &str) -> bool {
    match family {
        PhpPackageFamily::LiteSpeed => {
            package == prefix
                || package == format!("{prefix}-common")
                || package.ends_with("-debugsource")
                || package.ends_with("-devel")
                || package.ends_with("-dbg")
        }
        PhpPackageFamily::Modular => matches!(
            package,
            "php" | "php-cli" | "php-common" | "php-fpm" | "php-json" | "php-mysqlnd"
        ),
    }
}

fn package_allowed(family: PhpPackageFamily, prefix: &str, package: &str) -> bool {
    if package.is_empty() || package.contains('/') || package.contains(' ') {
        return false;
    }
    match family {
        PhpPackageFamily::LiteSpeed => {
            package == prefix
                || package.starts_with(&format!("{prefix}-"))
                    && package
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
        }
        PhpPackageFamily::Modular => {
            (package == "php" || package.starts_with("php-"))
                && package
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
        }
    }
}

/// List extension packages for a PHP branch (install + available).
pub fn list_extensions(branch: &str, search: &str) -> Result<Vec<PhpExtensionRow>, String> {
    let ver = resolve_version(branch)?;
    let pattern = match ver.family {
        PhpPackageFamily::LiteSpeed => format!("{}*", ver.prefix),
        PhpPackageFamily::Modular => "php-*".into(),
    };
    let installed = dnf_list_names(&pattern, true);
    let available = dnf_list_names(&pattern, false);
    let mut all: BTreeMap<String, bool> = BTreeMap::new();
    for name in &installed {
        all.insert(name.clone(), true);
    }
    for name in &available {
        all.entry(name.clone()).or_insert(false);
    }
    // Always include the base runtime package when LiteSpeed.
    if ver.family == PhpPackageFamily::LiteSpeed {
        let base_installed = installed.contains(&ver.prefix) || lsphp_dir_present(&ver.prefix);
        all.entry(ver.prefix.clone()).or_insert(base_installed);
        if base_installed {
            all.insert(ver.prefix.clone(), true);
        }
    }

    let search = search.trim().to_ascii_lowercase();
    let mut rows = Vec::new();
    let mut id = 1usize;
    for (package, is_inst) in all {
        if !package_allowed(ver.family, &ver.prefix, &package) {
            continue;
        }
        // Skip noisy -devel/-dbg/-debugsource in the main table unless searching.
        if search.is_empty()
            && (package.ends_with("-devel")
                || package.ends_with("-dbg")
                || package.ends_with("-debugsource"))
        {
            continue;
        }
        let name = display_name_for_package(&ver.prefix, &package);
        if !search.is_empty()
            && !package.to_ascii_lowercase().contains(&search)
            && !name.to_ascii_lowercase().contains(&search)
        {
            continue;
        }
        let description = rpm_short_desc(&package);
        rows.push(PhpExtensionRow {
            id,
            branch: ver.branch.clone(),
            package: package.clone(),
            name,
            description,
            installed: is_inst || (package == ver.prefix && lsphp_dir_present(&ver.prefix)),
            protected: is_protected_package(ver.family, &ver.prefix, &package),
        });
        id += 1;
    }
    Ok(rows)
}

fn rpm_short_desc(package: &str) -> String {
    if let Ok(raw) = run_capture(
        "rpm",
        &[
            "-q",
            "--qf",
            "%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH} %{SUMMARY}",
            package,
        ],
    ) {
        let line = raw.lines().next().unwrap_or("").trim();
        if !line.is_empty() && !line.contains("not installed") {
            return line.to_string();
        }
    }
    package.to_string()
}

pub fn install_extension(branch: &str, package: &str) -> Result<String, String> {
    let ver = resolve_version(branch)?;
    let package = package.trim();
    if !package_allowed(ver.family, &ver.prefix, package) {
        return Err(format!(
            "Package '{package}' is not allowed for PHP {}",
            ver.branch
        ));
    }
    if !dnf_available() {
        return Err("dnf is not available on this host".into());
    }
    run_capture(
        "dnf",
        &["--setopt=lock_timeout=60", "install", "-y", package],
    )?;
    Ok(format!("Installed {package} for PHP {}", ver.branch))
}

pub fn uninstall_extension(branch: &str, package: &str) -> Result<String, String> {
    let ver = resolve_version(branch)?;
    let package = package.trim();
    if !package_allowed(ver.family, &ver.prefix, package) {
        return Err(format!(
            "Package '{package}' is not allowed for PHP {}",
            ver.branch
        ));
    }
    if is_protected_package(ver.family, &ver.prefix, package) {
        return Err(format!(
            "Refusing to remove protected package {package}. Uninstall optional extensions only."
        ));
    }
    if !dnf_available() {
        return Err("dnf is not available on this host".into());
    }
    run_capture(
        "dnf",
        &["--setopt=lock_timeout=60", "remove", "-y", package],
    )?;
    Ok(format!("Removed {package} from PHP {}", ver.branch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsphp_prefix_maps() {
        assert_eq!(branch_to_lsphp_prefix("8.5").as_deref(), Some("lsphp85"));
        assert_eq!(branch_to_lsphp_prefix("8.2").as_deref(), Some("lsphp82"));
        assert!(branch_to_lsphp_prefix("7.4").is_none());
    }

    #[test]
    fn package_allowlist() {
        assert!(package_allowed(
            PhpPackageFamily::LiteSpeed,
            "lsphp85",
            "lsphp85-bcmath"
        ));
        assert!(!package_allowed(
            PhpPackageFamily::LiteSpeed,
            "lsphp85",
            "lsphp84-bcmath"
        ));
        assert!(!package_allowed(
            PhpPackageFamily::LiteSpeed,
            "lsphp85",
            "../evil"
        ));
        assert!(package_allowed(
            PhpPackageFamily::Modular,
            "php",
            "php-bcmath"
        ));
        assert!(!package_allowed(PhpPackageFamily::Modular, "php", "nginx"));
    }

    #[test]
    fn protected_base_packages() {
        assert!(is_protected_package(
            PhpPackageFamily::LiteSpeed,
            "lsphp85",
            "lsphp85-common"
        ));
        assert!(!is_protected_package(
            PhpPackageFamily::LiteSpeed,
            "lsphp85",
            "lsphp85-bcmath"
        ));
    }
}
