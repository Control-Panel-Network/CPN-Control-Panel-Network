//! Node/npm auto-heal for tip source builds (`installer-ui`).
//!
//! Vite 8 / rolldown need Node `^20.19 || >=22.12` (`styleText` needs `>=20.12`).
//! When discovery finds only Node 16/18 (or npm fails with `styleText`), download
//! Node 22 LTS under `/usr/local`, put it on PATH, and retry the UI build once.

use crate::installer::AppState;
use crate::upgrade_tip_cmd;
use crate::upgrade_tip_log;
use crate::upgrade_tip_toolchain::{
    self, MIN_NODE_MAJOR, MIN_NODE_MINOR, Toolchain, node_version_meets_min, probe_node_version,
};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

fn fail(message: impl Into<String>, retry: Option<u32>) -> String {
    let message = message.into();
    upgrade_tip_log::log_failure(&message, retry);
    message
}

/// Official LTS tarball pinned for auto-heal (linux x64 / arm64).
pub const HEAL_NODE_VERSION: &str = "22.14.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl NodeVersion {
    pub fn meets_tip_floor(self) -> bool {
        node_version_meets_min(self.major, self.minor)
    }

    fn from_tuple(parts: (u32, u32, u32)) -> Self {
        Self {
            major: parts.0,
            minor: parts.1,
            patch: parts.2,
        }
    }
}

fn is_exec(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.is_file()
            && path
                .metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn node_candidate_bins() -> Vec<PathBuf> {
    let mut out = upgrade_tip_toolchain::node_search_paths();
    let mut push = |p: PathBuf| {
        if !out.iter().any(|x| x == &p) {
            out.push(p);
        }
    };
    for base in ["/home/cpn/.nvm/versions/node", "/root/.nvm/versions/node"] {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                push(entry.path().join("bin").join("node"));
            }
        }
    }
    out
}

/// Best Node binary that already meets the tip floor, if any.
pub fn find_supported_node() -> Option<(PathBuf, NodeVersion)> {
    let mut best: Option<(PathBuf, NodeVersion)> = None;
    for candidate in node_candidate_bins() {
        if !is_exec(&candidate) {
            continue;
        }
        let Some(parts) = probe_node_version(&candidate) else {
            continue;
        };
        let ver = NodeVersion::from_tuple(parts);
        if !ver.meets_tip_floor() {
            continue;
        }
        let replace = match &best {
            None => true,
            Some((_, prev)) => ver > *prev,
        };
        if replace {
            best = Some((candidate, ver));
        }
    }
    best
}

fn npm_beside(node: &Path) -> Option<PathBuf> {
    let bin = node.parent()?;
    let npm = bin.join("npm");
    if is_exec(&npm) { Some(npm) } else { None }
}

/// Put a good Node ahead of distro `/usr/bin` so `#!/usr/bin/env node` and npm
/// do not pick Node 16 when cargo's parent dir was prepended.
pub fn apply_node_to_toolchain(tools: &mut Toolchain, node: &Path, npm: Option<PathBuf>) {
    if let Some(bin) = node.parent() {
        upgrade_tip_toolchain::prepend_path_front(bin, &mut tools.env);
    }
    tools.node = Some(node.to_path_buf());
    if let Some(npm) = npm.or_else(|| npm_beside(node)) {
        tools.npm = Some(npm);
    }
}

fn linux_arch_suffix() -> Option<&'static str> {
    let output = std::process::Command::new("uname")
        .arg("-m")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match arch.as_str() {
        "x86_64" | "amd64" => Some("x64"),
        "aarch64" | "arm64" => Some("arm64"),
        _ => None,
    }
}

async fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--max-time",
            "240",
            "--output",
        ])
        .arg(dest)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("Node download failed to start: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Node download failed ({})",
            stderr.trim().chars().take(180).collect::<String>()
        ));
    }
    Ok(())
}

