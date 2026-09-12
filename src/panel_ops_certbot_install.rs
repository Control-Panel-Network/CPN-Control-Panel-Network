//! Ensure ACME certbot is installed and on PATH (Alma/RHEL dnf or Debian apt).

use crate::panel_ops_ssl_le::certbot_available;
use std::process::{Command, Stdio};

fn run_install(bin: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run {bin}: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&output.stderr);
    let out = String::from_utf8_lossy(&output.stdout);
    Err(format!(
        "{bin} install failed: {} {}",
        out.chars().take(200).collect::<String>(),
        err.chars().take(200).collect::<String>()
    ))
}

/// Install certbot (and Cloudflare DNS plugin when available) if missing.
pub fn ensure_certbot_on_path() -> Result<String, String> {
    if certbot_available() {
        return Ok("certbot already on PATH".into());
    }
    #[cfg(windows)]
    {
        return Err(
            "certbot is not installed. On Windows labs, install certbot manually or issue SSL on the Linux panel host."
                .into(),
        );
    }
    #[cfg(not(windows))]
    {
        if Command::new("dnf")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            let _ = run_install(
                "dnf",
                &[
                    "-y",
                    "install",
                    "certbot",
                    "python3-certbot",
                    "python3-certbot-dns-cloudflare",
                ],
            );
            if !certbot_available() {
                run_install("dnf", &["-y", "install", "certbot"])?;
            }
        } else if Command::new("apt-get")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            let _ = run_install(
                "apt-get",
                &["install", "-y", "certbot", "python3-certbot-dns-cloudflare"],
            );
            if !certbot_available() {
                run_install("apt-get", &["install", "-y", "certbot"])?;
            }
        } else {
            return Err("No dnf/apt-get available to install certbot".into());
        }
        if certbot_available() {
            Ok("Installed certbot onto PATH".into())
        } else {
            Err("certbot still missing after package install".into())
        }
    }
}
