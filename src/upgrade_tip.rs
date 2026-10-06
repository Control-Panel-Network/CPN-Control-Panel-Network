//! Upgrade / repair from a git branch tip or commit SHA (source or prebuilt).

use crate::installer::AppState;
use crate::manifest::{self, ManifestSource, cli_bin, installer_bin};
use crate::releases_cache;
use crate::releases_stable_tip::{self, StableTipInfo};
use crate::upgrade_pkg::install_binary;
use crate::upgrade_tip_artifacts;
use crate::upgrade_tip_log;
use crate::upgrade_tip_toolchain::{self, Toolchain};
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

fn fail(message: impl Into<String>, retry: Option<u32>) -> String {
    let message = message.into();
    upgrade_tip_log::log_failure(&message, retry);
    message
}

async fn require_cmd(name: &str) -> Result<(), String> {
    match Command::new(name)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
    {
        Ok(status) if status.success() => Ok(()),
        other => {
            let detail = match other {
                Ok(status) => format!("exit {status}"),
                Err(error) => error.to_string(),
            };
            Err(fail(
                format!("Tip upgrade requires `{name}` ({detail})"),
                None,
            ))
        }
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
        .map_err(|error| fail(format!("Download tip tarball failed: {error}"), None))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(fail(
            format!(
                "Download tip tarball failed ({})",
                stderr.trim().chars().take(200).collect::<String>()
            ),
            None,
        ));
    }
    Ok(())
}

fn find_extracted_root(parent: &Path) -> Result<PathBuf, String> {
    let entries = std::fs::read_dir(parent)
        .map_err(|error| fail(format!("Could not list extract dir: {error}"), None))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("Cargo.toml").is_file() {
            return Ok(path);
        }
    }
    Err(fail(
        "Extracted tip archive did not contain a Cargo.toml root",
        None,
    ))
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

async fn run_checked(
    program: &str,
    args: &[&str],
    cwd: &Path,
    env: &[(String, String)],
) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let output = cmd
        .output()
        .await
        .map_err(|error| fail(format!("{program} failed to start: {error}"), None))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        return Err(fail(
            format!(
                "{program} {} failed: {}",
                args.join(" "),
                detail.chars().take(600).collect::<String>()
            ),
            None,
        ));
    }
    Ok(())
}

async fn build_installer_ui(
    state: &AppState,
    root: &Path,
    tools: &Toolchain,
) -> Result<(), String> {
    let ui = root.join("installer-ui");
    let dist_index = ui.join("dist").join("index.html");
    if dist_index.is_file() {
        return Ok(());
    }
    let Some(npm) = tools.npm.as_ref() else {
        return Err(fail(
            "installer-ui needs npm (not on PATH or common locations) and dist/index.html is missing",
            None,
        ));
    };
    state
        .progress("installing", 40, "Building installer UI (npm)")
        .await;
    let npm_s = npm.to_string_lossy().into_owned();
    run_checked(&npm_s, &["ci"], &ui, &tools.env).await?;
    run_checked(&npm_s, &["run", "build"], &ui, &tools.env).await?;
    if !dist_index.is_file() {
        return Err(fail(
            "installer-ui build did not produce dist/index.html",
            None,
        ));
    }
    Ok(())
}

async fn cargo_build_release(
    state: &AppState,
    root: &Path,
    git_sha: &str,
    tools: &Toolchain,
) -> Result<(), String> {
    build_installer_ui(state, root, tools).await?;
    state
        .progress("installing", 45, "Building panel from stable tip (cargo)")
        .await;
    let cargo = tools.cargo.to_string_lossy().into_owned();
    let mut env = tools.env.clone();
    env.push(("CPN_GIT_SHA".into(), git_sha.to_string()));
    run_checked(&cargo, &["build", "--release", "--locked"], root, &env).await?;
    Ok(())
}

async fn heal_rustup(state: &AppState) -> Result<(), String> {
    require_cmd("curl").await?;
    let home = upgrade_tip_toolchain::rustup_home_dir();
    let script = PathBuf::from("/var/tmp/cpn-rustup-init.sh");
    state
        .progress("installing", 8, "Installing Rust toolchain (rustup)")
        .await;
    upgrade_tip_log::log_info("Installing rustup for tip upgrade (retry=1)");
    let url = "https://sh.rustup.rs";
    let output = Command::new("curl")
        .args([
            "--proto",
            "=https",
            "--tlsv1.2",
            "-sSf",
            "--max-time",
            "120",
            "-o",
        ])
        .arg(&script)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| fail(format!("rustup-init download failed: {error}"), Some(1)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(fail(
            format!(
                "rustup-init download failed ({})",
                stderr.trim().chars().take(200).collect::<String>()
            ),
            Some(1),
        ));
    }
    let cargo_home = home.join(".cargo");
    let rustup_home = home.join(".rustup");
    let status = Command::new("sh")
        .arg(&script)
        .args(["-y", "--default-toolchain", "stable"])
        .env("HOME", &home)
        .env("CARGO_HOME", &cargo_home)
        .env("RUSTUP_HOME", &rustup_home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|error| fail(format!("rustup-init failed to start: {error}"), Some(1)))?;
    let _ = std::fs::remove_file(&script);
    if !status.success() {
        return Err(fail("rustup-init did not complete successfully", Some(1)));
    }
    Ok(())
}

