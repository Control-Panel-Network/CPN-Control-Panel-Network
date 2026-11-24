//! Delegation from a CPN panel container to the root-owned host image updater.
//!
//! The panel never receives the Docker socket. It writes a small request under
//! the existing `/var/lib/cpn` bind. A host systemd timer validates the running
//! container, persistent mounts, image allowlist, and Compose project before it
//! pulls and recreates the outer container.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const UPDATE_DIR: &str = "docker-host-update";
const HOST_CONFIG: &str = "host-config.json";
const HOST_STATUS: &str = "status.json";
const REQUEST: &str = "request.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DockerHostUpdateStatus {
    pub container_deployment: bool,
    pub host_updater_configured: bool,
    pub image: String,
    pub compose_service: String,
    pub last_check_at: u64,
    pub last_check_ok: bool,
    pub last_result: String,
    pub current_image_id: String,
    pub pending_request: bool,
    pub privilege_note: String,
}

#[derive(Debug, Clone, Serialize)]
struct DockerHostUpdateRequest<'a> {
    schema: u8,
    requested_at: u64,
    requested_by: &'a str,
    target_version: &'a str,
    target_tag: &'a str,
}

fn update_dir() -> PathBuf {
    paths::default_data_dir().join(UPDATE_DIR)
}

fn set_private(path: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// True only for the supported outer CPN image deployment.
///
/// `/.dockerenv` catches Docker. The explicit environment marker is included
/// in supported Compose files and avoids treating unrelated nested containers
/// as image-managed CPN deployments.
pub fn is_outer_container_deployment() -> bool {
    std::path::Path::new("/.dockerenv").exists()
        && std::env::var("CPN_OUTER_IMAGE_UPDATE")
            .map(|v| v == "host-helper")
            .unwrap_or(false)
}

pub fn host_update_status() -> DockerHostUpdateStatus {
    let container_deployment = is_outer_container_deployment();
    let dir = update_dir();
    let mut status = fs::read_to_string(dir.join(HOST_STATUS))
        .ok()
        .and_then(|raw| serde_json::from_str::<DockerHostUpdateStatus>(&raw).ok())
        .unwrap_or_default();

    status.container_deployment = container_deployment;
    status.host_updater_configured = dir.join(HOST_CONFIG).is_file();
    status.pending_request = dir.join(REQUEST).is_file();
    status.privilege_note = if status.host_updater_configured {
        "A root-owned host helper performs image pulls and Compose recreation. The panel container has no Docker socket.".into()
    } else if container_deployment {
        "Outer image updates require the root-owned host helper. Install it on the Docker host; no Docker socket is mounted into the panel.".into()
    } else {
        "Native install: package maintenance runs through the installer service.".into()
    };
    status
}

/// Queue a host image update without exposing host privileges to the panel.
pub fn queue_host_image_update(
    requested_by: &str,
    target_version: &str,
    target_tag: &str,
) -> Result<DockerHostUpdateStatus, String> {
    if !is_outer_container_deployment() {
        return Err("This is not a supported CPN outer-container deployment.".into());
    }
    let mut status = host_update_status();
    if !status.host_updater_configured {
        return Err(
            "Docker host updater is not configured. Run scripts/install-docker-image-updater.sh on the host with the Hub Compose file."
                .into(),
        );
    }

    let dir = update_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create Docker host update directory: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }

    let request = DockerHostUpdateRequest {
        schema: 1,
        requested_at: crate::auto_upgrade::now_unix(),
        requested_by,
        target_version,
        target_tag,
    };
    let raw = serde_json::to_string_pretty(&request)
        .map_err(|e| format!("Could not serialize Docker image update request: {e}"))?;
    let path = dir.join(REQUEST);
    let tmp = dir.join("request.json.tmp");
    {
        let mut file = fs::File::create(&tmp)
            .map_err(|e| format!("Could not create Docker image update request: {e}"))?;
        file.write_all(raw.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("Could not save Docker image update request: {e}"))?;
    }
    set_private(&tmp);
    fs::rename(&tmp, &path)
        .map_err(|e| format!("Could not publish Docker image update request: {e}"))?;
    set_private(&path);

    status.pending_request = true;
    status.last_result = format!(
        "Host image update queued for {target_tag}. The host timer will pull and recreate the CPN service."
    );
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_status_does_not_claim_host_privileges() {
        if !is_outer_container_deployment() {
            let status = host_update_status();
            assert!(!status.container_deployment);
            assert!(status.privilege_note.contains("Native install"));
        }
    }
}
