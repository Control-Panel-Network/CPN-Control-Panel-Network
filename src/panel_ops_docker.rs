//! Docker / Podman detection, listing, and container lifecycle for Server + Host package.

use std::process::Command;

const CPN_MANAGED_LABEL: &str = "com.cpn.managed";

#[derive(Debug, Clone)]
pub struct DockerStatus {
    pub installed: bool,
    pub running: bool,
    pub detail: String,
    pub bin: Option<&'static str>,
    pub containers: Vec<String>,
    pub images: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DockerContainerRow {
    pub id: String,
    pub name: String,
    pub image: String,
    pub tag: String,
    pub status: String,
    pub owner: String,
    pub cpn_managed: bool,
    pub running: bool,
}

#[derive(Debug, Clone)]
pub struct DockerImageRow {
    pub repository: String,
    pub tag: String,
    pub id: String,
    pub size: String,
}

pub fn docker_bin() -> Option<&'static str> {
    ["docker", "podman"].into_iter().find(|&candidate| {
        Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

fn docker_daemon_ok(bin: &str) -> bool {
    Command::new(bin)
        .args(["info"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn split_image_tag(image: &str) -> (String, String) {
    let image = image.trim();
    if image.is_empty() {
        return ("unknown".into(), "unknown".into());
    }
    // Digest or last colon after last slash is usually the tag.
    if let Some(slash) = image.rfind('/') {
        let after = &image[slash + 1..];
        if let Some(colon) = after.rfind(':') {
            let repo = format!("{}{}", &image[..=slash], &after[..colon]);
            let tag = after[colon + 1..].to_string();
            return (repo, if tag.is_empty() { "latest".into() } else { tag });
        }
    } else if let Some(colon) = image.rfind(':') {
        // Avoid splitting digests like sha256:...
        if !image[..colon].contains('@') && !image.starts_with("sha256:") {
            return (image[..colon].to_string(), image[colon + 1..].to_string());
        }
    }
    (image.to_string(), "latest".into())
}

fn label_map(raw: &str) -> Vec<(String, String)> {
    raw.split(',')
        .filter_map(|pair| {
            let pair = pair.trim();
            if pair.is_empty() {
                return None;
            }
            let (k, v) = pair.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

fn is_cpn_managed(labels: &[(String, String)]) -> bool {
    labels.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case(CPN_MANAGED_LABEL) && (v == "1" || v.eq_ignore_ascii_case("true"))
    })
}

fn owner_from_labels(labels: &[(String, String)], cpn_managed: bool) -> String {
    if let Some((_, v)) = labels
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("com.cpn.owner") || k.eq_ignore_ascii_case("owner"))
        && !v.is_empty()
    {
        return v.clone();
    }
    if cpn_managed {
        "CPN".into()
    } else {
        "Host".into()
    }
}

pub fn list_containers_detailed() -> Result<Vec<DockerContainerRow>, String> {
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    if !docker_daemon_ok(bin) {
        return Err(format!(
            "`{bin}` CLI found but the container engine is not running."
        ));
    }
    let output = Command::new(bin)
        .args([
            "ps",
            "-a",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Labels}}\t{{.State}}",
        ])
        .output()
        .map_err(|e| format!("Could not list containers: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Could not list containers: {}",
            err.trim().chars().take(200).collect::<String>()
        ));
    }
    let mut rows = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 {
            continue;
        }
        let id = parts[0].trim().to_string();
        let name = parts[1].trim().to_string();
        let (image, tag) = split_image_tag(parts[2].trim());
        let status = parts[3].trim().to_string();
        let labels = if parts.len() > 4 {
            label_map(parts[4])
        } else {
            Vec::new()
        };
        let state = parts
            .get(5)
            .map(|s| s.trim().to_ascii_lowercase())
            .unwrap_or_default();
        let cpn_managed = is_cpn_managed(&labels);
        let running = state == "running"
            || status.to_ascii_lowercase().starts_with("up ")
            || status.to_ascii_lowercase().contains("(healthy)");
        rows.push(DockerContainerRow {
            id,
            name: if name.is_empty() {
                "unknown".into()
            } else {
                name
            },
            image,
            tag,
            status,
            owner: owner_from_labels(&labels, cpn_managed),
            cpn_managed,
            running,
        });
        if rows.len() >= 200 {
            break;
        }
    }
    Ok(rows)
}

pub fn list_images_detailed() -> Result<Vec<DockerImageRow>, String> {
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    let output = Command::new(bin)
        .args([
            "images",
            "--format",
            "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.Size}}",
        ])
        .output()
        .map_err(|e| format!("Could not list images: {e}"))?;
    if !output.status.success() {
        return Err("Could not list images.".into());
    }
    let mut rows = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 {
            continue;
        }
        rows.push(DockerImageRow {
            repository: parts[0].trim().to_string(),
            tag: parts[1].trim().to_string(),
            id: parts[2].trim().to_string(),
            size: parts[3].trim().to_string(),
        });
        if rows.len() >= 200 {
            break;
        }
    }
    Ok(rows)
}

fn container_is_cpn_managed(bin: &str, name_or_id: &str) -> bool {
    let output = Command::new(bin)
        .args([
            "inspect",
            "--format",
            "{{index .Config.Labels \"com.cpn.managed\"}}",
            name_or_id,
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout)
                .trim()
                .to_ascii_lowercase();
            v == "1" || v == "true"
        }
        _ => false,
    }
}

