//! Qualify Docker Hub short names for non-interactive Podman pulls and sanitize CLI noise.

use std::fs;
use std::path::Path;
use std::process::Command;

/// True when the active CLI is Podman or the docker-compatible Podman shim.
pub fn container_engine_is_podman() -> bool {
    let Some(bin) = crate::panel_ops_docker::docker_bin() else {
        return false;
    };
    if bin == "podman" {
        return true;
    }
    if bin != "docker" {
        return false;
    }
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            )
            .to_ascii_lowercase();
            text.contains("podman")
        })
        .unwrap_or(false)
}

fn split_name_tag_digest(raw: &str) -> (String, Option<String>, Option<String>) {
    let trimmed = raw.trim();
    let (base, digest) = if let Some(at) = trimmed.find('@') {
        (
            trimmed[..at].trim().to_string(),
            Some(trimmed[at..].to_string()),
        )
    } else {
        (trimmed.to_string(), None)
    };
    let (name, tag) = if let Some(colon) = base.rfind(':') {
        let before = &base[..colon];
        let after = &base[colon + 1..];
        if base.contains('/')
            || (!before.is_empty()
                && !before.starts_with("sha256:")
                && !after.contains('/')
                && !after.is_empty())
        {
            (before.to_string(), Some(after.to_string()))
        } else {
            (base, None)
        }
    } else {
        (base, None)
    };
    (name, tag, digest)
}

/// Whether the name part still needs an explicit `docker.io` registry for Podman short-name rules.
pub fn image_ref_needs_docker_io_prefix(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    let first = name.split('/').next().unwrap_or(name);
    !(first.contains('.') || first.contains(':') || first.eq_ignore_ascii_case("localhost"))
}

/// Prefix unqualified Hub-style refs with `docker.io` (and `library/` for single-segment names).
pub fn normalize_container_image_ref(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let (name, tag, digest) = split_name_tag_digest(trimmed);
    if !image_ref_needs_docker_io_prefix(&name) {
        return trimmed.to_string();
    }
    let full_name = if name.contains('/') {
        format!("docker.io/{name}")
    } else {
        format!("docker.io/library/{name}")
    };
    let mut out = full_name;
    if let Some(t) = tag {
        if !t.is_empty() {
            out.push(':');
            out.push_str(&t);
        }
    }
    if let Some(d) = digest {
        out.push_str(&d);
    }
    out
}

/// Remove podman-docker shim banner lines from stderr shown in the panel.
pub fn sanitize_container_cli_message(raw: &str) -> String {
    let mut lines: Vec<&str> = raw.lines().collect();
    lines.retain(|line| {
        let t = line.trim();
        !t.is_empty()
            && !t.starts_with("Emulate Docker CLI using podman")
            && !t.eq_ignore_ascii_case("Create /etc/containers/nodocker to quiet msg.")
            && !t.contains("Create /etc/containers/nodocker to quiet msg.")
    });
    let joined = lines.join("\n").trim().to_string();
    if joined.is_empty() {
        raw.trim().chars().take(280).collect()
    } else {
        joined.chars().take(280).collect()
    }
}

/// Idempotent Podman helpers so non-TTY pulls resolve `docker.io` without prompts.
pub fn heal_container_engine_podman_pull() -> Result<String, String> {
    if !container_engine_is_podman() {
        return Ok("Container engine is not Podman; skipped Podman pull heal.".into());
    }
    let mut notes = Vec::new();
    let nodocker = Path::new("/etc/containers/nodocker");
    if !nodocker.is_file() {
        if let Err(e) = fs::write(nodocker, b"") {
            notes.push(format!("nodocker: {e}"));
        } else {
            notes.push("created /etc/containers/nodocker".into());
        }
    } else {
        notes.push("/etc/containers/nodocker present".into());
    }
    let drop_in_dir = Path::new("/etc/containers/registries.conf.d");
    if drop_in_dir.is_dir() || fs::create_dir_all(drop_in_dir).is_ok() {
        let drop_in = drop_in_dir.join("99-cpn-unqualified-search.conf");
        let body = "unqualified-search-registries = [\"docker.io\"]\n";
        let write = !drop_in.is_file()
            || fs::read_to_string(&drop_in)
                .map(|existing| existing.trim() != body.trim())
                .unwrap_or(true);
        if write {
            match fs::write(&drop_in, body) {
                Ok(()) => notes.push(format!("wrote {}", drop_in.display())),
                Err(e) => notes.push(format!("registries drop-in: {e}")),
            }
        } else {
            notes.push("Podman unqualified-search registries already configured".into());
        }
    }
    Ok(notes.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_short_user_repo() {
        assert_eq!(
            normalize_container_image_ref("nightscout/cgm-remote-monitor:latest_dev"),
            "docker.io/nightscout/cgm-remote-monitor:latest_dev"
        );
    }

    #[test]
    fn normalize_official_single_name() {
        assert_eq!(
            normalize_container_image_ref("nginx:alpine"),
            "docker.io/library/nginx:alpine"
        );
    }

    #[test]
    fn leaves_qualified_registry() {
        assert_eq!(
            normalize_container_image_ref("gcr.io/project/image:1"),
            "gcr.io/project/image:1"
        );
        assert_eq!(
            normalize_container_image_ref("docker.io/library/nginx:latest"),
            "docker.io/library/nginx:latest"
        );
    }

    #[test]
    fn sanitize_podman_banner() {
        let raw = "Emulate Docker CLI using podman. Create /etc/containers/nodocker to quiet msg.\nError: short-name resolution enforced but cannot prompt without a TTY\n";
        let out = sanitize_container_cli_message(raw);
        assert!(out.contains("short-name"));
        assert!(!out.to_lowercase().contains("emulate docker"));
    }
}
