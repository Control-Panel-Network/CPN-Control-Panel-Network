//! Docker detection and listing for Server hub.

use std::process::Command;

#[derive(Debug, Clone)]
pub struct DockerStatus {
    pub installed: bool,
    pub detail: String,
    pub containers: Vec<String>,
    pub images: Vec<String>,
}

fn docker_bin() -> Option<&'static str> {
    for candidate in ["docker", "podman"] {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(candidate);
        }
    }
    None
}

pub fn docker_status() -> DockerStatus {
    let Some(bin) = docker_bin() else {
        return DockerStatus {
            installed: false,
            detail: "Docker not installed (docker/podman CLI not found)".into(),
            containers: vec![],
            images: vec![],
        };
    };
    let containers = Command::new(bin)
        .args([
            "ps",
            "-a",
            "--format",
            "{{.ID}} {{.Image}} {{.Status}} {{.Names}}",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .take(50)
                .collect()
        })
        .unwrap_or_default();
    let images = Command::new(bin)
        .args([
            "images",
            "--format",
            "{{.Repository}}:{{.Tag}} {{.ID}} {{.Size}}",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .take(50)
                .collect()
        })
        .unwrap_or_default();
    DockerStatus {
        installed: true,
        detail: format!("Using `{bin}` CLI"),
        containers,
        images,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_honest() {
        let s = docker_status();
        assert!(!s.detail.is_empty());
    }
}