fn run_container_cmd(bin: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("Could not run {bin}: {e}"))?;
    if output.status.success() {
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if out.is_empty() {
            Ok(format!("{} {}", bin, args.join(" ")))
        } else {
            Ok(out.chars().take(300).collect())
        }
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "{} {} failed: {}",
            bin,
            args.join(" "),
            err.trim().chars().take(240).collect::<String>()
        ))
    }
}

/// Start / stop / restart a container. CPN-managed stacks are allowed (operators may bounce them).
pub fn container_action(action: &str, name_or_id: &str) -> Result<String, String> {
    let name = name_or_id.trim();
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains(' ')
        || name.contains(';')
    {
        return Err("Invalid container name.".into());
    }
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    match action.trim().to_ascii_lowercase().as_str() {
        "start" => {
            run_container_cmd(bin, &["start", name]).map(|_| format!("Started container `{name}`."))
        }
        "stop" => {
            run_container_cmd(bin, &["stop", name]).map(|_| format!("Stopped container `{name}`."))
        }
        "restart" => run_container_cmd(bin, &["restart", name])
            .map(|_| format!("Restarted container `{name}`.")),
        "remove" | "rm" | "delete" => {
            if container_is_cpn_managed(bin, name) {
                return Err(format!(
                    "Refusing to remove `{name}`: labeled {CPN_MANAGED_LABEL}=1 (CPN-managed compose). Use CPN upgrade `--bypass` or manage that stack under /var/lib/cpn/docker."
                ));
            }
            run_container_cmd(bin, &["rm", "-f", name])
                .map(|_| format!("Removed container `{name}`."))
        }
        other => Err(format!(
            "Unknown container action `{other}`. Use start, stop, restart, or remove."
        )),
    }
}

