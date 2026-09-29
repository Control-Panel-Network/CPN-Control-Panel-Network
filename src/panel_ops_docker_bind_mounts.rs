//! Host bind-mount directories for Create Container and compose stacks.

use crate::panel_ops_docker::docker_bin;
use crate::panel_ops_docker_image_ref::normalize_container_image_ref;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const DEFAULT_CONTAINER_UID: u32 = 1000;
const DEFAULT_CONTAINER_GID: u32 = 1000;

/// Host path from `host_path:container_path`.
pub fn host_path_from_binding(binding: &str) -> Option<PathBuf> {
    let host = binding.split_once(':')?.0.trim();
    if host.is_empty() || !host.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(host))
}

fn is_under_cpn_docker_data(path: &Path) -> bool {
    path.starts_with("/var/lib/cpn/docker-data")
}

fn parse_user_spec(raw: &str) -> Option<(u32, u32)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(uid) = raw.parse::<u32>() {
        return Some((uid, uid));
    }
    if let Some((u, g)) = raw.split_once(':') {
        let uid = u.trim().parse::<u32>().ok()?;
        let g = g.trim();
        let gid = if g.is_empty() {
            uid
        } else {
            g.parse::<u32>().ok()?
        };
        return Some((uid, gid));
    }
    None
}

fn parse_id_output(raw: &str) -> Option<(u32, u32)> {
    let mut uid = None;
    let mut gid = None;
    for token in raw.split_whitespace() {
        if let Some(rest) = token.strip_prefix("uid=") {
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            uid = num.parse().ok();
        }
        if let Some(rest) = token.strip_prefix("gid=") {
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            gid = num.parse().ok();
        }
    }
    match (uid, gid) {
        (Some(u), Some(g)) => Some((u, g)),
        (Some(u), None) => Some((u, u)),
        _ => None,
    }
}

fn runtime_uid_gid_from_image(image_ref: &str) -> Option<(u32, u32)> {
    let bin = docker_bin()?;
    let image = normalize_container_image_ref(image_ref);
    let output = Command::new(bin)
        .args(["run", "--rm", "--entrypoint", "id", &image])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_id_output(&String::from_utf8_lossy(&output.stdout))
}

fn image_user_ids(image_ref: &str) -> Option<(u32, u32)> {
    let bin = docker_bin()?;
    let image = normalize_container_image_ref(image_ref);
    let output = Command::new(bin)
        .args(["inspect", "--format", "{{.Config.User}}", &image])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if output.status.success() {
        let user = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Some(ids) = parse_user_spec(&user) {
            return Some(ids);
        }
        if !user.is_empty()
            && let Some(ids) = runtime_uid_gid_from_image(image_ref)
        {
            return Some(ids);
        }
    }
    runtime_uid_gid_from_image(image_ref).or(Some((DEFAULT_CONTAINER_UID, DEFAULT_CONTAINER_GID)))
}

#[cfg(unix)]
fn chown_path(path: &Path, uid: u32, gid: u32) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return;
    };
    unsafe {
        libc::chown(c_path.as_ptr(), uid as libc::uid_t, gid as libc::gid_t);
    }
}

#[cfg(not(unix))]
fn chown_path(_path: &Path, _uid: u32, _gid: u32) {}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

/// Create host bind-mount paths with modes suitable for non-root container users.
pub fn ensure_bind_mount_host_dirs(
    bindings: &[String],
    image_ref: Option<&str>,
) -> Result<(), String> {
    if bindings.is_empty() {
        return Ok(());
    }
    let user_ids = image_ref.and_then(image_user_ids);
    let (uid, gid) = user_ids.unwrap_or((DEFAULT_CONTAINER_UID, DEFAULT_CONTAINER_GID));

    for binding in bindings {
        let Some(host) = host_path_from_binding(binding) else {
            continue;
        };
        if host
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(format!(
                "Invalid bind mount host path `{}`.",
                host.display()
            ));
        }
        fs::create_dir_all(&host).map_err(|e| {
            format!(
                "Could not create bind mount directory {}: {e}",
                host.display()
            )
        })?;

        let mode = if is_under_cpn_docker_data(&host) {
            0o775
        } else {
            0o755
        };
        set_mode(&host, mode);
        chown_path(&host, uid, gid);

        // Ensure parent chain under docker-data is traversable.
        if is_under_cpn_docker_data(&host)
            && let Some(parent) = host.parent()
        {
            set_mode(parent, 0o775);
            chown_path(parent, uid, gid);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_path_parses() {
        assert_eq!(
            host_path_from_binding("/var/lib/cpn/docker-data/x:/data"),
            Some(PathBuf::from("/var/lib/cpn/docker-data/x"))
        );
        assert!(host_path_from_binding("bad").is_none());
    }

    #[test]
    fn user_spec_parses() {
        assert_eq!(parse_user_spec("1000"), Some((1000, 1000)));
        assert_eq!(parse_user_spec("1000:1001"), Some((1000, 1001)));
        assert_eq!(parse_user_spec(""), None);
    }

    #[test]
    fn id_output_parses() {
        assert_eq!(
            parse_id_output("uid=1000(user) gid=1000(user) groups=1000(user)"),
            Some((1000, 1000))
        );
    }
}
