//! Apply `/var/lib/cpn/php-default.json` to host Remi/php-fpm and LiteSpeed lsphp.

use crate::litespeed_stack::openlitespeed_installed;
use crate::os_support::detect_guest_os;
use crate::php_defaults::{
    PhpDefaultRecord, host_php_cli_branch, host_php_fpm_branch, load_php_default,
    prepare_and_persist_php, save_php_default, stream_for_branch, today_ymd,
};
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

fn persist_operator_choice(branch: &str, stream: &str, message: &str) -> Result<(), String> {
    let record = PhpDefaultRecord {
        branch: branch.trim().to_string(),
        stream: stream.to_string(),
        requested: branch.trim().to_string(),
        message: message.to_string(),
    };
    save_php_default(&record)?;
    if load_php_default().is_none() {
        return Err(format!(
            "Wrote php-default.json for PHP {branch} but could not re-read it from {}",
            crate::paths::default_data_dir()
                .join("php-default.json")
                .display()
        ));
    }
    Ok(())
}

/// Persist and apply the host PHP default so CLI and php-fpm (phpMyAdmin) align.
///
/// LiteSpeed `lsphpXX` packages share `/var/lib/php/opcache` with Remi modular PHP on
/// some EL releases, so CPN does not force-install lsphp for the same branch when
/// Remi `php`/`php-fpm` are already the host runtime. Manage `lsphp*` separately from
/// the extensions UI when those packages are already present.
pub fn ensure_host_php_default(requested: Option<&str>) -> Result<String, String> {
    let guest = detect_guest_os()?;
    let req = requested
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .or_else(|| load_php_default().map(|r| r.requested))
        .filter(|v| !v.is_empty());

    // Persist the operator choice first so the UI never claims "not persisted yet"
    // after a successful Set host default click, even if module enable is slow/noisy.
    if let Some(branch) = req.as_deref() {
        let stream =
            stream_for_branch(&guest, branch).unwrap_or_else(|| format!("php:remi-{branch}"));
        persist_operator_choice(
            branch,
            &stream,
            &format!("Operator selected PHP {branch} as host default."),
        )?;
    }

    let record = match prepare_and_persist_php(&guest, req.as_deref(), &today_ymd()) {
        Ok(r) => r,
        Err(err) => {
            // Preference is already on disk; surface the enable/install problem clearly.
            if let Some(branch) = req.as_deref() {
                let stream = stream_for_branch(&guest, branch)
                    .unwrap_or_else(|| format!("php:remi-{branch}"));
                let _ = persist_operator_choice(
                    branch,
                    &stream,
                    &format!(
                        "Saved PHP {branch} as host default preference; module apply reported: {err}"
                    ),
                );
                return Err(format!(
                    "Host default PHP {branch} was saved to php-default.json, but applying the module failed: {err}"
                ));
            }
            return Err(err);
        }
    };

    if dnf_available() && record.stream != "apt" {
        // switch-to already syncs modular packages; ensure core set is present for
        // first-boot / missing php-fpm, then restart so PMA picks up the binary.
        if let Err(err) = run_dnf_install(&[
            "php",
            "php-cli",
            "php-fpm",
            "php-mysqlnd",
            "php-mbstring",
            "php-xml",
            "php-json",
        ]) {
            return Err(format!(
                "Host default PHP {} is persisted, but php-fpm packages could not be installed: {err}",
                record.branch
            ));
        }
        let _ = Command::new("systemctl")
            .args(["reset-failed", "php-fpm"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = Command::new("systemctl")
            .args(["restart", "php-fpm"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        // Fail loud if JSON/module claim X but CLI/FPM still run another branch
        // (the historical bug: enable-only left php-fpm on the previous Remi stream).
        let fpm_branch = host_php_fpm_branch();
        let cli_branch = host_php_cli_branch();
        let runtime = fpm_branch
            .clone()
            .or_else(|| cli_branch.clone())
            .unwrap_or_default();
        if runtime.is_empty() {
            return Err(format!(
                "Host default PHP {} was saved, but php-fpm could not report its version after apply.",
                record.branch
            ));
        }
        if runtime != record.branch {
            return Err(format!(
                "Host default PHP {} was saved (stream {}), but php-fpm still reports PHP {runtime}. Re-run Set as host default or install Remi packages for {}.",
                record.branch, record.stream, record.branch
            ));
        }
    }

    let mut note = record.message.clone();
    if openlitespeed_installed()
        && let Some(prefix) = branch_to_lsphp_prefix(&record.branch)
    {
        let lsphp_bin = format!("/usr/local/lsws/{prefix}/bin/lsphp");
        if Path::new(&lsphp_bin).is_file() {
            note.push_str(&format!(" LiteSpeed {prefix} already present."));
        } else {
            // Avoid Remi vs lsphp file conflicts on /var/lib/php/opcache.
            note.push_str(&format!(
                " Skipped auto-install of {prefix} (may conflict with Remi php-fpm); install from PHP Extensions if needed."
            ));
        }
    }

    if load_php_default().is_none() {
        return Err(
            "Host PHP default apply finished but php-default.json could not be re-read".into(),
        );
    }

    // php-fpm restart clears PMA PHP sessions. Refresh TempDir ownership, the
    // sign-on bridge / SignonURL, and (when OLS owns HTTP) the loopback listener
    // so Open phpMyAdmin remints SSO from the still-valid CPN panel session.
    let _ = crate::apps_phpmyadmin::ensure_phpmyadmin_runtime_dirs();
    if openlitespeed_installed() {
        let _ = crate::apps_phpmyadmin::ensure_fpm_socket_for_ols();
    }
    let _ = crate::apps_phpmyadmin_sso::refresh_phpmyadmin_signon();

    Ok(format!("Host PHP default is {} ({})", record.branch, note))
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
