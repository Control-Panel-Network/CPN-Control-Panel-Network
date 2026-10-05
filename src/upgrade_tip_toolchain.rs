//! Locate cargo/npm outside systemd PATH (rustup, /home/cpn, /usr/local).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Toolchain {
    pub cargo: PathBuf,
    pub npm: Option<PathBuf>,
    pub env: Vec<(String, String)>,
}

#[cfg(unix)]
fn is_exec(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && path
            .metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_exec(path: &Path) -> bool {
    path.is_file()
}

fn which_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_exec(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub fn cargo_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(found) = which_in_path("cargo") {
        paths.push(found);
    }
    for raw in [
        "/home/cpn/.cargo/bin/cargo",
        "/root/.cargo/bin/cargo",
        "/usr/local/cargo/bin/cargo",
        "/usr/local/bin/cargo",
        "/opt/cargo/bin/cargo",
    ] {
        let path = PathBuf::from(raw);
        if !paths.iter().any(|p| p == &path) {
            paths.push(path);
        }
    }
    paths
}

pub fn npm_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(found) = which_in_path("npm") {
        paths.push(found);
    }
    for raw in [
        "/usr/bin/npm",
        "/usr/local/bin/npm",
        "/home/cpn/.local/bin/npm",
        "/root/.local/bin/npm",
    ] {
        let path = PathBuf::from(raw);
        if !paths.iter().any(|p| p == &path) {
            paths.push(path);
        }
    }
    paths
}

fn first_exec(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| is_exec(p)).cloned()
}

fn prepend_path(dir: &Path, env: &mut Vec<(String, String)>) {
    let mut value = dir.display().to_string();
    if let Ok(existing) = std::env::var("PATH") {
        value.push(':');
        value.push_str(&existing);
    }
    env.retain(|(k, _)| k != "PATH");
    env.push(("PATH".into(), value));
}

fn env_for_cargo(cargo: &Path) -> Vec<(String, String)> {
    let mut env = vec![("CARGO_TERM_COLOR".into(), "never".into())];
    if let Some(bin) = cargo.parent() {
        prepend_path(bin, &mut env);
        let bin_s = bin.to_string_lossy();
        if bin_s.ends_with(".cargo/bin")
            && let Some(cargo_home) = bin.parent()
        {
            env.push(("CARGO_HOME".into(), cargo_home.display().to_string()));
            if let Some(user_home) = cargo_home.parent() {
                env.push(("HOME".into(), user_home.display().to_string()));
                let rustup = user_home.join(".rustup");
                if rustup.is_dir() {
                    env.push(("RUSTUP_HOME".into(), rustup.display().to_string()));
                }
            }
        }
    }
    let target = PathBuf::from("/home/cpn/cpn-cargo-target");
    if Path::new("/home/cpn").is_dir() {
        env.push(("CARGO_TARGET_DIR".into(), target.display().to_string()));
    }
    env
}

pub fn discover() -> Option<Toolchain> {
    let cargo = first_exec(&cargo_search_paths())?;
    let npm = first_exec(&npm_search_paths());
    let env = env_for_cargo(&cargo);
    Some(Toolchain { cargo, npm, env })
}

pub fn missing_cargo_message() -> String {
    "Could not find `cargo` after searching PATH, rustup homes, /home/cpn/.cargo/bin, /root/.cargo/bin, and /usr/local. GitHub Actions artifacts for this commit were not available. Install the Rust toolchain (rustup) on this host, or retry after CI tip binaries exist.".into()
}

pub fn rustup_home_dir() -> PathBuf {
    if Path::new("/home/cpn").is_dir() {
        PathBuf::from("/home/cpn")
    } else {
        PathBuf::from("/root")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_candidates_include_lab_rustup() {
        let paths = cargo_search_paths();
        let display: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        assert!(
            display
                .iter()
                .any(|p| p.contains("/home/cpn/.cargo/bin/cargo"))
        );
        assert!(
            display
                .iter()
                .any(|p| p.contains("/root/.cargo/bin/cargo"))
        );
    }

    #[test]
    fn missing_message_does_not_push_release_only() {
        let msg = missing_cargo_message();
        assert!(msg.contains("cargo"));
        assert!(!msg.contains("Use a published Release"));
    }
}
