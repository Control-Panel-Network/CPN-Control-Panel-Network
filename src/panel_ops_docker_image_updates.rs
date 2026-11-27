//! Cached registry digest checks for Docker Active Containers (detect only; no auto-pull).
//!
//! Uses the local container CLI `manifest inspect` when possible, with a Docker Hub v2 fallback.

use crate::account::now_unix;
use crate::paths::default_data_dir;
use crate::panel_ops_docker::{docker_bin, DockerContainerRow};
use crate::panel_ops_docker_image_ref::normalize_container_image_ref;
use crate::panel_ops_docker_probe::{LIST_TIMEOUT, output_with_timeout};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// How long cached registry results stay valid before the list shows "Not checked" again unless refreshed.
pub const CACHE_TTL_SECS: u64 = 3 * 3600;

/// Cap registry calls per manual "Check for updates" to avoid Hub rate limits.
pub const MAX_CHECKS_PER_REFRESH: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCheckState {
    UpToDate,
    UpdateAvailable,
    Unknown,
    NotChecked,
}

#[derive(Debug, Clone)]
pub struct ImageUpdateSnapshot {
    pub state: UpdateCheckState,
    pub detail: String,
    pub checked_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    state: String,
    detail: String,
    local_digest: String,
    remote_digest: String,
    checked_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UpdateCacheFile {
    schema: u8,
    entries: HashMap<String, CacheEntry>,
}

fn cache_path() -> PathBuf {
    default_data_dir().join("docker-registry-update-cache.json")
}

fn load_cache() -> UpdateCacheFile {
    let Ok(raw) = fs::read_to_string(cache_path()) else {
        return UpdateCacheFile {
            schema: 1,
            entries: HashMap::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_cache(cache: &UpdateCacheFile) -> Result<(), String> {
    if let Some(parent) = cache_path().parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create panel data dir for update cache: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(cache)
        .map_err(|e| format!("Could not serialize Docker update cache: {e}"))?;
    let tmp = cache_path().with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)
            .map_err(|e| format!("Could not write Docker update cache: {e}"))?;
        file.write_all(raw.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|e| format!("Could not write Docker update cache: {e}"))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, cache_path())
        .map_err(|e| format!("Could not save Docker update cache: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(cache_path(), fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn entry_state(raw: &str) -> UpdateCheckState {
    match raw {
        "uptodate" => UpdateCheckState::UpToDate,
        "update" => UpdateCheckState::UpdateAvailable,
        "unknown" => UpdateCheckState::Unknown,
        _ => UpdateCheckState::NotChecked,
    }
}

fn state_token(state: UpdateCheckState) -> &'static str {
    match state {
        UpdateCheckState::UpToDate => "uptodate",
        UpdateCheckState::UpdateAvailable => "update",
        UpdateCheckState::Unknown => "unknown",
        UpdateCheckState::NotChecked => "notchecked",
    }
}

fn cache_fresh(entry: &CacheEntry) -> bool {
    let age = now_unix().saturating_sub(entry.checked_at);
    age <= CACHE_TTL_SECS
}

pub fn container_image_ref(row: &DockerContainerRow) -> Option<String> {
    let image = row.image.trim();
    let tag = row.tag.trim();
    if image.is_empty()
        || tag.is_empty()
        || image.eq_ignore_ascii_case("unknown")
        || tag.eq_ignore_ascii_case("unknown")
        || tag == "<none>"
        || image == "<none>"
    {
        return None;
    }
    let raw = if image.contains('@') {
        image.to_string()
    } else {
        format!("{image}:{tag}")
    };
    let normalized = normalize_container_image_ref(&raw);
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

/// Normalize any digest or repo-digest string to `sha256:...` for comparison.
pub fn normalize_digest(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(at) = t.rfind('@') {
        return normalize_digest(&t[at + 1..]);
    }
    if t.starts_with("sha256:") && t.len() > 15 {
        return Some(t.to_string());
    }
  None
}

fn local_digest(bin: &str, image_ref: &str) -> Option<String> {
    let formats = [
        "{{index .RepoDigests 0}}",
        "{{join .RepoDigests \",\"}}",
        "{{.Id}}",
    ];
    for fmt in formats {
        let output = output_with_timeout(
            bin,
            &["image", "inspect", "--format", fmt, image_ref],
            LIST_TIMEOUT,
        )
        .ok()?;
        if !output.status.success() {
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for part in stdout.split(',') {
            if let Some(d) = normalize_digest(part.trim()) {
                return Some(d);
            }
        }
        if fmt == "{{.Id}}" {
            let id = stdout.trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct ManifestInspect {
    #[serde(default, alias = "digest")]
    #[allow(non_snake_case)]
    Digest: String,
    #[serde(default)]
    config: ManifestConfig,
}

#[derive(Debug, Default, Deserialize)]
struct ManifestConfig {
    #[serde(default)]
    digest: String,
}

fn manifest_digest(bin: &str, image_ref: &str) -> Result<String, String> {
    let output = output_with_timeout(bin, &["manifest", "inspect", image_ref], LIST_TIMEOUT)
        .map_err(|e| format!("manifest inspect failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(msg.chars().take(200).collect());
    }
    let parsed: ManifestInspect = serde_json::from_str(&stdout)
        .map_err(|_| "manifest inspect returned unexpected JSON".to_string())?;
    if let Some(d) = normalize_digest(&parsed.Digest) {
        return Ok(d);
    }
    if let Some(d) = normalize_digest(&parsed.config.digest) {
        return Ok(d);
    }
    Err("manifest inspect had no digest".into())
}

fn hub_repo_parts(image_ref: &str) -> Option<(String, String)> {
    let normalized = normalize_container_image_ref(image_ref);
    let rest = normalized.strip_prefix("docker.io/")?;
    let (path, tag) = if let Some(colon) = rest.rfind(':') {
        let tag_part = &rest[colon + 1..];
        if tag_part.contains('/') {
            return None;
        }
        (rest[..colon].to_string(), tag_part.to_string())
    } else {
        (rest.to_string(), "latest".to_string())
    };
    if path.is_empty() || tag.is_empty() {
        return None;
    }
    let repo = if path.contains('/') {
        path
    } else {
        format!("library/{path}")
    };
    Some((repo, tag))
}

fn hub_remote_digest_http(image_ref: &str) -> Result<String, String> {
    let (repo, tag) = hub_repo_parts(image_ref).ok_or_else(|| "Not a Docker Hub image.".to_string())?;
    let scope = format!("repository:{repo}:pull");
    let token_url = format!(
        "https://auth.docker.io/token?service=registry.docker.io&scope={}",
        url_encode(&scope)
    );
    let token_body = curl_json(&token_url, None)?;
    let token: HubToken =
        serde_json::from_str(&token_body).map_err(|_| "Docker Hub auth token parse failed".to_string())?;
    let bearer = token
        .token
        .or(token.access_token)
        .ok_or_else(|| "Docker Hub auth token missing".to_string())?;
    let manifest_url = format!("https://registry-1.docker.io/v2/{repo}/manifests/{tag}");
    let accepts = "application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json, application/vnd.oci.image.index.v1+json, application/vnd.oci.image.manifest.v1+json";
    let digest = curl_manifest_digest(&manifest_url, &bearer, accepts)?;
    normalize_digest(&digest).ok_or_else(|| "Registry manifest had no digest".to_string())
}

#[derive(Debug, Deserialize)]
struct HubToken {
    token: Option<String>,
    access_token: Option<String>,
}

fn url_encode(raw: &str) -> String {
    raw.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn curl_json(url: &str, auth: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "--fail-with-body",
        "--silent",
        "--show-error",
        "--max-time",
        "25",
        "-H",
        "Accept: application/json",
    ]);
    if let Some(token) = auth {
        cmd.arg("-H").arg(format!("Authorization: Bearer {token}"));
    }
    cmd.arg(url);
    let output = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("curl unavailable: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let body = String::from_utf8_lossy(&output.stdout);
        let msg = if err.trim().is_empty() { body.trim() } else { err.trim() };
        if msg.contains("429") || body.contains("429") {
            return Err("Docker Hub rate limit; try again later.".into());
        }
        return Err(msg.chars().take(160).collect());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn curl_manifest_digest(url: &str, bearer: &str, accept: &str) -> Result<String, String> {
    let auth_header = format!("Authorization: Bearer {bearer}");
    let accept_header = format!("Accept: {accept}");
    let output = Command::new("curl")
        .args([
            "--fail-with-body",
            "--silent",
            "--show-error",
            "--max-time",
            "25",
            "-D",
            "-",
            "-o",
            "/dev/null",
            "-H",
            &accept_header,
            "-H",
            &auth_header,
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("curl unavailable: {e}"))?;
    let headers = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        if err.contains("429") || headers.contains("429") {
            return Err("Docker Hub rate limit; try again later.".into());
        }
        if err.contains("401") || err.contains("403") || headers.contains("401") {
            return Err("Registry requires authentication.".into());
        }
        return Err(err.trim().chars().take(160).collect());
    }
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("docker-content-digest:") {
            let digest = line.split(':').skip(1).collect::<Vec<_>>().join(":").trim().to_string();
            if !digest.is_empty() {
                return Ok(digest);
            }
        }
    }
    Err("Registry response missing Docker-Content-Digest".into())
}

fn remote_digest(bin: &str, image_ref: &str) -> Result<String, String> {
    match manifest_digest(bin, image_ref) {
        Ok(d) => Ok(d),
        Err(cli_err) => {
            let normalized = normalize_container_image_ref(image_ref);
            if normalized.starts_with("docker.io/") || hub_repo_parts(image_ref).is_some() {
                hub_remote_digest_http(image_ref).map_err(|http_err| {
                    format!("{cli_err}; Hub HTTP fallback: {http_err}")
                        .chars()
                        .take(240)
                        .collect()
                })
            } else {
                Err(format!(
                    "Unsupported or private registry ({}). {}",
                    normalized.split('/').next().unwrap_or("unknown"),
                    cli_err.chars().take(120).collect::<String>()
                ))
            }
        }
    }
}

fn classify(local: &str, remote: &str) -> (UpdateCheckState, String) {
    let local_d = normalize_digest(local);
    let remote_d = normalize_digest(remote);
    match (local_d, remote_d) {
        (Some(l), Some(r)) if l == r => (
            UpdateCheckState::UpToDate,
            "Local image matches the registry digest for this tag.".into(),
        ),
        (Some(l), Some(r)) => (
            UpdateCheckState::UpdateAvailable,
            format!("Registry has a newer digest ({r}) than local ({l})."),
        ),
        (Some(l), None) => (
            UpdateCheckState::Unknown,
            format!("Local digest {l}; registry digest unavailable."),
        ),
        (None, Some(r)) => (
            UpdateCheckState::Unknown,
            format!("Registry digest {r}; local image has no repo digest."),
        ),
        (None, None) => (
            UpdateCheckState::Unknown,
            "Could not compare digests.".into(),
        ),
    }
}

fn snapshot_from_cache(entry: &CacheEntry) -> ImageUpdateSnapshot {
    ImageUpdateSnapshot {
        state: entry_state(&entry.state),
        detail: entry.detail.clone(),
        checked_at: Some(entry.checked_at),
    }
}

fn not_checked(reason: &str) -> ImageUpdateSnapshot {
    ImageUpdateSnapshot {
        state: UpdateCheckState::NotChecked,
        detail: reason.into(),
        checked_at: None,
    }
}

fn check_image_ref(image_ref: &str) -> ImageUpdateSnapshot {
    let Some(bin) = docker_bin() else {
        return ImageUpdateSnapshot {
            state: UpdateCheckState::Unknown,
            detail: "Docker/Podman CLI not found.".into(),
            checked_at: Some(now_unix()),
        };
    };
    let local = match local_digest(bin, image_ref) {
        Some(d) => d,
        None => {
            return ImageUpdateSnapshot {
                state: UpdateCheckState::Unknown,
                detail: "No local repo digest (custom build or dangling image).".into(),
                checked_at: Some(now_unix()),
            };
        }
    };
    let remote = match remote_digest(bin, image_ref) {
        Ok(d) => d,
        Err(e) => {
            let lower = e.to_ascii_lowercase();
            let detail = if lower.contains("rate limit") {
                "Registry rate limit; cached result kept or try later.".to_string()
            } else if lower.contains("authentication") || lower.contains("unauthorized") {
                "Private registry or authentication required.".to_string()
            } else {
                e
            };
            return ImageUpdateSnapshot {
                state: UpdateCheckState::Unknown,
                detail,
                checked_at: Some(now_unix()),
            };
        }
    };
    let (state, detail) = classify(&local, &remote);
    ImageUpdateSnapshot {
        state,
        detail,
        checked_at: Some(now_unix()),
    }
}

fn unique_image_refs(rows: &[DockerContainerRow]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        if let Some(r) = container_image_ref(row) {
            if seen.insert(r.clone()) {
                out.push(r);
            }
        }
    }
    out
}

/// Map each container id to an update snapshot (shared cache keyed by normalized image ref).
pub fn container_update_map(
    rows: &[DockerContainerRow],
    force_refresh: bool,
) -> HashMap<String, ImageUpdateSnapshot> {
    let mut cache = load_cache();
    let mut per_ref: HashMap<String, ImageUpdateSnapshot> = HashMap::new();

    if force_refresh {
        let mut checks = 0usize;
        for image_ref in unique_image_refs(rows) {
            let snap = if checks >= MAX_CHECKS_PER_REFRESH {
                ImageUpdateSnapshot {
                    state: UpdateCheckState::Unknown,
                    detail: format!(
                        "Skipped: more than {MAX_CHECKS_PER_REFRESH} unique images on this host."
                    ),
                    checked_at: Some(now_unix()),
                }
            } else {
                checks += 1;
                check_image_ref(&image_ref)
            };
            cache.entries.insert(
                image_ref.clone(),
                CacheEntry {
                    state: state_token(snap.state).to_string(),
                    detail: snap.detail.clone(),
                    local_digest: String::new(),
                    remote_digest: String::new(),
                    checked_at: snap.checked_at.unwrap_or_else(now_unix),
                },
            );
            per_ref.insert(image_ref, snap);
        }
        cache.schema = 1;
        let _ = save_cache(&cache);
    } else {
        for image_ref in unique_image_refs(rows) {
            let snap = if let Some(entry) = cache.entries.get(&image_ref) {
                if cache_fresh(entry) {
                    snapshot_from_cache(entry)
                } else {
                    not_checked("Cached result expired. Use Check for updates.")
                }
            } else {
                not_checked("Use Check for updates to query the registry.")
            };
            per_ref.insert(image_ref, snap);
        }
    }

    let mut out = HashMap::new();
    for row in rows {
        let snap = match container_image_ref(row) {
            None => ImageUpdateSnapshot {
                state: UpdateCheckState::Unknown,
                detail: "Image reference missing or untagged.".into(),
                checked_at: None,
            },
            Some(ref image_ref) => per_ref
                .get(image_ref.as_str())
                .cloned()
                .unwrap_or_else(|| not_checked("No registry data for this image.")),
        };
        out.insert(row.id.clone(), snap);
    }
    out
}

pub fn cache_age_note() -> String {
    format!(
        "Registry update checks are cached for up to {} hours. Pull or recreate still requires Manage Images or Compose Stacks.",
        CACHE_TTL_SECS / 3600
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_digest_variants() {
        let full = "sha256:abcd1234ef567890abcd1234ef567890abcd1234ef567890abcd1234ef567890";
        assert_eq!(
            normalize_digest(&format!("docker.io/library/nginx@{full}")),
            Some(full.into())
        );
        assert_eq!(normalize_digest(full), Some(full.into()));
        assert!(normalize_digest("").is_none());
    }

    #[test]
    fn hub_repo_parts_parses() {
        let parts = hub_repo_parts("nginx:alpine").expect("hub");
        assert_eq!(parts.0, "library/nginx");
        assert_eq!(parts.1, "alpine");
        let parts2 = hub_repo_parts("filebrowser/filebrowser:latest").expect("hub2");
        assert_eq!(parts2.0, "filebrowser/filebrowser");
    }

    #[test]
    fn classify_detects_update() {
        let local =
            "sha256:1111111111111111111111111111111111111111111111111111111111111111";
        let remote =
            "sha256:2222222222222222222222222222222222222222222222222222222222222222";
        let (st, _) = classify(local, remote);
        assert_eq!(st, UpdateCheckState::UpdateAvailable);
        let (st2, _) = classify(local, local);
        assert_eq!(st2, UpdateCheckState::UpToDate);
    }

    #[test]
    fn container_image_ref_skips_unknown() {
        let row = DockerContainerRow {
            id: "id".into(),
            name: "n".into(),
            image: "Unknown".into(),
            tag: "Unknown".into(),
            status: "".into(),
            owner: "".into(),
            cpn_managed: false,
            running: false,
            restarting: false,
        };
        assert!(container_image_ref(&row).is_none());
    }
}