/// Install Node 22 LTS under `/usr/local` and return its `node` path.
pub async fn install_node_lts() -> Result<(PathBuf, NodeVersion), String> {
    let arch = linux_arch_suffix().ok_or_else(|| {
        "Cannot auto-install Node: unsupported CPU arch (need x86_64 or aarch64)".to_string()
    })?;
    let ver = HEAL_NODE_VERSION;
    let folder = format!("node-v{ver}-linux-{arch}");
    let dest_root = PathBuf::from(format!("/usr/local/{folder}"));
    let node_bin = dest_root.join("bin").join("node");
    if is_exec(&node_bin)
        && let Some(parts) = probe_node_version(&node_bin)
    {
        let found = NodeVersion::from_tuple(parts);
        if found.meets_tip_floor() {
            upgrade_tip_log::log_info(format!(
                "Reusing existing Node at {} (v{}.{}.{})",
                node_bin.display(),
                found.major,
                found.minor,
                found.patch
            ));
            return Ok((node_bin, found));
        }
    }

    let tarball = PathBuf::from(format!("/var/tmp/cpn-node-{ver}-{arch}.tar.xz"));
    let url = format!("https://nodejs.org/dist/v{ver}/{folder}.tar.xz");
    upgrade_tip_log::log_info(format!(
        "Auto-healing tip Node: downloading {url} (retry=1)"
    ));
    download_file(&url, &tarball).await?;
    let _ = std::fs::remove_dir_all(&dest_root);
    let status = Command::new("tar")
        .args(["-xJf"])
        .arg(&tarball)
        .arg("-C")
        .arg("/usr/local")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|error| format!("Node extract failed: {error}"))?;
    let _ = std::fs::remove_file(&tarball);
    if !status.success() {
        return Err("Node extract (tar -xJf) failed".into());
    }
    if !is_exec(&node_bin) {
        return Err(format!(
            "Node heal finished but {} is missing",
            node_bin.display()
        ));
    }
    let parts = probe_node_version(&node_bin).ok_or_else(|| {
        format!(
            "Installed Node at {} but could not read its version",
            node_bin.display()
        )
    })?;
    let ver = NodeVersion::from_tuple(parts);
    if !ver.meets_tip_floor() {
        return Err(format!(
            "Installed Node v{}.{}.{} is still below the tip floor (need >={}.{})",
            ver.major, ver.minor, ver.patch, MIN_NODE_MAJOR, MIN_NODE_MINOR
        ));
    }
    // Best-effort convenience symlinks; tip jobs use PATH from apply_node_to_toolchain.
    for name in ["node", "npm", "npx"] {
        let target = dest_root.join("bin").join(name);
        let link = PathBuf::from("/usr/local/bin").join(name);
        if target.is_file() {
            let _ = std::fs::remove_file(&link);
            #[cfg(unix)]
            {
                let _ = std::os::unix::fs::symlink(&target, &link);
            }
        }
    }
    upgrade_tip_log::log_info(format!(
        "Node auto-heal installed {} (v{}.{}.{})",
        node_bin.display(),
        ver.major,
        ver.minor,
        ver.patch
    ));
    Ok((node_bin, ver))
}

/// Ensure toolchain PATH/npm use a supported Node; install Node 22 LTS when needed.
pub async fn ensure_supported_node(tools: &mut Toolchain) -> Result<NodeVersion, String> {
    if let Some(node) = tools.node.clone()
        && let Some(parts) = probe_node_version(&node)
    {
        let ver = NodeVersion::from_tuple(parts);
        if ver.meets_tip_floor() {
            upgrade_tip_log::log_info(format!(
                "Tip Node OK: {} (v{}.{}.{})",
                node.display(),
                ver.major,
                ver.minor,
                ver.patch
            ));
            apply_node_to_toolchain(tools, &node, tools.npm.clone());
            return Ok(ver);
        }
    }
    if let Some((node, ver)) = find_supported_node() {
        upgrade_tip_log::log_info(format!(
            "Tip Node OK: {} (v{}.{}.{})",
            node.display(),
            ver.major,
            ver.minor,
            ver.patch
        ));
        apply_node_to_toolchain(tools, &node, npm_beside(&node));
        return Ok(ver);
    }
    upgrade_tip_log::log_failure(
        format!(
            "No Node >={MIN_NODE_MAJOR}.{MIN_NODE_MINOR} for installer-ui; auto-installing Node {HEAL_NODE_VERSION}"
        ),
        Some(1),
    );
    let (node, ver) = install_node_lts().await?;
    apply_node_to_toolchain(tools, &node, npm_beside(&node));
    Ok(ver)
}