async fn ensure_toolchain(state: &AppState) -> Result<Toolchain, String> {
    if let Some(found) = upgrade_tip_toolchain::discover() {
        upgrade_tip_log::log_info(format!("Using cargo at {}", found.cargo.display()));
        return Ok(found);
    }
    upgrade_tip_log::log_failure("cargo not on PATH; attempting rustup heal", Some(1));
    heal_rustup(state).await?;
    upgrade_tip_toolchain::discover()
        .ok_or_else(|| fail(upgrade_tip_toolchain::missing_cargo_message(), Some(1)))
}

fn maybe_cleanup_build_trees(root: &Path) {
    let script = root.join("scripts/cleanup-old-build-trees.sh");
    if !script.is_file() {
        return;
    }
    let _ = std::process::Command::new("bash")
        .arg(&script)
        .arg("--keep-count")
        .arg("1")
        .status();
}

async fn install_built_bins(root: &Path, tools: &Toolchain, git_sha: &str) -> Result<(), String> {
    let Some(built_installer) = crate::upgrade_tip_verify::find_built_installer(root, tools) else {
        return Err(fail("tip build missing target/release/cpn-installer", None));
    };
    crate::upgrade_tip_verify::require_built_sha(&built_installer, git_sha)
        .map_err(|error| fail(error, None))?;
    let cli = built_installer.with_file_name("cpn");
    install_from_paths(&built_installer, &cli).await
}

async fn install_from_paths(installer: &Path, cli: &Path) -> Result<(), String> {
    install_binary(installer.to_str().unwrap_or(""), installer_bin())
        .await
        .map_err(|error| fail(error, None))?;
    if cli.is_file() {
        let _ = install_binary(cli.to_str().unwrap_or(""), cli_bin()).await;
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
        if cli.is_file() {
            let _ = install_binary(cli.to_str().unwrap_or(""), "/usr/local/bin/cpn").await;
        }
    }
    Ok(())
}

pub async fn apply_tip_ref(
    state: &AppState,
    repo: &str,
    git_ref: &str,
) -> Result<TipApplyResult, String> {
    require_cmd("curl").await?;
    require_cmd("tar").await?;
    upgrade_tip_log::log_info(format!("Tip upgrade start repo={repo} ref={git_ref}"));

    state
        .progress("downloading", 10, format!("Resolving tip ref {git_ref}"))
        .await;
    let sha = releases_stable_tip::resolve_ref_sha(repo, git_ref)
        .await
        .map_err(|error| fail(error, None))?;
    let short = crate::build_meta::short_sha(&sha);
    let branch = releases_stable_tip::stable_branch();
    let label = if git_ref == branch || git_ref.eq_ignore_ascii_case("stable") {
        format!("{branch} @ {short}")
    } else {
        format!("commit @ {short}")
    };
    upgrade_tip_log::log_info(format!("Resolved tip {label} sha={short}"));

    if let Some(on_disk) =
        crate::build_meta::sha_embedded_in_binary(std::path::Path::new(installer_bin()))
        && crate::build_meta::sha_equal(&on_disk, &sha)
    {
        upgrade_tip_log::log_info(format!(
            "Installed binary already matches tip {short}; skipping source rebuild"
        ));
        return Ok(TipApplyResult {
            sha,
            short_sha: short,
            branch_label: label,
            package_version: env!("CARGO_PKG_VERSION").to_string(),
            source: ManifestSource::Local,
        });
    }

    let work = ephemeral_dir("cpn-tip")?;
    state
        .progress("downloading", 15, "Checking GitHub Actions tip binaries")
        .await;
    match upgrade_tip_artifacts::try_apply_actions_artifacts(repo, &sha, &work).await {
        Ok(true) => {
            if crate::upgrade_tip_verify::require_built_sha(
                std::path::Path::new(installer_bin()),
                &sha,
            )
            .is_ok()
            {
                upgrade_tip_log::log_info(format!("Installed prebuilt tip binaries for {label}"));
                let _ = std::fs::remove_dir_all(&work);
                return Ok(TipApplyResult {
                    sha,
                    short_sha: short,
                    branch_label: label,
                    package_version: env!("CARGO_PKG_VERSION").to_string(),
                    source: ManifestSource::Local,
                });
            }
            upgrade_tip_log::log_info(
                "Prebuilt tip SHA did not match; building from source instead",
            );
        }
        Ok(false) => {
            upgrade_tip_log::log_info("No GitHub Actions tip binaries; building from source");
        }
        Err(error) => {
            upgrade_tip_log::log_failure(&error, None);
            state.log(error.clone(), "error");
        }
    }

    let mut tools = ensure_toolchain(state).await?;
    crate::upgrade_tip_verify::apply_tip_build_env(&mut tools, &sha);

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
        .map_err(|error| fail(format!("tar extract failed: {error}"), None))?;
    if !extract_status.success() {
        let _ = std::fs::remove_dir_all(&work);
        return Err(fail("tar extract of tip archive failed", None));
    }
    let root = find_extracted_root(&work)?;
    maybe_cleanup_build_trees(&root);
    crate::upgrade_tip_verify::write_tip_sha_file(&root, &sha)?;
    let package_version = read_cargo_version(&root);
    cargo_build_release(state, &root, &sha, &tools).await?;
    install_built_bins(&root, &tools, &sha).await?;

    let _ = std::fs::remove_dir_all(&work);
    upgrade_tip_log::log_info(format!(
        "Installed {package_version} from source tip {label}"
    ));
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

#[cfg(test)]
mod tests {
    #[test]
    fn missing_cargo_copy_stays_commit_capable() {
        let msg = crate::upgrade_tip_toolchain::missing_cargo_message();
        assert!(msg.contains("retry"));
        assert!(!msg.to_ascii_lowercase().contains("use a published release"));
    }
}
