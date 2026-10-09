//! Per-commit cargo target dir plus post-build SHA check for tip upgrades.

use crate::build_meta::{self, sha_equal, short_sha};
use crate::upgrade_tip_log;
use crate::upgrade_tip_toolchain::Toolchain;
use std::path::{Path, PathBuf};

pub fn tip_cargo_target_dir(git_sha: &str) -> PathBuf {
    let short = short_sha(git_sha);
    let leaf = if short.is_empty() {
        "tip-unknown".to_string()
    } else {
        format!("tip-{short}")
    };
    if Path::new("/home/cpn").is_dir() {
        PathBuf::from("/home/cpn/cpn-cargo-target").join(leaf)
    } else {
        PathBuf::from("/var/tmp/cpn-cargo-target").join(leaf)
    }
}

pub fn apply_tip_build_env(tools: &mut Toolchain, git_sha: &str) {
    let target = tip_cargo_target_dir(git_sha);
    let _ = std::fs::create_dir_all(&target);
    tools
        .env
        .retain(|(key, _)| key != "CARGO_TARGET_DIR" && key != "CARGO_INCREMENTAL");
    tools
        .env
        .push(("CARGO_TARGET_DIR".into(), target.display().to_string()));
    tools.env.push(("CARGO_INCREMENTAL".into(), "0".into()));
    tools
        .env
        .retain(|(key, _)| key != "CPN_GIT_SHA" && key != "CPN_BUILD_SHA");
    tools.env.push(("CPN_GIT_SHA".into(), git_sha.to_string()));
    tools
        .env
        .push(("CPN_BUILD_SHA".into(), git_sha.to_string()));
}

pub fn write_tip_sha_file(root: &Path, git_sha: &str) -> Result<(), String> {
    let path = root.join(".cpn-git-sha");
    std::fs::write(&path, format!("{}\n", git_sha.trim()))
        .map_err(|error| format!("Could not write .cpn-git-sha: {error}"))?;
    Ok(())
}

/// Ensure extracted source compiles a strings-visible `CPN_BUILD_SHA=` keep-static.
/// Older commit tarballs may omit it; LTO can also drop an unused concat.
pub fn inject_keep_module(root: &Path, git_sha: &str) -> Result<(), String> {
    let sha = git_sha.trim();
    if !crate::build_meta::looks_like_git_sha(sha) {
        return Err("Cannot inject SHA keep module: value is not a git SHA".into());
    }
    let src = root.join("src");
    if !src.is_dir() {
        return Ok(());
    }
    let keep_path = src.join("cpn_build_sha_keep.rs");
    // Build the marker prefix at runtime: a literal `CPN_BUILD_SHA=` inside this
    // format string ends up in .rodata of the *built* panel as a decoy prefix
    // (followed by fmt boundary bytes), which older readers mistook for the marker.
    let prefix = String::from_utf8(crate::build_meta::marker_needle())
        .map_err(|_| "Marker prefix is not valid UTF-8".to_string())?;
    let body = format!(
        "//! SHA keep-static for commit upgrades.\n\
#[used]\n\
#[allow(dead_code)]\n\
static CPN_BUILD_SHA_KEEP: &[u8] = b\"{prefix}{sha}\\0\";\n"
    );
    std::fs::write(&keep_path, body)
        .map_err(|error| format!("Could not write SHA keep module: {error}"))?;
    let lib_path = src.join("lib.rs");
    if lib_path.is_file() {
        let lib = std::fs::read_to_string(&lib_path).unwrap_or_default();
        if !lib.contains("mod cpn_build_sha_keep") {
            let mut next = lib;
            if !next.ends_with('\n') {
                next.push('\n');
            }
            next.push_str("mod cpn_build_sha_keep;\n");
            std::fs::write(&lib_path, next)
                .map_err(|error| format!("Could not patch lib.rs for SHA keep module: {error}"))?;
        }
    }
    Ok(())
}

pub fn stamp_built_installer(path: &Path, git_sha: &str) -> Result<(), String> {
    crate::build_meta::stamp_sha_marker_if_missing(path, git_sha).inspect_err(|error| {
        upgrade_tip_log::log_failure(error, None);
    })
}

