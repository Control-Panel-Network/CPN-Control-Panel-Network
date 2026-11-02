//! Locate cargo/npm/node outside systemd PATH (rustup, /home/cpn, /usr/local).
//!
//! Tip source builds need Node for `installer-ui` (Vite 8 requires
//! `^20.19.0 || >=22.12.0`). Distro `/usr/bin/cargo` must not prepend `/usr/bin`
//! ahead of a newer Node under `/usr/local` (that produced `styleText` import
//! failures when npm 11 ran under Node 16).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Vite 8 / rolldown engines floor (`styleText` needs Node >= 20.12; pin 20.19).
pub const MIN_NODE_MAJOR: u32 = 20;
pub const MIN_NODE_MINOR: u32 = 19;

#[derive(Debug, Clone)]
pub struct Toolchain {
    pub cargo: PathBuf,
    pub npm: Option<PathBuf>,
    pub node: Option<PathBuf>,
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
    // Prefer rustup homes before PATH. Distro `/usr/bin/cargo` would prepend
    // `/usr/bin` and shadow Node 20+ under `/usr/local/bin`.
    for raw in [
        "/home/cpn/.cargo/bin/cargo",
        "/root/.cargo/bin/cargo",
        "/usr/local/cargo/bin/cargo",
        "/usr/local/bin/cargo",
        "/opt/cargo/bin/cargo",
    ] {
        paths.push(PathBuf::from(raw));
    }
    if let Some(found) = which_in_path("cargo")
        && !paths.iter().any(|p| p == &found)
    {
        paths.push(found);
    }
    paths
}

pub fn npm_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for raw in [
        "/usr/local/bin/npm",
        "/home/cpn/.local/bin/npm",
        "/root/.local/bin/npm",
        "/usr/bin/npm",
    ] {
        paths.push(PathBuf::from(raw));
    }
    if let Some(found) = which_in_path("npm")
        && !paths.iter().any(|p| p == &found)
    {
        paths.push(found);
    }
    paths
}

pub fn node_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for raw in [
        "/usr/local/bin/node",
        "/home/cpn/.local/bin/node",
        "/root/.local/bin/node",
        "/usr/bin/node",
    ] {
        paths.push(PathBuf::from(raw));
    }
    // Official tarball installs under /usr/local/node-v*/bin
    if let Ok(entries) = std::fs::read_dir("/usr/local") {
        let mut versioned: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("node-v"))
                    .unwrap_or(false)
            })
            .map(|p| p.join("bin/node"))
            .collect();
        versioned.sort();
        versioned.reverse();
        for path in versioned {
            if !paths.iter().any(|p| p == &path) {
                paths.push(path);
            }
        }
    }
    if let Some(found) = which_in_path("node")
        && !paths.iter().any(|p| p == &found)
    {
        paths.push(found);
    }
    paths
}

fn first_exec(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| is_exec(p)).cloned()
}

fn path_env_value(dirs: &[PathBuf]) -> String {
    let mut parts: Vec<String> = dirs
        .iter()
        .filter(|d| d.as_os_str().len() > 0)
        .map(|d| d.display().to_string())
        .collect();
    if let Ok(existing) = std::env::var("PATH") {
        for part in std::env::split_paths(&existing) {
            let s = part.display().to_string();
            if !parts.iter().any(|p| p == &s) {
                parts.push(s);
            }
        }
    }
    parts.join(":")
}

fn current_path_value(env: &[(String, String)]) -> String {
    env.iter()
        .find(|(k, _)| k == "PATH")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default())
}

/// Prepend `dir` in front of the toolchain PATH (Node must win over `/usr/bin`).
pub fn prepend_path_front(dir: &Path, env: &mut Vec<(String, String)>) {
    let mut parts = vec![dir.display().to_string()];
    let existing = current_path_value(env);
    if !existing.is_empty() {
        for part in std::env::split_paths(&existing) {
            let s = part.display().to_string();
            if !parts.iter().any(|p| p == &s) {
                parts.push(s);
            }
        }
    }
    env.retain(|(k, _)| k != "PATH");
    env.push(("PATH".into(), parts.join(":")));
}

