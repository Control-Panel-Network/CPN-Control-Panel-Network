//! Docker Hub search, image pull/delete/prune, and create-container helpers.

use crate::panel_ops_docker::{docker_bin, list_containers_detailed};
use serde::Deserialize;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct DockerHubSearchHit {
    pub name: String,
    pub description: String,
    pub star_count: u64,
    pub is_official: bool,
}

#[derive(Debug, Deserialize)]
struct HubSearchResponse {
    results: Option<Vec<HubSearchResult>>,
}

#[derive(Debug, Deserialize)]
struct HubSearchResult {
    repo_name: Option<String>,
    name: Option<String>,
    short_description: Option<String>,
    star_count: Option<u64>,
    is_official: Option<bool>,
}

/// Validate a Docker image reference for pull/run (no shell metacharacters).
pub fn validate_image_ref(raw: &str) -> Result<String, String> {
    let image = raw.trim();
    if image.is_empty() {
        return Err("Image name is required.".into());
    }
    if image.len() > 255 {
        return Err("Image name is too long.".into());
    }
    let ok = image
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':' | '@'));
    if !ok {
        return Err(
            "Invalid image name. Use repository and optional tag (for example nginx:alpine)."
                .into(),
        );
    }
    if image.contains("..") || image.starts_with('-') || image.contains("//") {
        return Err("Invalid image name.".into());
    }
    Ok(image.to_string())
}

fn validate_container_name(raw: &str) -> Result<Option<String>, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name.len() > 64 {
        return Err("Container name is too long.".into());
    }
    let ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !ok || name.starts_with('-') || name.contains('/') || name.contains('\\') {
        return Err("Invalid container name.".into());
    }
    Ok(Some(name.to_string()))
}

fn validate_restart_policy(raw: &str) -> Result<String, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "no" => Ok("no".into()),
        "always" => Ok("always".into()),
        "unless-stopped" => Ok("unless-stopped".into()),
        "on-failure" => Ok("on-failure".into()),
        other => Err(format!(
            "Invalid restart policy `{other}`. Use no, always, unless-stopped, or on-failure."
        )),
    }
}

fn parse_port_mappings(raw: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for part in raw.split(|c: char| c == '\n' || c == ',' || c == ';') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let ok = p
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ':' | '/' | '.' | '-'));
        if !ok || p.len() > 64 {
            return Err(format!(
                "Invalid port mapping `{p}`. Use host:container (for example 8080:80)."
            ));
        }
        out.push(p.to_string());
        if out.len() > 32 {
            return Err("Too many port mappings.".into());
        }
    }
    Ok(out)
}

fn parse_env_vars(raw: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once('=').ok_or_else(|| {
            format!("Invalid env line `{line}`. Use KEY=value (one per line).")
        })?;
        let key = k.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(format!("Invalid env key `{key}`."));
        }
        if v.contains('\0') || key.contains('\0') {
            return Err("Invalid env value.".into());
        }
        out.push((key.to_string(), v.to_string()));
        if out.len() > 64 {
            return Err("Too many environment variables.".into());
        }
    }
    Ok(out)
}

fn require_bin() -> Result<&'static str, String> {
    docker_bin().ok_or_else(|| "Docker/Podman CLI not found.".into())
}

fn run_cmd(bin: &str, args: &[&str], timeout_note: &str) -> Result<String, String> {
    let output = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not run {bin}: {e}"))?;
    if output.status.success() {
        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if out.is_empty() {
            Ok(timeout_note.to_string())
        } else {
            Ok(out.chars().take(400).collect())
        }
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let msg = if err.trim().is_empty() {
            out.trim()
        } else {
            err.trim()
        };
        Err(format!(
            "{} {} failed: {}",
            bin,
            args.first().unwrap_or(&""),
            msg.chars().take(280).collect::<String>()
        ))
    }
}

/// Public Docker Hub repository search (no credentials).
pub fn search_docker_hub(query: &str) -> Result<Vec<DockerHubSearchHit>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    if q.len() > 100
        || !q
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/' | '.' | ' '))
    {
        return Err("Invalid search query.".into());
    }
    let encoded: String = q
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    let url = format!(
        "https://hub.docker.com/v2/search/repositories/?query={encoded}&page_size=12"
    );
    let output = Command::new("curl")
        .args([
            "--fail-with-body",
            "--silent",
            "--show-error",
            "--max-time",
            "20",
            "-H",
            "Accept: application/json",
            &url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not search Docker Hub (curl missing?): {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Docker Hub search failed: {}",
            err.trim().chars().take(160).collect::<String>()
        ));
    }
    let body = String::from_utf8_lossy(&output.stdout);
    let parsed: HubSearchResponse = serde_json::from_str(&body)
        .map_err(|_| "Docker Hub returned an unexpected response.".to_string())?;
    let mut hits = Vec::new();
    for row in parsed.results.unwrap_or_default() {
        let name = row
            .repo_name
            .or(row.name)
            .unwrap_or_default()
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        hits.push(DockerHubSearchHit {
            name,
            description: row.short_description.unwrap_or_default(),
            star_count: row.star_count.unwrap_or(0),
            is_official: row.is_official.unwrap_or(false),
        });
        if hits.len() >= 12 {
            break;
        }
    }
    Ok(hits)
}

