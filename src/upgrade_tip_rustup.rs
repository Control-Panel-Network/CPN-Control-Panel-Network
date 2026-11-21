//! Rustup heal for tip/source upgrades when `cargo` is missing from PATH.
//!
//! Captures rustup-init stdout/stderr into the panel log (never secrets),
//! retries with backoff, uses noninteractive flags, and verifies cargo after
//! install. Avoids `.status()` with unread piped stdio (deadlock / lost errors).

use crate::installer::AppState;
use crate::upgrade_tip_log;
use crate::upgrade_tip_toolchain;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const RUSTUP_INIT_URL: &str = "https://sh.rustup.rs";
const RUSTUP_SCRIPT: &str = "/var/tmp/cpn-rustup-init.sh";
const MAX_ATTEMPTS: u32 = 3;

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn sanitize_log_chunk(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Keep operator-useful rustup lines; drop very long noise.
    clip(trimmed, 800)
}

fn essential_path() -> String {
    let mut dirs: Vec<PathBuf> = Vec::new();
    // Always prepend Linux panel defaults (harmless if missing on Windows CI).
    for raw in [
        "/usr/local/sbin",
        "/usr/local/bin",
        "/usr/sbin",
        "/usr/bin",
        "/sbin",
        "/bin",
    ] {
        dirs.push(PathBuf::from(raw));
    }
    for raw in ["/root/.cargo/bin", "/home/cpn/.cargo/bin"] {
        let p = PathBuf::from(raw);
        if p.is_dir() && !dirs.iter().any(|d| d == &p) {
            dirs.push(p);
        }
    }
    if let Ok(existing) = std::env::var("PATH") {
        for part in std::env::split_paths(&existing) {
            if !part.as_os_str().is_empty() && !dirs.iter().any(|d| d == &part) {
                dirs.push(part);
            }
        }
    }
    std::env::join_paths(&dirs)
        .map(|os| os.to_string_lossy().into_owned())
        .unwrap_or_else(|_| {
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into()
        })
}

fn cargo_after_heal(home: &Path) -> Option<PathBuf> {
    let candidate = home.join(".cargo/bin/cargo");
    if candidate.is_file() {
        return Some(candidate);
    }
    upgrade_tip_toolchain::discover().map(|t| t.cargo)
}

fn log_combined_output(stdout: &[u8], stderr: &[u8], attempt: u32) {
    let out = String::from_utf8_lossy(stdout);
    let err = String::from_utf8_lossy(stderr);
    for line in out.lines().chain(err.lines()) {
        let chunk = sanitize_log_chunk(line);
        if chunk.is_empty() {
            continue;
        }
        let lower = chunk.to_ascii_lowercase();
        if lower.contains("error")
            || lower.contains("failed")
            || lower.contains("fatal")
            || lower.contains("permission denied")
            || lower.contains("no space")
        {
            upgrade_tip_log::log_failure(format!("rustup-init: {chunk}"), Some(attempt));
        } else {
            upgrade_tip_log::log_info(format!("rustup-init: {chunk}"));
        }
    }
}

async fn download_rustup_script(script: &Path, attempt: u32) -> Result<(), String> {
    let path_env = essential_path();
    let output = Command::new("curl")
        .args([
            "--proto",
            "=https",
            "--tlsv1.2",
            "-sSf",
            "--retry",
            "3",
            "--retry-delay",
            "2",
            "--max-time",
            "120",
            "-o",
        ])
        .arg(script)
        .arg(RUSTUP_INIT_URL)
        .env("PATH", &path_env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("rustup-init download failed to start: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "rustup-init download failed ({})",
            clip(stderr.trim(), 240)
        ));
    }
    let meta = std::fs::metadata(script).map_err(|error| {
        format!("rustup-init script missing after download: {error}")
    })?;
    if meta.len() < 64 {
        return Err(format!(
            "rustup-init download too small ({} bytes); refusing to run",
            meta.len()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(script, perms);
    }
    upgrade_tip_log::log_info(format!(
        "Downloaded rustup-init script ({} bytes, attempt={attempt})",
        meta.len()
    ));
    Ok(())
}

async fn run_rustup_init(script: &Path, home: &Path, attempt: u32) -> Result<(), String> {
    let cargo_home = home.join(".cargo");
    let rustup_home = home.join(".rustup");
    let path_env = essential_path();
    // Use `.output()` so pipes are drained (avoids deadlock and lost stderr).
    let output = Command::new("sh")
        .arg(script)
        .args([
            "-y",
            "--default-toolchain",
            "stable",
            "--profile",
            "minimal",
        ])
        .env("HOME", home)
        .env("CARGO_HOME", &cargo_home)
        .env("RUSTUP_HOME", &rustup_home)
        .env("PATH", &path_env)
        .env("RUSTUP_INIT_SKIP_PATH_CHECK", "yes")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("rustup-init failed to start: {error}"))?;
    log_combined_output(&output.stdout, &output.stderr, attempt);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            clip(stderr.trim(), 320)
        } else {
            clip(stdout.trim(), 320)
        };
        return Err(format!(
            "rustup-init exited {status} ({detail})",
            status = output.status
        ));
    }
    Ok(())
}