pub fn container_logs(name_or_id: &str, lines: usize) -> Result<String, String> {
    let name = name_or_id.trim();
    if name.is_empty() || name.contains(' ') || name.contains(';') {
        return Err("Invalid container name.".into());
    }
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    let n = lines.clamp(20, 500).to_string();
    let output = Command::new(bin)
        .args(["logs", "--tail", &n, name])
        .output()
        .map_err(|e| format!("Could not read logs: {e}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&output.stderr).to_string();
    }
    if text.is_empty() {
        text = "(no log output)".into();
    }
    // Cap payload for HTML page.
    Ok(text.chars().take(20_000).collect())
}

pub fn docker_status() -> DockerStatus {
    let Some(bin) = docker_bin() else {
        return DockerStatus {
            installed: false,
            running: false,
            detail: "Docker not installed (docker/podman CLI not found)".into(),
            bin: None,
            containers: vec![],
            images: vec![],
        };
    };
    let running = docker_daemon_ok(bin);
    let containers = if running {
        list_containers_detailed()
            .unwrap_or_default()
            .into_iter()
            .map(|c| format!("{} {} {}:{} {}", c.id, c.name, c.image, c.tag, c.status))
            .take(50)
            .collect()
    } else {
        vec![]
    };
    let images = list_images_detailed()
        .unwrap_or_default()
        .into_iter()
        .map(|i| format!("{}:{} {} {}", i.repository, i.tag, i.id, i.size))
        .take(50)
        .collect();
    DockerStatus {
        installed: true,
        running,
        detail: if running {
            format!("Using `{bin}` CLI (engine running)")
        } else {
            format!("Using `{bin}` CLI (engine not running)")
        },
        bin: Some(bin),
        containers,
        images,
    }
}

fn enable_container_engine() -> Result<String, String> {
    let mut notes = Vec::new();
    for unit in ["docker", "podman", "podman.socket"] {
        let status = Command::new("systemctl")
            .args(["enable", "--now", unit])
            .status();
        if let Ok(s) = status
            && s.success()
        {
            notes.push(format!("enabled {unit}"));
        }
    }
    if notes.is_empty() {
        Ok(
            "Container packages installed; start docker or podman if the CLI is not ready yet."
                .into(),
        )
    } else {
        Ok(format!("Started container engine ({})", notes.join(", ")))
    }
}

/// Install Docker Engine (or Podman + docker-compatible CLI) via dnf/apt.
/// Does not recreate or touch CPN-managed compose under /var/lib/cpn/docker.
pub fn install_docker_engine() -> Result<String, String> {
    if let Some(bin) = docker_bin() {
        if docker_daemon_ok(bin) {
            return Ok(format!("Container engine already available via `{bin}`."));
        }
        let note = enable_container_engine()?;
        if docker_daemon_ok(bin) {
            return Ok(format!("Started existing `{bin}` engine. {note}"));
        }
    }

    let pm = crate::apps_pkg::package_manager()?;
    let mut messages = Vec::new();
    if pm == "dnf" {
        // Prefer real Docker when the distro/repo provides it; fall back to Podman.
        let try_sets: &[&[&str]] = &[
            &["docker-ce", "docker-ce-cli", "containerd.io"],
            &["moby-engine", "moby-cli"],
            &["docker"],
            &["podman", "podman-docker"],
        ];
        let mut installed = false;
        let mut last_err = String::new();
        for pkgs in try_sets {
            match crate::apps_pkg::install_packages_dnf_or_apt(pkgs, &[]) {
                Ok(()) => {
                    messages.push(format!("Installed packages: {}", pkgs.join(", ")));
                    installed = true;
                    break;
                }
                Err(e) => last_err = e,
            }
        }
        if !installed {
            return Err(format!(
                "Could not install a container engine via dnf ({last_err}). Add a Docker CE repo or install podman manually."
            ));
        }
    } else {
        let try_sets: &[&[&str]] = &[&["docker.io"], &["docker-ce"], &["podman"]];
        let mut installed = false;
        let mut last_err = String::new();
        for pkgs in try_sets {
            match crate::apps_pkg::install_packages_dnf_or_apt(&[], pkgs) {
                Ok(()) => {
                    messages.push(format!("Installed packages: {}", pkgs.join(", ")));
                    installed = true;
                    break;
                }
                Err(e) => last_err = e,
            }
        }
        if !installed {
            return Err(format!(
                "Could not install a container engine via apt ({last_err})."
            ));
        }
    }

    messages.push(enable_container_engine()?);
    messages.push(
        "Existing CPN-managed compose under /var/lib/cpn/docker and containers labeled com.cpn.managed=1 were left untouched."
            .into(),
    );
    Ok(messages.join(" "))
}

/// Remove Docker Engine packages. Never deletes /var/lib/cpn/docker or unlabeled volumes.
pub fn uninstall_docker_engine() -> Result<String, String> {
    let _ = crate::apps_pkg::disable_now(&["docker", "podman.socket"]);
    let _ = crate::apps_pkg::stop_units(&["docker", "podman"]);
    // Remove docker-family packages only; leave bare podman if other tools need it when
    // podman-docker was the only CPN-added shim... We still remove podman-docker / docker.io.
    let _ = crate::apps_pkg::remove_packages_dnf_or_apt(
        &[
            "docker-ce",
            "docker-ce-cli",
            "containerd.io",
            "moby-engine",
            "moby-cli",
            "docker",
            "podman-docker",
        ],
        &["docker.io", "docker-ce", "docker-ce-cli", "containerd.io"],
    );
    Ok(
        "Uninstalled Docker Engine packages when present. Volumes and /var/lib/cpn/docker compose projects were not deleted."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_honest() {
        let s = docker_status();
        assert!(!s.detail.is_empty());
    }

    #[test]
    fn split_image_tag_basic() {
        let (repo, tag) = split_image_tag("filebrowser/filebrowser:latest");
        assert_eq!(repo, "filebrowser/filebrowser");
        assert_eq!(tag, "latest");
        let (repo2, tag2) = split_image_tag("nginx");
        assert_eq!(repo2, "nginx");
        assert_eq!(tag2, "latest");
    }

    #[test]
    fn reject_bad_container_name() {
        assert!(container_action("start", "../evil").is_err());
        assert!(container_action("remove", "a;rm").is_err());
    }
}