/// Cap parallel rustc jobs from free RAM so tip builds on 4 to 8 GB labs
/// are less likely to hit the OOM killer (SIGKILL / exit 137).
pub fn cargo_build_jobs() -> u32 {
    let nproc = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(2)
        .max(1);
    let mem_mb = mem_available_mb().unwrap_or(2048);
    // Roughly 1.5 GiB per parallel rustc; leave headroom for the panel and linker.
    let by_ram = ((mem_mb / 1536).max(1)).min(u64::from(u32::MAX)) as u32;
    nproc.min(by_ram).clamp(1, 4)
}

fn mem_available_mb() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

/// Parse `v20.19.0` / `20.19.0` into (major, minor, patch).
pub fn parse_node_version(output: &str) -> Option<(u32, u32, u32)> {
    let trimmed = output.trim().trim_start_matches('v');
    let mut parts = trimmed.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts
        .next()
        .and_then(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .ok()
        })
        .unwrap_or(0);
    Some((major, minor, patch))
}

pub fn node_version_meets_min(major: u32, minor: u32) -> bool {
    major > MIN_NODE_MAJOR || (major == MIN_NODE_MAJOR && minor >= MIN_NODE_MINOR)
}

pub fn probe_node_version(node: &Path) -> Option<(u32, u32, u32)> {
    let output = Command::new(node).arg("-v").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_node_version(&text)
}

fn npm_beside(node: &Path) -> Option<PathBuf> {
    let bin = node.parent()?;
    let npm = bin.join("npm");
    if is_exec(&npm) { Some(npm) } else { None }
}

fn discover_node_npm() -> (Option<PathBuf>, Option<PathBuf>) {
    for node in node_search_paths() {
        if !is_exec(&node) {
            continue;
        }
        let Some((major, minor, _)) = probe_node_version(&node) else {
            continue;
        };
        if !node_version_meets_min(major, minor) {
            continue;
        }
        let npm = npm_beside(&node).or_else(|| first_exec(&npm_search_paths()));
        return (Some(node), npm);
    }
    // Fall back to whatever is present (caller may error with a clear message).
    let node = first_exec(&node_search_paths());
    let npm = node
        .as_ref()
        .and_then(|n| npm_beside(n))
        .or_else(|| first_exec(&npm_search_paths()));
    (node, npm)
}

