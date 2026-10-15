//! Optional CSF (ConfigServer Security & Firewall) host package helpers.

use crate::apps_pkg::{package_manager, run_pkg};
use crate::panel_ops_security::{cmd_stdout, which_exists};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CsfStatus {
    pub installed: bool,
    pub enabled: bool,
    pub version: String,
    pub detail: String,
}

pub fn csf_status() -> CsfStatus {
    let bin = which_exists("csf") || Path::new("/usr/sbin/csf").is_file();
    if !bin {
        return CsfStatus {
            installed: false,
            enabled: false,
            version: String::new(),
            detail: "CSF is not installed. Install the optional Host package from Plugins or Server > Services.".into(),
        };
    }
    let version = cmd_stdout("csf", &["-v"])
        .or_else(|| cmd_stdout("/usr/sbin/csf", &["-v"]))
        .unwrap_or_else(|| "CSF".into());
    let enabled = Path::new("/etc/csf/csf.conf").is_file()
        && Command::new("systemctl")
            .args(["is-active", "--quiet", "csf"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    CsfStatus {
        installed: true,
        enabled,
        version: version.chars().take(80).collect(),
        detail: if enabled {
            "CSF is installed and the service reports active.".into()
        } else {
            "CSF binaries are present. Enable/start the CSF service from Server > Services.".into()
        },
    }
}

/// Best-effort install of CSF from the vendor tarball (AlmaLinux/RHEL family).
pub fn install_csf() -> Result<String, String> {
    if csf_status().installed {
        return Ok("CSF is already installed.".into());
    }
    // Ensure perl (CSF dependency) via package manager when available.
    if let Ok(pm) = package_manager() {
        let _ = if pm == "dnf" || pm == "yum" {
            run_pkg(&["install", "-y", "perl", "wget", "tar"])
        } else {
            run_pkg(&["install", "-y", "perl", "wget", "tar"])
        };
    }
    let cache = Path::new("/var/lib/cpn/cache/csf");
    fs::create_dir_all(cache).map_err(|e| format!("Cannot create CSF cache: {e}"))?;
    let tgz = cache.join("csf.tgz");
    let status = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "20",
            "--max-time",
            "180",
            "-o",
            &tgz.display().to_string(),
            "https://download.configserver.com/csf.tgz",
        ])
        .status()
        .map_err(|e| format!("curl failed: {e}"))?;
    if !status.success() {
        return Err("Could not download CSF package from ConfigServer.".into());
    }
    let extract = cache.join("src");
    let _ = fs::remove_dir_all(&extract);
    fs::create_dir_all(&extract).map_err(|e| format!("Cannot extract CSF: {e}"))?;
    let tar = Command::new("tar")
        .args([
            "-xzf",
            &tgz.display().to_string(),
            "-C",
            &extract.display().to_string(),
        ])
        .status()
        .map_err(|e| format!("tar failed: {e}"))?;
    if !tar.success() {
        return Err("Could not extract CSF archive.".into());
    }
    let installer = extract.join("csf").join("install.sh");
    if !installer.is_file() {
        return Err("CSF install.sh was not found in the archive.".into());
    }
    let install = Command::new("sh")
        .arg(&installer)
        .current_dir(extract.join("csf"))
        .status()
        .map_err(|e| format!("CSF install.sh failed: {e}"))?;
    if !install.success() {
        return Err("CSF install.sh exited with an error.".into());
    }
    let _ = Command::new("systemctl")
        .args(["enable", "--now", "csf"])
        .status();
    let _ = Command::new("systemctl")
        .args(["enable", "--now", "lfd"])
        .status();
    Ok("CSF installed. Review /etc/csf/csf.conf and whitelist panel IPs before enabling TESTING=0.".into())
}

pub fn enable_csf() -> Result<String, String> {
    if !csf_status().installed {
        return Err("CSF is not installed.".into());
    }
    let e = Command::new("systemctl")
        .args(["enable", "--now", "csf"])
        .status()
        .map_err(|e| format!("systemctl failed: {e}"))?;
    let l = Command::new("systemctl")
        .args(["enable", "--now", "lfd"])
        .status()
        .map_err(|e| format!("systemctl failed: {e}"))?;
    if e.success() && l.success() {
        Ok("CSF and LFD enabled.".into())
    } else {
        Err("Could not enable CSF/LFD units.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_without_csf_is_honest() {
        let st = csf_status();
        if !which_exists("csf") && !Path::new("/usr/sbin/csf").is_file() {
            assert!(!st.installed);
            assert!(st.detail.to_ascii_lowercase().contains("not installed"));
        }
    }
}