pub fn looks_like_node_engine_failure(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("styletext")
        || lower.contains("does not provide an export named")
        || lower.contains("ebadengine")
        || lower.contains("unsupported engine")
        || lower.contains("node.js v16")
        || lower.contains("node.js v18")
        || (lower.contains("syntaxerror") && lower.contains("node:util"))
}

async fn run_installer_ui_npm(tools: &Toolchain, ui: &Path) -> Result<(), String> {
    let Some(npm) = tools.npm.as_ref() else {
        return Err(fail(
            "installer-ui needs npm (not on PATH or common locations) and dist/index.html is missing",
            None,
        ));
    };
    if let Some(node) = tools.node.as_ref() {
        upgrade_tip_log::log_info(format!(
            "installer-ui Node {} with npm {}",
            node.display(),
            npm.display()
        ));
    }
    let npm_s = npm.to_string_lossy().into_owned();
    upgrade_tip_cmd::run_checked(&npm_s, &["ci"], ui, &tools.env).await?;
    upgrade_tip_cmd::run_checked(&npm_s, &["run", "build"], ui, &tools.env).await?;
    Ok(())
}

/// Build `installer-ui` for tip source compiles; auto-heal Node and retry once.
pub async fn build_installer_ui(
    state: &AppState,
    root: &Path,
    tools: &mut Toolchain,
) -> Result<(), String> {
    let ui = root.join("installer-ui");
    let dist_index = ui.join("dist").join("index.html");
    if dist_index.is_file() {
        return Ok(());
    }
    state
        .progress("installing", 38, "Checking Node for installer UI")
        .await;
    ensure_supported_node(tools)
        .await
        .map_err(|error| fail(error, Some(1)))?;
    if let Err(error) = upgrade_tip_toolchain::require_node_for_installer_ui(tools) {
        return Err(fail(error, Some(1)));
    }
    state
        .progress("installing", 40, "Building installer UI (npm)")
        .await;
    match run_installer_ui_npm(tools, &ui).await {
        Ok(()) => {}
        Err(first) => {
            if looks_like_node_engine_failure(&first) {
                upgrade_tip_log::log_failure(
                    format!(
                        "installer-ui npm failed on Node engine ({first}); auto-healing Node and retrying"
                    ),
                    Some(1),
                );
                state
                    .progress("installing", 39, "Auto-installing Node for tip UI build")
                    .await;
                let (node, ver) = install_node_lts()
                    .await
                    .map_err(|error| fail(error, Some(1)))?;
                apply_node_to_toolchain(tools, &node, None);
                upgrade_tip_log::log_info(format!(
                    "Node auto-heal complete (v{}.{}.{}); retrying npm run build",
                    ver.major, ver.minor, ver.patch
                ));
                state
                    .progress(
                        "installing",
                        40,
                        "Retrying installer UI (npm) after Node heal",
                    )
                    .await;
                run_installer_ui_npm(tools, &ui).await?;
            } else {
                return Err(first);
            }
        }
    }
    if !dist_index.is_file() {
        return Err(fail(
            "installer-ui build did not produce dist/index.html",
            None,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tip_floor_rejects_old_node() {
        assert!(
            !NodeVersion {
                major: 16,
                minor: 20,
                patch: 2
            }
            .meets_tip_floor()
        );
        assert!(
            !NodeVersion {
                major: 18,
                minor: 20,
                patch: 0
            }
            .meets_tip_floor()
        );
        assert!(
            !NodeVersion {
                major: 20,
                minor: 11,
                patch: 1
            }
            .meets_tip_floor()
        );
        assert!(
            NodeVersion {
                major: 20,
                minor: 19,
                patch: 0
            }
            .meets_tip_floor()
        );
        assert!(
            NodeVersion {
                major: 22,
                minor: 14,
                patch: 0
            }
            .meets_tip_floor()
        );
    }

    #[test]
    fn classifies_styletext_failure() {
        assert!(looks_like_node_engine_failure(
            "SyntaxError: The requested module 'node:util' does not provide an export named 'styleText'"
        ));
        assert!(looks_like_node_engine_failure(
            "EBADENGINE Unsupported engine"
        ));
        assert!(!looks_like_node_engine_failure("cargo build failed OOM"));
    }

    #[test]
    fn parse_helper_matches_toolchain() {
        assert_eq!(
            upgrade_tip_toolchain::parse_node_version("v22.14.0\n"),
            Some((22, 14, 0))
        );
    }
}
