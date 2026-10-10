//! Optional prebuilt binaries / packages from GitHub Actions for a tip commit SHA.

use crate::os_support::{self, PackageFamily};
use crate::releases_cache;
use crate::upgrade_pkg::{self, install_binary};
use crate::upgrade_tip_log;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

fn installer_user_agent() -> String {
    format!("CPN-Installer/{}", env!("CARGO_PKG_VERSION"))
}

async fn curl_github_json(url: &str) -> Result<(u16, String), String> {
    let tmp = std::env::temp_dir().join(format!("cpn-gh-art-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let mut args: Vec<String> = vec![
        "--silent".into(),
        "--show-error".into(),
        "--location".into(),
        "--max-time".into(),
        "20".into(),
        "-o".into(),
        tmp.to_string_lossy().into_owned(),
        "-w".into(),
        "%{http_code}".into(),
        "-H".into(),
        "Accept: application/vnd.github+json".into(),
        "-H".into(),
        format!("User-Agent: {}", installer_user_agent()),
    ];
    if let Some(token) = releases_cache::github_token() {
        args.push("-H".into());
        args.push(format!("Authorization: Bearer {token}"));
    }
    args.push(url.into());
    let output = Command::new("curl")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("GitHub Actions query failed: {error}"))?;
    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u16>()
        .unwrap_or(0);
    let body = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    Ok((status, body))
}

/// Raw Linux tip binaries (zip containing `cpn-installer`).
fn artifact_looks_like_linux_bins(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.contains("windows") || lower.contains("win64") {
        return false;
    }
    lower.contains("cpn-linux")
        || lower.contains("tip-bin")
        || lower.contains("linux-tip")
        || (lower.contains("cpn-installer") && lower.contains("linux"))
}

/// Guest-matching Release workflow packages (`cpn-rpm-el9`, `cpn-rpm-el10`, `cpn-deb-release`).
fn artifact_looks_like_guest_package(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.contains("windows") {
        return false;
    }
    let Ok(guest) = os_support::detect_guest_os() else {
        return false;
    };
    match guest.family {
        PackageFamily::Dnf => {
            lower == format!("cpn-rpm-el{}", guest.major)
                || lower.contains(&format!("rpm-el{}", guest.major))
        }
        PackageFamily::Apt => {
            lower.contains("cpn-deb") || lower.contains("deb-release") || lower.ends_with("-deb")
        }
        PackageFamily::Windows => false,
    }
}

fn find_built_installer(root: &Path) -> Option<PathBuf> {
    let direct = root.join("cpn-installer");
    if direct.is_file() {
        return Some(direct);
    }
    let nested = root.join("target/release/cpn-installer");
    if nested.is_file() {
        return Some(nested);
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && let Some(found) = find_built_installer(&path)
        {
            return Some(found);
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("cpn-installer") && path.is_file() {
            return Some(path);
        }
    }
    None
}

fn find_package_file(root: &Path, ext: &str) -> Option<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir()
            && let Some(found) = find_package_file(&path, ext)
        {
            return Some(found);
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        {
            return Some(path);
        }
    }
    None
}

/// Returns Ok(true) when binaries were installed from Actions artifacts.
pub async fn try_apply_actions_artifacts(
    repo: &str,
    sha: &str,
    work: &Path,
) -> Result<bool, String> {
    let url =
        format!("https://api.github.com/repos/{repo}/actions/runs?head_sha={sha}&per_page=10");
    let (status, body) = curl_github_json(&url).await?;
    if status == 401 || status == 403 {
        upgrade_tip_log::log_info(format!(
            "GitHub Actions artifact lookup HTTP {status} (token missing or rate-limited); falling through to source build"
        ));
        return Ok(false);
    }
    if status != 200 {
        upgrade_tip_log::log_info(format!(
            "GitHub Actions runs HTTP {status} for {repo}@{sha}; no prebuilt tip binaries"
        ));
        return Ok(false);
    }
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let runs = value
        .get("workflow_runs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if runs.is_empty() {
        upgrade_tip_log::log_info(format!(
            "No GitHub Actions runs for {repo}@{sha}; building from source"
        ));
        return Ok(false);
    }
    let mut seen_names: Vec<String> = Vec::new();
    for run in &runs {
        let Some(run_id) = run.get("id").and_then(|v| v.as_u64()) else {
            continue;
        };
        let art_url =
            format!("https://api.github.com/repos/{repo}/actions/runs/{run_id}/artifacts");
        let (art_status, art_body) = curl_github_json(&art_url).await?;
        if art_status != 200 {
            continue;
        }
        let art_json: serde_json::Value =
            serde_json::from_str(&art_body).unwrap_or(serde_json::Value::Null);
        let artifacts = art_json
            .get("artifacts")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        // Prefer raw tip binaries, then guest-matching RPM/DEB packages.
        let mut ordered: Vec<serde_json::Value> = Vec::new();
        for artifact in &artifacts {
            let name = artifact.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if artifact_looks_like_linux_bins(name) {
                ordered.push(artifact.clone());
            }
        }
        for artifact in &artifacts {
            let name = artifact.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if artifact_looks_like_guest_package(name) {
                ordered.push(artifact.clone());
            }
        }
        for artifact in ordered {
            let name = artifact.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let expired = artifact
                .get("expired")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let Some(dl) = artifact
                .get("archive_download_url")
                .and_then(|v| v.as_str())
            else {
                continue;
            };
            if expired {
                continue;
            }
            seen_names.push(name.to_string());
            upgrade_tip_log::log_info(format!("Downloading GitHub Actions artifact {name}"));
            if install_from_zip(dl, work, name).await? {
                return Ok(true);
            }
        }
    }
    if seen_names.is_empty() {
        upgrade_tip_log::log_info(format!(
            "GitHub Actions runs for {repo}@{sha} had no guest-matching tip binary or package artifact; building from source"
        ));
    } else {
        upgrade_tip_log::log_info(format!(
            "GitHub Actions artifacts tried ({}) but none installed; building from source",
            seen_names.join(", ")
        ));
    }
    Ok(false)
}

