//! Path allowlisting and directory listing for Root File Manager.

use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

/// Admin Root File Manager may browse the whole host filesystem (documented risk).
pub fn allowed_roots() -> Vec<PathBuf> {
    vec![PathBuf::from("/")]
}

#[derive(Debug, Clone)]
pub struct DirEntryInfo {
    /// Filesystem basename (used for operations).
    pub basename: String,
    /// Display label (may include symlink target).
    pub label: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub mtime_label: String,
    pub mode_label: String,
}

/// Resolve `requested` under an allowlisted root. Rejects traversal and NUL bytes.
pub fn resolve_under_allowlist(requested: &str) -> Result<PathBuf, String> {
    let raw = requested.trim();
    if raw.is_empty() {
        return Ok(PathBuf::from("/"));
    }
    if raw.contains('\0') {
        return Err("Invalid path".into());
    }
    let path = PathBuf::from(raw);
    // On Windows, Unix-style `/home` has a root but is not `is_absolute()` (no drive prefix).
    if !(path.is_absolute() || path.has_root()) {
        return Err("Path must be absolute".into());
    }
    let normalized = normalize_path(&path)?;
    for root in allowed_roots() {
        if path_is_under(&normalized, &root)? {
            return Ok(normalized);
        }
    }
    Err("Path is outside the Root File Manager allowlist".into())
}

/// Join a directory with a single file/folder name (rejects nested segments).
pub fn join_child(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let clean = validate_entry_name(name)?;
    let joined = parent.join(clean);
    resolve_under_allowlist(&joined.display().to_string())
}

pub fn validate_entry_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Name is required".into());
    }
    if name.contains('\0') || name.contains('/') || name.contains('\\') {
        return Err("Invalid name".into());
    }
    if name == "." || name == ".." {
        return Err("Invalid name".into());
    }
    Ok(name)
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
    if out.as_os_str().is_empty() {
        out.push("/");
    }
    Ok(out)
}

fn path_is_under(path: &Path, root: &Path) -> Result<bool, String> {
    let path_n = normalize_path(path)?;
    let root_n = normalize_path(root)?;
    if root_n == Path::new("/") {
        return Ok(path_n.has_root() || path_n.is_absolute());
    }
    let mut path_comps = path_n.components();
    for root_comp in root_n.components() {
        match path_comps.next() {
            Some(c) if c == root_comp => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// Paths that should not be overwritten or deleted via the manager.
pub fn is_protected_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    matches!(
        s.as_ref(),
        "/" | "/bin" | "/boot" | "/dev" | "/etc" | "/lib" | "/lib64" | "/proc" | "/root" | "/sbin"
            | "/sys" | "/usr" | "/var"
    ) || s.starts_with("/proc/")
        || s.starts_with("/sys/")
        || s.starts_with("/dev/")
}

/// List directory entries with size, mtime, and permission labels.
pub fn list_dir(path: &Path) -> Result<Vec<DirEntryInfo>, String> {
    let meta =
        std::fs::metadata(path).map_err(|e| format!("Cannot stat {}: {e}", path.display()))?;
    if !meta.is_dir() {
        return Err("Not a directory".into());
    }
    let mut entries = Vec::new();
    let rd = std::fs::read_dir(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    for ent in rd.flatten() {
        let basename = ent.file_name().to_string_lossy().to_string();
        if basename == "." || basename == ".." {
            continue;
        }
        let is_symlink = ent.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        let meta = ent.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = if is_dir {
            0
        } else {
            meta.as_ref().map(|m| m.len()).unwrap_or(0)
        };
        let mtime_label = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(format_mtime)
            .unwrap_or_else(|| "-".into());
        let mode_label = meta
            .as_ref()
            .map(format_mode)
            .unwrap_or_else(|| "----------".into());
        let label = if is_symlink {
            match std::fs::read_link(ent.path()) {
                Ok(target) => format!("{basename} -> {}", target.display()),
                Err(_) => basename.clone(),
            }
        } else {
            basename.clone()
        };
        entries.push(DirEntryInfo {
            basename,
            label,
            is_dir,
            is_symlink,
            size,
            mtime_label,
            mode_label,
        });
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.basename.to_lowercase().cmp(&b.basename.to_lowercase()),
    });
    Ok(entries)
}

fn format_mtime(t: SystemTime) -> String {
    use std::time::UNIX_EPOCH;
    let Ok(dur) = t.duration_since(UNIX_EPOCH) else {
        return "-".into();
    };
    let secs = dur.as_secs() as i64;
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400) as u32;
    let (y, m, d) = civil_from_days(days);
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    format!("{d:02}/{m:02}/{y} {hh:02}:{mm:02}")
}

/// Howard Hinnant civil_from_days (UTC).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn format_mode(meta: &std::fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        let file_type = if meta.file_type().is_symlink() {
            'l'
        } else if meta.is_dir() {
            'd'
        } else {
            '-'
        };
        let mut s = String::from(file_type);
        for shift in [8u32, 7, 6, 5, 4, 3, 2, 1, 0] {
            let ch = match shift % 3 {
                2 => 'r',
                1 => 'w',
                _ => 'x',
            };
            if mode & (1 << shift) != 0 {
                s.push(ch);
            } else {
                s.push('-');
            }
        }
        s
    }
    #[cfg(not(unix))]
    {
        if meta.is_dir() {
            "drwxrwxrwx".into()
        } else if meta.permissions().readonly() {
            "-r--r--r--".into()
        } else {
            "-rw-rw-rw-".into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_absolute_paths_under_root() {
        assert!(resolve_under_allowlist("/").is_ok());
        assert!(resolve_under_allowlist("/home").is_ok());
        assert!(resolve_under_allowlist("/var/www/html").is_ok());
        assert!(resolve_under_allowlist("/etc").is_ok());
        assert!(resolve_under_allowlist("/home/../etc/passwd").is_ok());
    }

    #[test]
    fn rejects_relative() {
        assert!(resolve_under_allowlist("home/foo").is_err());
        assert!(resolve_under_allowlist("../home").is_err());
    }

    #[test]
    fn validates_names() {
        assert!(validate_entry_name("ok.txt").is_ok());
        assert!(validate_entry_name("../x").is_err());
        assert!(validate_entry_name("a/b").is_err());
    }

    #[test]
    fn protected_roots() {
        assert!(is_protected_path(Path::new("/")));
        assert!(is_protected_path(Path::new("/etc")));
        assert!(!is_protected_path(Path::new("/home/site")));
    }
}
