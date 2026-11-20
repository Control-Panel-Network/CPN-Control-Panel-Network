//! LiteSpeed `lsphp*` host runtime detection (CyberPanel / OLS migration installs).
//!
//! Remi modular `php-fpm` and `lsphp*-common` both own `/var/lib/php/opcache` on EL9+.
//! CPN must not pull Remi `php-fpm` / `php-opcache` when LiteSpeed PHP is already present.

use crate::apps_pkg::{package_installed, rpm_or_dpkg_installed};
use crate::php_defaults::{PhpDefaultRecord, save_php_default, normalize_php_request};
use crate::os_support::GuestOs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LSPHP_SERIES: &[(&str, &str)] = &[
    ("8.6", "lsphp86"),
    ("8.5", "lsphp85"),
    ("8.4", "lsphp84"),
    ("8.3", "lsphp83"),
    ("8.2", "lsphp82"),
];

const LSPHP_COMMON_SUFFIX: &str = "-common";

/// Highest installed LiteSpeed PHP branch (for example `8.5`) and package prefix (`lsphp85`).
pub fn installed_lsphp_series() -> Option<(String, String)> {
    for &(branch, prefix) in LSPHP_SERIES {
        let common = format!("{prefix}{LSPHP_COMMON_SUFFIX}");
        if rpm_or_dpkg_installed(&[prefix])
            || package_installed(&common)
            || lsphp_binary_path(prefix).is_some()
        {
            return Some((branch.to_string(), prefix.to_string()));
        }
    }
    None
}

fn lsphp_binary_path(prefix: &str) -> Option<PathBuf> {
    let bin = PathBuf::from(format!("/usr/local/lsws/{prefix}/bin/lsphp"));
    if bin.is_file() {
        return Some(bin);
    }
    let alt = PathBuf::from(format!("/usr/local/lsws/{prefix}/bin/php"));
    if alt.is_file() {
        return Some(alt);
    }
    None
}

/// Best CLI binary for PHP version checks on LiteSpeed-only hosts.
pub fn best_lsphp_cli_binary() -> Option<PathBuf> {
    installed_lsphp_series()
        .and_then(|(_, prefix)| lsphp_binary_path(&prefix))
}

fn lsphp_common_on_disk() -> bool {
    rpm_or_dpkg_installed(&[
        "lsphp86-common",
        "lsphp85-common",
        "lsphp84-common",
        "lsphp83-common",
        "lsphp82-common",
    ])
}

/// True when LiteSpeed PHP is present and Remi modular `php-fpm` must not be installed.
pub fn should_skip_remi_php_packages() -> bool {
    lsphp_common_on_disk() || installed_lsphp_series().is_some()
}

/// Public alias for installers and PMA recipes.
pub fn host_uses_lsphp_runtime() -> bool {
    should_skip_remi_php_packages()
}

/// Persist host PHP metadata without enabling Remi modular streams.
pub fn prepare_lsphp_host_php_record(
    guest: &GuestOs,
    requested: Option<&str>,
) -> Result<PhpDefaultRecord, String> {
    let (default_branch, prefix) = installed_lsphp_series().ok_or_else(|| {
        "LiteSpeed lsphp packages were not found on this host.".to_string()
    })?;
    let requested_norm = normalize_php_request(requested, Some(guest));
    let branch = if requested_norm == default_branch || requested.is_none() {
        default_branch.clone()
    } else {
        // Honor operator branch when a matching lsphp tree exists.
        LSPHP_SERIES
            .iter()
            .find(|(b, _)| *b == requested_norm.as_str())
            .map(|(b, _)| b.to_string())
            .unwrap_or(default_branch)
    };
    let stream = format!("lsphp:{prefix}");
    let message = format!(
        "Using existing LiteSpeed {prefix} (PHP {branch}) as host runtime. Skipped Remi php-fpm install to avoid /var/lib/php/opcache conflicts with lsphp."
    );
    let record = PhpDefaultRecord {
        branch,
        stream,
        requested: requested_norm,
        message,
    };
    save_php_default(&record)?;
    Ok(record)
}

/// Install common lsphp extensions for webmail / phpMyAdmin without touching Remi php-fpm.
pub fn ensure_lsphp_panel_extensions() -> Result<String, String> {
    let Some((branch, prefix)) = installed_lsphp_series() else {
        return Err("No lsphp series detected.".into());
    };
    let pkgs = [
        format!("{prefix}-mysqlnd"),
        format!("{prefix}-gd"),
        format!("{prefix}-mbstring"),
        format!("{prefix}-xml"),
        format!("{prefix}-json"),
        format!("{prefix}-zip"),
        format!("{prefix}-process"),
    ];
    let mut install: Vec<String> = Vec::new();
    for pkg in &pkgs {
        if !package_installed(pkg) {
            install.push(pkg.clone());
        }
    }
    if install.is_empty() {
        return Ok(format!("LiteSpeed {prefix} extensions already present (PHP {branch})."));
    }
    let args: Vec<&str> = std::iter::once("install")
        .chain(std::iter::once("-y"))
        .chain(install.iter().map(String::as_str))
        .collect();
    let status = Command::new("dnf")
        .args(&args)
        .status()
        .map_err(|e| format!("dnf install lsphp extensions: {e}"))?;
    if !status.success() {
        return Err(format!(
            "Could not install LiteSpeed PHP extensions: {}",
            install.join(", ")
        ));
    }
    Ok(format!(
        "Installed LiteSpeed PHP extensions for {prefix} (PHP {branch})."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_order_prefers_newest_first() {
        assert_eq!(LSPHP_SERIES[0].0, "8.6");
        assert_eq!(LSPHP_SERIES[1].1, "lsphp85");
    }
}