async fn install_from_zip(url: &str, work: &Path, artifact_name: &str) -> Result<bool, String> {
    let zip_path = work.join("tip-artifact.zip");
    let extract = work.join("artifact");
    let _ = std::fs::create_dir_all(&extract);
    let mut args: Vec<String> = vec![
        "--fail".into(),
        "--location".into(),
        "--silent".into(),
        "--show-error".into(),
        "--max-time".into(),
        "180".into(),
        "--output".into(),
        zip_path.to_string_lossy().into_owned(),
        "-H".into(),
        format!("User-Agent: {}", installer_user_agent()),
    ];
    if let Some(token) = releases_cache::github_token() {
        args.push("-H".into());
        args.push(format!("Authorization: Bearer {token}"));
    }
    args.push(url.into());
    let output = Command::new("curl")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("Artifact download failed: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        upgrade_tip_log::log_info(format!(
            "Artifact download skipped ({})",
            stderr.trim().chars().take(160).collect::<String>()
        ));
        return Ok(false);
    }
    let unzip = Command::new("unzip")
        .args(["-o", "-q"])
        .arg(&zip_path)
        .arg("-d")
        .arg(&extract)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await;
    if !unzip.map(|s| s.success()).unwrap_or(false) {
        let _ = Command::new("bsdtar")
            .args(["-xf"])
            .arg(&zip_path)
            .arg("-C")
            .arg(&extract)
            .status()
            .await;
    }
    if let Some(installer) = find_built_installer(&extract) {
        return install_bins(&installer).await.map(|_| true);
    }
    if artifact_looks_like_guest_package(artifact_name) {
        if let Some(rpm) = find_package_file(&extract, "rpm") {
            upgrade_tip_log::log_info(format!(
                "Installing tip package {}",
                rpm.file_name().and_then(|n| n.to_str()).unwrap_or("rpm")
            ));
            upgrade_pkg::install_rpm(rpm.to_str().unwrap_or(""), false, false).await?;
            return Ok(true);
        }
        if let Some(deb) = find_package_file(&extract, "deb") {
            upgrade_tip_log::log_info(format!(
                "Installing tip package {}",
                deb.file_name().and_then(|n| n.to_str()).unwrap_or("deb")
            ));
            upgrade_pkg::install_deb(deb.to_str().unwrap_or(""), false, false).await?;
            return Ok(true);
        }
    }
    upgrade_tip_log::log_info(format!(
        "Actions artifact {artifact_name} had no cpn-installer binary or guest package"
    ));
    Ok(false)
}

async fn install_bins(installer: &Path) -> Result<(), String> {
    install_binary(
        installer.to_str().unwrap_or(""),
        crate::manifest::installer_bin(),
    )
    .await?;
    let cli = installer.with_file_name("cpn");
    if cli.is_file() {
        let _ = install_binary(cli.to_str().unwrap_or(""), crate::manifest::cli_bin()).await;
    }
    let local_installer = PathBuf::from("/usr/local/bin/cpn-installer");
    if local_installer
        .parent()
        .map(|p| p.is_dir())
        .unwrap_or(false)
    {
        let _ = install_binary(
            installer.to_str().unwrap_or(""),
            local_installer.to_str().unwrap_or(""),
        )
        .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_artifact_names_match() {
        assert!(artifact_looks_like_linux_bins("cpn-linux-tip-bins"));
        assert!(artifact_looks_like_linux_bins("cpn-installer-linux"));
        assert!(!artifact_looks_like_linux_bins("cpn-windows-release"));
        assert!(!artifact_looks_like_linux_bins("cpn-rpm-el10"));
    }
}