fn env_for_toolchain(cargo: &Path, node: Option<&Path>) -> Vec<(String, String)> {
    let mut env = vec![("CARGO_TERM_COLOR".into(), "never".into())];
    let mut path_dirs: Vec<PathBuf> = Vec::new();
    // Node bin first: if cargo is `/usr/bin/cargo`, putting cargo's parent
    // first would shadow `/usr/local` Node with distro Node 16/18.
    if let Some(node) = node
        && let Some(bin) = node.parent()
    {
        path_dirs.push(bin.to_path_buf());
    }
    if let Some(bin) = cargo.parent() {
        if !path_dirs.iter().any(|p| p == bin) {
            path_dirs.push(bin.to_path_buf());
        }
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
    env.retain(|(k, _)| k != "PATH");
    env.push(("PATH".into(), path_env_value(&path_dirs)));
    let target = PathBuf::from("/home/cpn/cpn-cargo-target");
    if Path::new("/home/cpn").is_dir() {
        env.push(("CARGO_TARGET_DIR".into(), target.display().to_string()));
    }
    let jobs = cargo_build_jobs();
    env.retain(|(k, _)| k != "CARGO_BUILD_JOBS");
    env.push(("CARGO_BUILD_JOBS".into(), jobs.to_string()));
    env
}

pub fn discover() -> Option<Toolchain> {
    let cargo = first_exec(&cargo_search_paths())?;
    let (node, npm) = discover_node_npm();
    let env = env_for_toolchain(&cargo, node.as_deref());
    Some(Toolchain {
        cargo,
        npm,
        node,
        env,
    })
}

/// Fail early when installer-ui must be built but Node is too old or missing.
pub fn require_node_for_installer_ui(tools: &Toolchain) -> Result<(), String> {
    let Some(node) = tools.node.as_ref() else {
        return Err(format!(
            "installer-ui needs Node.js >={MIN_NODE_MAJOR}.{MIN_NODE_MINOR} (Vite 8). No node binary found under /usr/local, rustup homes, or PATH. Install Node 20.19+ or 22.12+ (for example from nodejs.org tarball under /usr/local), or use Upgrade to latest release / GitHub Actions tip binaries."
        ));
    };
    let Some((major, minor, patch)) = probe_node_version(node) else {
        return Err(format!(
            "Could not read Node version from {} (`node -v`). Install Node >={MIN_NODE_MAJOR}.{MIN_NODE_MINOR}.",
            node.display()
        ));
    };
    if !node_version_meets_min(major, minor) {
        return Err(format!(
            "installer-ui needs Node.js >={MIN_NODE_MAJOR}.{MIN_NODE_MINOR} (Vite 8 / styleText). Found v{major}.{minor}.{patch} at {}. Install Node 20.19+ or 22.12+ and ensure it sorts ahead of distro /usr/bin/node, or use a published release / Actions tip binary instead of a source tip build.",
            node.display()
        ));
    }
    if tools.npm.is_none() {
        return Err(
            "installer-ui needs npm next to a supported Node (or on PATH) and dist/index.html is missing"
                .into(),
        );
    }
    Ok(())
}

pub fn missing_cargo_message() -> String {
    "Could not find `cargo` after searching rustup homes, /home/cpn/.cargo/bin, /root/.cargo/bin, /usr/local, and PATH. GitHub Actions artifacts for this commit were not available. Install the Rust toolchain (rustup) on this host, or retry after CI tip binaries exist.".into()
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
    fn cargo_candidates_prefer_lab_rustup_before_path() {
        let paths = cargo_search_paths();
        let display: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        let home = display
            .iter()
            .position(|p| p.contains("/home/cpn/.cargo/bin/cargo"))
            .expect("home cargo");
        let root = display
            .iter()
            .position(|p| p.contains("/root/.cargo/bin/cargo"))
            .expect("root cargo");
        assert!(home < root);
        assert!(display[0].contains("/home/cpn/.cargo/bin/cargo"));
    }

    #[test]
    fn missing_message_does_not_push_release_only() {
        let msg = missing_cargo_message();
        assert!(msg.contains("cargo"));
        assert!(!msg.contains("Use a published Release"));
    }

    #[test]
    fn cargo_jobs_at_least_one_and_capped() {
        let jobs = cargo_build_jobs();
        assert!(jobs >= 1);
        assert!(jobs <= 4);
        assert!(!format!("{jobs}").contains('\u{2014}'));
    }

    #[test]
    fn parses_node_versions() {
        assert_eq!(parse_node_version("v20.19.0"), Some((20, 19, 0)));
        assert_eq!(parse_node_version("v16.20.2\n"), Some((16, 20, 2)));
        assert_eq!(parse_node_version("24.15.0"), Some((24, 15, 0)));
        assert!(node_version_meets_min(20, 19));
        assert!(node_version_meets_min(22, 12));
        assert!(node_version_meets_min(24, 0));
        assert!(!node_version_meets_min(20, 18));
        assert!(!node_version_meets_min(18, 20));
        assert!(!node_version_meets_min(16, 20));
    }

    #[test]
    fn node_requirement_message_mentions_styletext_floor() {
        let tools = Toolchain {
            cargo: PathBuf::from("/usr/bin/cargo"),
            npm: Some(PathBuf::from("/usr/local/bin/npm")),
            node: None,
            env: vec![],
        };
        let err = require_node_for_installer_ui(&tools).expect_err("missing node");
        assert!(err.contains("20.19"));
        assert!(!err.contains('\u{2014}'));
    }
}
