//! Upgrade / repair from a git branch tip or commit SHA (source build).

use crate::installer::AppState;
use crate::manifest::{self, ManifestSource, cli_bin, installer_bin};
use crate::releases_cache;
use crate::releases_stable_tip::{self, StableTipInfo};
use crate::upgrade_pkg::install_binary;
use rand::{Rng, distr::Alphanumeric};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

pub struct TipApplyResult {
    pub sha: String,
    pub short_sha: String,
    pub branch_label: String,
    pub package_version: String,
    pub source: ManifestSource,
}

fn ephemeral_dir(prefix: &str) -> Result<PathBuf, String> {
    let suffix: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    let dir = PathBuf::from(format!("/var/tmp/{prefix}-{suffix}"));
    std::fs::create_dir_all(&dir).map_err(|error| format!("Could not create temp dir: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

fn require_cmd(name: &str) -> Result<(), String> {
    match std::process::Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        _ => Err(format!(
            "Commit/tip upgrade requires `{name}` on PATH (lab/source hosts). Use a published Release when the toolchain is not installed."
        )),
    }
}

async fn download_tarball(repo: &str, git_ref: &str, dest: &Path) -> Result<(), String> {
    let url = format!("https://api.github.com/repos/{repo}/tarball/{git_ref}");
    let mut args: Vec<String> = vec![
        "--fail".into(),
        "--location".into(),
        "--silent".into(),
        "--show-error".into(),
        "--max-time".into(),
        "180".into(),
        "--output".into(),
        dest.to_string_lossy().into_owned(),
        "-H".into(),
        "Accept: application/vnd.github+json".into(),
        "-H".into(),
        format!("User-Agent: CPN-Installer/{}", env!("CARGO_PKG_VERSION")),
    ];
    if let Some(token) = releases_cache::github_token() {
        args.push("-H".into());
        args.push(format!("Authorization: Bearer {token}"));
    }
    args.push(url);
    let output = Command::new("curl")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("Download tip tarball failed: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Download tip tarball failed ({})",
            stderr.trim().chars().take(200).collect::<String>()
        ));
    }
    Ok(())
}

fn find_extracted_root(parent: &Path) -> Result<PathBuf, String> {
    let entries = std::fs::read_dir(parent)
        .map_err(|error| format!("Could not list extract dir: {error}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("Cargo.toml").is_file() {
            return Ok(path);
        }
    }
    Err("Extracted tip archive did not contain a Cargo.toml root".into())
}

fn read_cargo_version(root: &Path) -> String {
    let path = root.join("Cargo.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return env!("CARGO_PKG_VERSION").to_string();
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("version") {
            let rest = rest.trim().trim_start_matches('=').trim();
            let ver = rest.trim_matches('"').trim_matches('\'').trim();
            if !ver.is_empty() {
                return ver.to_string();
            }
        }
    }
    env!("CARGO_PKG_VERSION").to_string()
}

async fn cargo_build_release(state: &AppState, root: &Path, git_sha: &str) -> Result<(), String> {
    state
        .progress("installing", 45, "Building panel from stable tip (cargo)")
        .await;
    let output = Command::new("cargo")
        .args(["build", "--release", "--locked"])
        .env("CPN_GIT_SHA", git_sha)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("cargo build failed to start: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "cargo build --release failed: {}",
            stderr.trim().chars().take(400).collect::<String>()
        ));
    }
    Ok(())
}

pub async fn apply_tip_ref(
    state: &AppState,
    repo: &str,
    git_ref: &str,
) -> Result<TipApplyResult, String> {
    require_cmd("curl")?;
    require_cmd("tar")?;
    require_cmd("cargo")?;

    state
        .progress("downloading", 10, format!("Resolving tip ref {git_ref}"))
        .await;
    let sha = releases_stable_tip::resolve_ref_sha(repo, git_ref).await?;
    let short = crate::build_meta::short_sha(&sha);
    let branch = releases_stable_tip::stable_branch();
    let label = if git_ref == branch || git_ref.eq_ignore_ascii_case("stable") {
        format!("{branch} @ {short}")
    } else {
        format!("commit @ {short}")
    };

    let work = ephemeral_dir("cpn-tip")?;
    let tarball = work.join("tip.tar.gz");
    state
        .progress(
            "downloading",
            20,
            format!("Downloading source tarball ({label})"),
        )
        .await;
    download_tarball(repo, &sha, &tarball).await?;

    state
        .progress("installing", 35, "Extracting tip source")
        .await;
    let extract_status = Command::new("tar")
        .args(["-xzf", tarball.to_str().unwrap_or("tip.tar.gz"), "-C"])
        .arg(&work)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|error| format!("tar extract failed: {error}"))?;
    if !extract_status.success() {
        let _ = std::fs::remove_dir_all(&work);
        return Err("tar extract of tip archive failed".into());
    }
    let root = find_extracted_root(&work)?;
    let package_version = read_cargo_version(&root);
    cargo_build_release(state, &root, &sha).await?;

    let built_installer = root.join("target/release/cpn-installer");
    let built_cli = root.join("target/release/cpn");
    if !built_installer.is_file() {
        let _ = std::fs::remove_dir_all(&work);
        return Err("tip build missing target/release/cpn-installer".into());
    }

    state
        .progress("installing", 75, format!("Installing binaries ({label})"))
        .await;
    install_binary(
        built_installer.to_str().unwrap_or(""),
        installer_bin(),
    )
    .await?;
    if built_cli.is_file() {
        let _ = install_binary(built_cli.to_str().unwrap_or(""), cli_bin()).await;
    }

    let local_installer = PathBuf::from("/usr/local/bin/cpn-installer");
    let local_cli = PathBuf::from("/usr/local/bin/cpn");
    if local_installer
        .parent()
        .map(|p| p.is_dir())
        .unwrap_or(false)
    {
        let _ = install_binary(
            built_installer.to_str().unwrap_or(""),
            local_installer.to_str().unwrap_or(""),
        )
        .await;
        if built_cli.is_file() {
            let _ = install_binary(
                built_cli.to_str().unwrap_or(""),
                local_cli.to_str().unwrap_or(""),
            )
            .await;
        }
    }

    let _ = std::fs::remove_dir_all(&work);
    Ok(TipApplyResult {
        sha,
        short_sha: short,
        branch_label: label,
        package_version,
        source: ManifestSource::Local,
    })
}

pub async fn tip_info_for_confirm(repo: &str) -> StableTipInfo {
    releases_stable_tip::fetch_stable_tip(repo).await
}

pub fn record_tip_install(
    result: &TipApplyResult,
    selected_server: Option<crate::model::ServerEngine>,
    selected_mail: Option<crate::model::MailSystem>,
) -> Result<(), String> {
    let tag = format!("stable@{}", result.short_sha);
    manifest::record_install_with_commit(
        &result.package_version,
        &tag,
        result.source.clone(),
        Some(result.sha.as_str()),
        selected_server,
        selected_mail,
    )?;
    Ok(())
}