pub fn pull_image(image_ref: &str) -> Result<String, String> {
    let image = validate_image_ref(image_ref)?;
    let bin = require_bin()?;
    run_cmd(bin, &["pull", &image], &format!("Pulled `{image}`."))
        .map(|_| format!("Pulled `{image}`."))
}

/// True when any container (running or stopped) references this image id or repo:tag.
pub fn image_in_use(repository: &str, tag: &str, image_id: &str) -> bool {
    let ref_name = if repository == "<none>" || tag == "<none>" {
        image_id.trim().to_string()
    } else {
        format!("{}:{}", repository.trim(), tag.trim())
    };
    if ref_name.is_empty() {
        return false;
    }
    let Ok(bin) = require_bin() else {
        return true;
    };
    let output = Command::new(bin)
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("ancestor={ref_name}"),
            "--format",
            "{{.Names}}",
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => !String::from_utf8_lossy(&o.stdout).trim().is_empty(),
        _ => list_containers_detailed()
            .unwrap_or_default()
            .iter()
            .any(|c| {
                format!("{}:{}", c.image, c.tag) == ref_name
                    || c.image == ref_name
                    || (!image_id.is_empty() && c.id.starts_with(image_id))
            }),
    }
}

pub fn delete_image(repository: &str, tag: &str, image_id: &str) -> Result<String, String> {
    let repo = repository.trim();
    let tag = tag.trim();
    let id = image_id.trim();
    let target = if !repo.is_empty() && repo != "<none>" && !tag.is_empty() && tag != "<none>" {
        validate_image_ref(&format!("{repo}:{tag}"))?
    } else if !id.is_empty() && id.chars().all(|c| c.is_ascii_hexdigit() || c == ':') {
        id.to_string()
    } else {
        return Err("Invalid image reference for delete.".into());
    };
    if image_in_use(repo, tag, id) {
        return Err(format!(
            "Cannot delete `{target}`: it is in use by a container. Remove or recreate that container first."
        ));
    }
    let bin = require_bin()?;
    run_cmd(bin, &["rmi", &target], &format!("Deleted `{target}`."))
        .map(|_| format!("Deleted image `{target}`."))
}

pub fn prune_unused_images() -> Result<String, String> {
    let bin = require_bin()?;
    let out = run_cmd(bin, &["image", "prune", "-f"], "Pruned unused images.")?;
    if out.contains("Total reclaimed space") || out.contains("deleted") {
        Ok(out.chars().take(300).collect())
    } else {
        Ok(format!("Pruned unused images. {out}"))
    }
}

pub struct CreateContainerRequest<'a> {
    pub image: &'a str,
    pub name: &'a str,
    pub ports: &'a str,
    pub env: &'a str,
    pub restart: &'a str,
    pub start: bool,
}

pub fn create_container(req: CreateContainerRequest<'_>) -> Result<String, String> {
    let image = validate_image_ref(req.image)?;
    let name = validate_container_name(req.name)?;
    let restart = validate_restart_policy(req.restart)?;
    let ports = parse_port_mappings(req.ports)?;
    let envs = parse_env_vars(req.env)?;
    let bin = require_bin()?;

    let mut args: Vec<String> = if req.start {
        vec!["run".into(), "-d".into()]
    } else {
        vec!["create".into()]
    };
    if let Some(ref n) = name {
        args.push("--name".into());
        args.push(n.clone());
    }
    if restart != "no" {
        args.push("--restart".into());
        args.push(restart.clone());
    }
    for p in &ports {
        args.push("-p".into());
        args.push(p.clone());
    }
    for (k, v) in &envs {
        args.push("-e".into());
        args.push(format!("{k}={v}"));
    }
    // Never label user-created containers as CPN-managed.
    args.push(image.clone());

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_cmd(bin, &arg_refs, "Container created.")?;
    let label = name.as_deref().unwrap_or(out.trim());
    if req.start {
        Ok(format!(
            "Created and started container `{label}` from `{image}`."
        ))
    } else {
        Ok(format!(
            "Created container `{label}` from `{image}` (not started)."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_ref_ok() {
        assert!(validate_image_ref("nginx:alpine").is_ok());
        assert!(validate_image_ref("library/nginx").is_ok());
        assert!(validate_image_ref("nginx;rm").is_err());
        assert!(validate_image_ref("").is_err());
    }

    #[test]
    fn ports_and_env() {
        assert_eq!(parse_port_mappings("8080:80, 8443:443").unwrap().len(), 2);
        assert!(parse_port_mappings("80;rm").is_err());
        let envs = parse_env_vars("FOO=bar\n#c\nBAZ=1").unwrap();
        assert_eq!(envs.len(), 2);
        assert!(parse_env_vars("BAD LINE").is_err());
    }

    #[test]
    fn restart_policy() {
        assert_eq!(
            validate_restart_policy("unless-stopped").unwrap(),
            "unless-stopped"
        );
        assert!(validate_restart_policy("sometimes").is_err());
    }
}