fn installer_candidates(root: &Path, tools: &Toolchain) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for (key, value) in &tools.env {
        if key == "CARGO_TARGET_DIR" {
            candidates.push(PathBuf::from(value).join("release/cpn-installer"));
        }
    }
    candidates.push(root.join("target/release/cpn-installer"));
    candidates
}

pub fn find_built_installer(root: &Path, tools: &Toolchain) -> Option<PathBuf> {
    installer_candidates(root, tools)
        .into_iter()
        .find(|path| path.is_file())
}

pub fn require_built_sha(path: &Path, expected_sha: &str) -> Result<(), String> {
    let found = build_meta::sha_embedded_in_binary(path).unwrap_or_default();
    if found.is_empty() {
        let msg = format!(
            "Commit build at {} has no readable CPN_BUILD_SHA marker. Refusing to install a binary that cannot prove it is {}. Retry the commit upgrade; if it repeats, report the panel log.",
            path.display(),
            short_sha(expected_sha)
        );
        upgrade_tip_log::log_failure(&msg, None);
        return Err(msg);
    }
    if !sha_equal(&found, expected_sha) {
        let msg = format!(
            "Commit build SHA {} does not match requested {}. Shared cargo cache was not used for this commit. Rebuild required.",
            short_sha(&found),
            short_sha(expected_sha)
        );
        upgrade_tip_log::log_failure(&msg, None);
        return Err(msg);
    }
    upgrade_tip_log::log_info(format!(
        "Commit binary SHA ok {} ({})",
        short_sha(expected_sha),
        path.display()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_commit_target_includes_short_sha() {
        let dir = tip_cargo_target_dir("c27a4a2aba629238593ae13907a8b5e66bb1f58c");
        let text = dir.display().to_string().replace('\\', "/");
        assert!(text.contains("tip-c27a4a2"), "got {text}");
        assert!(!text.ends_with("/cpn-cargo-target"));
    }

    #[test]
    fn env_overrides_shared_target() {
        let mut tools = Toolchain {
            cargo: PathBuf::from("/home/cpn/.cargo/bin/cargo"),
            npm: None,
            env: vec![(
                "CARGO_TARGET_DIR".into(),
                "/home/cpn/cpn-cargo-target".into(),
            )],
        };
        apply_tip_build_env(&mut tools, "c27a4a2aba629238593ae13907a8b5e66bb1f58c");
        let target = tools
            .env
            .iter()
            .find(|(k, _)| k == "CARGO_TARGET_DIR")
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        assert!(target.contains("tip-c27a4a2"), "got {target}");
        assert!(
            tools
                .env
                .iter()
                .any(|(k, v)| k == "CARGO_INCREMENTAL" && v == "0")
        );
        assert!(
            tools
                .env
                .iter()
                .any(|(k, v)| k == "CPN_GIT_SHA" && v.starts_with("c27a4a2"))
        );
        assert!(
            tools
                .env
                .iter()
                .any(|(k, v)| k == "CPN_BUILD_SHA" && v.starts_with("c27a4a2"))
        );
    }

    #[test]
    fn injects_keep_module_into_fake_crate() {
        let dir = std::env::temp_dir().join(format!("cpn-keep-{}", std::process::id()));
        let src = dir.join("src");
        let _ = std::fs::create_dir_all(&src);
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.1.0\"\n",
        )
        .expect("toml");
        std::fs::write(src.join("lib.rs"), "//! crate\n").expect("lib");
        inject_keep_module(&dir, "c27a4a2aba629238593ae13907a8b5e66bb1f58c").expect("inject");
        let keep = std::fs::read_to_string(src.join("cpn_build_sha_keep.rs")).expect("keep");
        assert!(keep.contains("CPN_BUILD_SHA=c27a4a2aba629238593ae13907a8b5e66bb1f58c"));
        let lib = std::fs::read_to_string(src.join("lib.rs")).expect("lib2");
        assert!(lib.contains("mod cpn_build_sha_keep;"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
