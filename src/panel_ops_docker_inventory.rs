//! Container and image inventory for `/docker/list` and `/docker/images`.
//!
//! Resolves real image/tag metadata (including Podman ID-only `ps` rows) and maps
//! owner labels to Host / Unassigned / account names.

use crate::panel_ops_docker_probe::{
    INFO_TIMEOUT, LIST_TIMEOUT, VERSION_TIMEOUT, output_with_timeout, probe_ok,
};

fn docker_bin() -> Option<&'static str> {
    ["docker", "podman"]
        .into_iter()
        .find(|&candidate| probe_ok(candidate, &["--version"], VERSION_TIMEOUT))
}

fn docker_daemon_ok(bin: &str) -> bool {
    probe_ok(bin, &["info"], INFO_TIMEOUT)
}

const CPN_MANAGED_LABEL: &str = "com.cpn.managed";
const MAX_LIST_ROWS: usize = 500;

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
    pub restarting: bool,
}

#[derive(Debug, Clone)]
pub struct DockerImageRow {
    pub repository: String,
    pub tag: String,
    pub id: String,
    pub size: String,
}

pub(crate) fn split_image_tag(image: &str) -> (String, String) {
    let image = image.trim();
    if image.is_empty()
        || image.eq_ignore_ascii_case("<none>")
        || image.eq_ignore_ascii_case("unknown")
        || image.eq_ignore_ascii_case("<no value>")
    {
        return ("Unknown".into(), "Unknown".into());
    }
    if let Some(slash) = image.rfind('/') {
        let after = &image[slash + 1..];
        if let Some(colon) = after.rfind(':') {
            let repo = format!("{}{}", &image[..=slash], &after[..colon]);
            let tag = after[colon + 1..].to_string();
            return (repo, if tag.is_empty() { "latest".into() } else { tag });
        }
    } else if let Some(colon) = image.rfind(':') {
        if !image[..colon].contains('@') && !image.starts_with("sha256:") {
            return (image[..colon].to_string(), image[colon + 1..].to_string());
        }
    }
    (image.to_string(), "latest".into())
}

fn label_map(raw: &str) -> Vec<(String, String)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    if raw.starts_with("map[") && raw.ends_with(']') {
        let inner = raw[4..raw.len() - 1].trim();
        return inner
            .split_whitespace()
            .filter_map(|pair| {
                let pair = pair.trim();
                if pair.is_empty() {
                    return None;
                }
                let (k, v) = pair.split_once(':')?;
                Some((k.trim().to_string(), v.trim().to_string()))
            })
            .collect();
    }
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

/// Owner column: account name from labels, else Host (managed/system), else Unassigned.
pub fn owner_from_labels(labels: &[(String, String)], cpn_managed: bool) -> String {
    if let Some((_, v)) = labels
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("com.cpn.owner") || k.eq_ignore_ascii_case("owner"))
    {
        let v = v.trim();
        if !v.is_empty() && !v.eq_ignore_ascii_case("unknown") {
            if v.eq_ignore_ascii_case("cpn") || v.eq_ignore_ascii_case("system") {
                return "Host".into();
            }
            return v.to_string();
        }
    }
    if labels.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("com.cpn.system") && (v == "1" || v.eq_ignore_ascii_case("true"))
    }) || cpn_managed
    {
        return "Host".into();
    }
    "Unassigned".into()
}

fn image_ref_looks_incomplete(raw: &str) -> bool {
    let t = raw.trim();
    if t.is_empty()
        || t.eq_ignore_ascii_case("<none>")
        || t.eq_ignore_ascii_case("unknown")
        || t.eq_ignore_ascii_case("<no value>")
    {
        return true;
    }
    let base = t.split('@').next().unwrap_or(t);
    let name = base.split(':').next().unwrap_or(base);
    !name.contains('/')
        && name.len() >= 12
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_hexdigit())
}

fn inspect_image_ref(bin: &str, id_or_name: &str) -> Option<String> {
    let formats = [
        "{{.Config.Image}}",
        "{{.ImageName}}",
        "{{index .Config.Labels \"org.opencontainers.image.ref.name\"}}",
        "{{.Image}}",
    ];
    for fmt in formats {
        let output =
            output_with_timeout(bin, &["inspect", "--format", fmt, id_or_name], LIST_TIMEOUT)
                .ok()?;
        if !output.status.success() {
            continue;
        }
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.is_empty()
            || s.eq_ignore_ascii_case("<none>")
            || s.eq_ignore_ascii_case("<no value>")
            || s.eq_ignore_ascii_case("unknown")
        {
            continue;
        }
        if image_ref_looks_incomplete(&s) {
            continue;
        }
        return Some(s);
    }
    None
}

fn resolve_image_tag(bin: &str, container_id: &str, raw_image: &str) -> (String, String) {
    let mut candidate = raw_image.trim().to_string();
    if image_ref_looks_incomplete(&candidate) {
        if let Some(resolved) = inspect_image_ref(bin, container_id) {
            candidate = resolved;
        }
    }
    let (image, tag) = split_image_tag(&candidate);
    if image.eq_ignore_ascii_case("unknown") {
        if let Some(resolved) = inspect_image_ref(bin, container_id) {
            return split_image_tag(&resolved);
        }
    }
    (image, tag)
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
    let output = output_with_timeout(
        bin,
        &[
            "ps",
            "-a",
            "--format",
            "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Labels}}\t{{.State}}",
        ],
        LIST_TIMEOUT,
    )
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
        let (image, tag) = resolve_image_tag(bin, &id, parts[2].trim());
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
        let status_lower = status.to_ascii_lowercase();
        let restarting = state == "restarting" || status_lower.contains("restarting");
        let running = !restarting
            && (state == "running"
                || status_lower.starts_with("up ")
                || status_lower.contains("(healthy)"));
        rows.push(DockerContainerRow {
            id,
            name: if name.is_empty() {
                "(unnamed)".into()
            } else {
                name
            },
            image,
            tag,
            status,
            owner: owner_from_labels(&labels, cpn_managed),
            cpn_managed,
            running,
            restarting,
        });
        if rows.len() >= MAX_LIST_ROWS {
            break;
        }
    }
    Ok(rows)
}

