//! Path allowlisting for root file manager and related ops.

use std::path::{Component, Path, PathBuf};

/// Roots operators may browse from the panel file manager.
pub fn allowed_roots() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/home"),
        PathBuf::from("/var/www"),
        crate::paths::default_data_dir(),
    ]
}

/// Resolve `requested` under an allowlisted root. Rejects traversal and escapes.
pub fn resolve_under_allowlist(requested: &str) -> Result<PathBuf, String> {
    let raw = requested.trim();
    if raw.is_empty() {
        return Ok(PathBuf::from("/home"));
    }
    if raw.contains('\0') {
        return Err("Invalid path".into());
    }
    let path = PathBuf::from(raw);
    // On Windows, Unix-style `/home` has a root but is not `is_absolute()` (no drive prefix).
    // Panel labs are Linux; still accept rooted paths for portable validation/tests.
    if !(path.is_absolute() || path.has_root()) {
        return Err("Path must be absolute".into());
    }
    let normalized = normalize_path(&path)?;
    for root in allowed_roots() {
        if path_is_under(&normalized, &root)? {
            return Ok(normalized);
        }
    }
    Err("Path is outside allowlisted roots (/home, /var/www, CPN data dir)".into())
}

fn normalize_path(path: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return Err("Path traversal rejected".into());
                }
            }
            Component::Normal(seg) => out.push(seg),
        }
    }
    Ok(out)
}

fn path_is_under(path: &Path, root: &Path) -> Result<bool, String> {
    let path_n = normalize_path(path)?;
    let root_n = normalize_path(root)?;
    let mut path_comps = path_n.components();
    for root_comp in root_n.components() {
        match path_comps.next() {
            Some(c) if c == root_comp => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// List directory entries (name, is_dir, size if file).
pub fn list_dir(path: &Path) -> Result<Vec<(String, bool, u64)>, String> {
    let meta =
        std::fs::metadata(path).map_err(|e| format!("Cannot stat {}: {e}", path.display()))?;
    if !meta.is_dir() {
        return Err("Not a directory".into());
    }
    let mut entries = Vec::new();
    let rd = std::fs::read_dir(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if name == "." || name == ".." {
            continue;
        }
        let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let size = if is_dir {
            0
        } else {
            ent.metadata().map(|m| m.len()).unwrap_or(0)
        };
        entries.push((name, is_dir, size));
    }
    entries.sort_by(|a, b| match (a.1, b.1) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
    });
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        assert!(resolve_under_allowlist("/home/../etc/passwd").is_err());
        assert!(resolve_under_allowlist("/home/user/../../etc").is_err());
    }

    #[test]
    fn allows_home_and_var_www() {
        assert!(resolve_under_allowlist("/home").is_ok());
        assert!(resolve_under_allowlist("/home/example.com").is_ok());
        assert!(resolve_under_allowlist("/var/www").is_ok());
        assert!(resolve_under_allowlist("/var/www/html").is_ok());
    }

    #[test]
    fn rejects_outside() {
        assert!(resolve_under_allowlist("/etc").is_err());
        assert!(resolve_under_allowlist("/root").is_err());
        assert!(resolve_under_allowlist("/homeevil").is_err());
    }

    #[test]
    fn rejects_relative() {
        assert!(resolve_under_allowlist("home/foo").is_err());
        assert!(resolve_under_allowlist("../home").is_err());
    }
}