/// Install rustup into the lab/root home when tip source builds need cargo.
pub async fn heal_rustup(state: &AppState) -> Result<(), String> {
    let path_env = essential_path();
    let curl_ok = Command::new("curl")
        .args(["--version"])
        .env("PATH", &path_env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);
    if !curl_ok {
        return Err(
            "Tip upgrade rustup heal requires `curl` on PATH (/usr/bin/curl)".into(),
        );
    }

    let home = upgrade_tip_toolchain::rustup_home_dir();
    let script = PathBuf::from(RUSTUP_SCRIPT);
    state
        .progress("installing", 8, "Installing Rust toolchain (rustup)")
        .await;

    let mut last_error = String::from("rustup-init did not complete successfully");
    for attempt in 1..=MAX_ATTEMPTS {
        upgrade_tip_log::log_info(format!(
            "Installing rustup for tip upgrade (attempt {attempt}/{MAX_ATTEMPTS}, home={})",
            home.display()
        ));
        let _ = std::fs::remove_file(&script);

        if let Err(error) = download_rustup_script(&script, attempt).await {
            last_error = error;
            upgrade_tip_log::log_failure(&last_error, Some(attempt));
            if attempt < MAX_ATTEMPTS {
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            }
            continue;
        }

        match run_rustup_init(&script, &home, attempt).await {
            Ok(()) => {
                let _ = std::fs::remove_file(&script);
                if let Some(cargo) = cargo_after_heal(&home) {
                    upgrade_tip_log::log_info(format!(
                        "rustup heal ok; cargo at {}",
                        cargo.display()
                    ));
                    return Ok(());
                }
                last_error = format!(
                    "rustup-init finished but cargo still missing under {}/.cargo/bin",
                    home.display()
                );
                upgrade_tip_log::log_failure(&last_error, Some(attempt));
            }
            Err(error) => {
                let _ = std::fs::remove_file(&script);
                last_error = error;
                upgrade_tip_log::log_failure(
                    format!("{last_error} (will retry if attempts remain)"),
                    Some(attempt),
                );
            }
        }

        if attempt < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
        }
    }

    Err(format!(
        "{last_error}. Install rustup manually (curl https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal), ensure gcc/make are present for source builds, or upgrade via a published release package when available."
    ))
}

/// Warn when the C toolchain needed by most crates is missing.
pub fn warn_if_missing_cc() {
    let path_env = essential_path();
    let found = ["cc", "gcc"].iter().any(|name| {
        std::env::split_paths(&path_env).any(|dir| {
            let candidate = dir.join(name);
            candidate.is_file()
        })
    });
    if !found {
        upgrade_tip_log::log_failure(
            "No C compiler (`cc`/`gcc`) on PATH; tip source builds often fail until gcc/make/glibc-devel are installed",
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn essential_path_includes_usr_bin() {
        let path = essential_path();
        assert!(path.contains("/usr/bin"));
        assert!(!path.contains('\u{2014}'));
    }

    #[test]
    fn sanitize_clips_long_lines() {
        let long = "x".repeat(2000);
        assert!(sanitize_log_chunk(&long).len() <= 800);
    }

    #[test]
    fn clip_empty_ok() {
        assert_eq!(clip("", 10), "");
        assert_eq!(clip("abcdef", 3), "abc");
    }
}