pub fn list_images_detailed() -> Result<Vec<DockerImageRow>, String> {
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    if !docker_daemon_ok(bin) {
        return Err(format!(
            "`{bin}` CLI found but the container engine is not running."
        ));
    }
    let output = output_with_timeout(
        bin,
        &[
            "images",
            "--format",
            "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.Size}}",
        ],
        LIST_TIMEOUT,
    )
    .map_err(|e| format!("Could not list images: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Could not list images: {}",
            err.trim().chars().take(200).collect::<String>()
        ));
    }
    let mut rows = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 {
            continue;
        }
        let repository = parts[0].trim().to_string();
        let tag = parts[1].trim().to_string();
        let id = parts[2].trim().to_string();
        let size = parts[3].trim().to_string();
        if repository.is_empty() && tag.is_empty() && id.is_empty() {
            continue;
        }
        rows.push(DockerImageRow {
            repository: if repository.is_empty() {
                "<none>".into()
            } else {
                repository
            },
            tag: if tag.is_empty() { "<none>".into() } else { tag },
            id,
            size,
        });
        if rows.len() >= MAX_LIST_ROWS {
            break;
        }
    }
    Ok(rows)
}

/// Filter rows by a case-insensitive substring across name/image/tag/owner/status.
pub fn filter_containers<'a>(
    rows: &'a [DockerContainerRow],
    query: &str,
) -> Vec<&'a DockerContainerRow> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return rows.iter().collect();
    }
    rows.iter()
        .filter(|row| {
            [
                row.name.as_str(),
                row.image.as_str(),
                row.tag.as_str(),
                row.owner.as_str(),
                row.status.as_str(),
                row.id.as_str(),
            ]
            .iter()
            .any(|field| field.to_ascii_lowercase().contains(&q))
        })
        .collect()
}

pub fn filter_images<'a>(rows: &'a [DockerImageRow], query: &str) -> Vec<&'a DockerImageRow> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return rows.iter().collect();
    }
    rows.iter()
        .filter(|row| {
            [
                row.repository.as_str(),
                row.tag.as_str(),
                row.id.as_str(),
                row.size.as_str(),
            ]
            .iter()
            .any(|field| field.to_ascii_lowercase().contains(&q))
        })
        .collect()
}

pub fn docker_list_per_page(raw: &str) -> usize {
    match raw.trim().parse::<usize>().unwrap_or(10) {
        n @ (5 | 10 | 25 | 50) => n,
        _ => 10,
    }
}

pub fn docker_list_page(raw: &str) -> usize {
    raw.trim().parse::<usize>().unwrap_or(1).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn split_unknown_image() {
        let (r, t) = split_image_tag("");
        assert_eq!(r, "Unknown");
        assert_eq!(t, "Unknown");
        let (r2, t2) = split_image_tag("<none>");
        assert_eq!(r2, "Unknown");
        assert_eq!(t2, "Unknown");
    }

    #[test]
    fn owner_labels_host_unassigned() {
        assert_eq!(
            owner_from_labels(&[("com.cpn.owner".into(), "cpnowner".into())], false),
            "cpnowner"
        );
        assert_eq!(owner_from_labels(&[], true), "Host");
        assert_eq!(owner_from_labels(&[], false), "Unassigned");
        assert_eq!(
            owner_from_labels(&[("com.cpn.system".into(), "1".into())], false),
            "Host"
        );
        assert_eq!(
            owner_from_labels(&[("com.cpn.owner".into(), "CPN".into())], false),
            "Host"
        );
        assert_eq!(
            owner_from_labels(&[("com.cpn.owner".into(), "unknown".into())], false),
            "Unassigned"
        );
    }

    #[test]
    fn label_map_podman_ps_format() {
        let raw = "map[com.cpn.owner:cpnowner org.opencontainers.image.name:filebrowser]";
        let labels = label_map(raw);
        assert_eq!(owner_from_labels(&labels, false), "cpnowner");
    }

    #[test]
    fn incomplete_image_detection() {
        assert!(image_ref_looks_incomplete(""));
        assert!(image_ref_looks_incomplete("<none>"));
        assert!(image_ref_looks_incomplete("a1b2c3d4e5f67890"));
        assert!(!image_ref_looks_incomplete("nginx:alpine"));
        assert!(!image_ref_looks_incomplete(
            "docker.io/library/nginx:latest"
        ));
    }

    #[test]
    fn filter_and_paging_defaults() {
        assert_eq!(docker_list_per_page(""), 10);
        assert_eq!(docker_list_per_page("10"), 10);
        assert_eq!(docker_list_per_page("7"), 10);
        assert_eq!(docker_list_page("0"), 1);
        assert_eq!(docker_list_page("3"), 3);
    }
}
