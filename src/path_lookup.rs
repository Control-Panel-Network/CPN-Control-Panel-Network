//! Locate executables through `PATH` without spawning a process.
//!
//! Linux hosts normally resolve tools with fixed `/usr/bin`-style probes plus a
//! `which` fallback. Windows (Phase A developer builds and the Windows service)
//! has no `which`, and executables carry an implicit `.exe` suffix, so the
//! probes here cover both so the same helpers behave on every build host.

use std::path::{Path, PathBuf};

/// True when `bin` (a bare program name, no directory separators) exists as a
/// file in one of the `PATH` directories. On Windows the `.exe` suffix is tried
/// as well when the name has no extension.
pub fn path_env_has(bin: &str) -> bool {
    find_in_path(bin).is_some()
}

/// Resolve `bin` to its first `PATH` hit, or `None` when absent.
pub fn find_in_path(bin: &str) -> Option<PathBuf> {
    if bin.is_empty() || bin.contains(['/', '\\']) {
        return None;
    }
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var).find_map(|dir| find_in_dir(&dir, bin))
}

fn find_in_dir(dir: &Path, bin: &str) -> Option<PathBuf> {
    if dir.as_os_str().is_empty() {
        return None;
    }
    let plain = dir.join(bin);
    if plain.is_file() {
        return Some(plain);
    }
    if cfg!(windows) && !bin.contains('.') {
        let exe = dir.join(format!("{bin}.exe"));
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_shell_on_every_build_host() {
        // Linux/macOS: `sh`. Windows: `cmd` (resolved as cmd.exe) or `powershell`.
        assert!(path_env_has("sh") || path_env_has("cmd") || path_env_has("powershell"));
    }

    #[test]
    fn rejects_paths_and_empty_names() {
        assert!(!path_env_has(""));
        assert!(!path_env_has("/usr/bin/sh"));
        assert!(!path_env_has("bin\\cmd.exe"));
        assert!(find_in_path("").is_none());
    }

    #[test]
    fn missing_tool_is_none() {
        assert!(find_in_path("cpn-definitely-not-installed-tool").is_none());
    }

    #[test]
    fn find_in_dir_handles_temp_file() {
        let dir = std::env::temp_dir().join(format!("cpn-path-lookup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = if cfg!(windows) {
            "cpntool.exe"
        } else {
            "cpntool"
        };
        std::fs::write(dir.join(name), b"").unwrap();
        assert!(find_in_dir(&dir, "cpntool").is_some());
        assert!(find_in_dir(&dir, "othertool").is_none());
        assert!(find_in_dir(Path::new(""), "cpntool").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
