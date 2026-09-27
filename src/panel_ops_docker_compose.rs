//! CPN-managed Docker Compose projects under `<data>/docker/` with host data under `<data>/docker-data/`.

use crate::panel_ops_docker::docker_bin;
use crate::paths::join_data;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const CPN_MANAGED_LABEL: &str = "com.cpn.managed";
const COMPOSE_NAMES: [&str; 3] = ["compose.yml", "docker-compose.yml", "compose.yaml"];

#[derive(Debug, Clone)]
pub struct ComposeStackRow {
    pub id: String,
    pub compose_file: PathBuf,
    pub project_dir: PathBuf,
    pub data_dir: PathBuf,
    pub image_hint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialStackTemplate {
    Custom,
    Nginx,
    MariaDb,
    Redis,
}

impl OfficialStackTemplate {
    pub fn from_form(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "nginx" => Self::Nginx,
            "mariadb" => Self::MariaDb,
            "redis" => Self::Redis,
            _ => Self::Custom,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Custom => "Custom (you choose the image)",
            Self::Nginx => "Nginx (Docker Official Image)",
            Self::MariaDb => "MariaDB (maintainer image on Docker Hub)",
            Self::Redis => "Redis (Docker Official Image)",
        }
    }

    pub fn default_image(self) -> Option<&'static str> {
        match self {
            Self::Custom => None,
            Self::Nginx => Some("nginx:alpine"),
            Self::MariaDb => Some("mariadb:11"),
            Self::Redis => Some("redis:7-alpine"),
        }
    }

    pub fn default_container_data_path(self) -> &'static str {
        match self {
            Self::Nginx => "/usr/share/nginx/html",
            Self::MariaDb => "/var/lib/mysql",
            Self::Redis => "/data",
            Self::Custom => "/data",
        }
    }
}

pub fn compose_projects_root() -> PathBuf {
    join_data("docker")
}

pub fn compose_host_data_root() -> PathBuf {
    join_data("docker-data")
}

pub fn find_compose_file(dir: &Path) -> Option<PathBuf> {
    COMPOSE_NAMES
        .into_iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

pub fn list_compose_stacks() -> Result<Vec<ComposeStackRow>, String> {
    let root = compose_projects_root();
    let Ok(entries) = fs::read_dir(&root) else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    for entry in entries.flatten() {
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }
        let Some(compose_file) = find_compose_file(&project_dir) else {
            continue;
        };
        let id = project_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("stack")
            .to_string();
        let data_dir = compose_host_data_root().join(&id).join("data");
        let image_hint = read_compose_image_hint(&compose_file).unwrap_or_else(|| "—".into());
        rows.push(ComposeStackRow {
            id,
            compose_file,
            project_dir,
            data_dir,
            image_hint,
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(rows)
}

fn read_compose_image_hint(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("image:") {
            return Some(
                t.trim_start_matches("image:")
                    .trim()
                    .trim_matches('"')
                    .to_string(),
            );
        }
    }
    None
}

pub fn validate_stack_id(raw: &str) -> Result<String, String> {
    let id = raw.trim().to_ascii_lowercase();
    if id.is_empty() {
        return Err("Stack name is required.".into());
    }
    if id.len() > 48 {
        return Err("Stack name is too long (max 48).".into());
    }
    let ok = id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-');
    if !ok {
        return Err("Stack name may use lowercase letters, digits, and hyphens only.".into());
    }
    Ok(id)
}

fn validate_container_data_path(raw: &str) -> Result<String, String> {
    let p = raw.trim();
    if p.is_empty() {
        return Err("Container data path is required.".into());
    }
    if p.len() > 120 || !p.starts_with('/') || p.contains("..") || p.contains(':') {
        return Err("Container data path must be an absolute path inside the container (for example /data).".into());
    }
    Ok(p.to_string())
}

fn require_bin() -> Result<&'static str, String> {
    docker_bin().ok_or_else(|| "Docker/Podman CLI not found.".into())
}

fn compose_args(file: &Path, project_dir: &Path) -> Vec<String> {
    vec![
        "compose".into(),
        "-f".into(),
        file.to_string_lossy().into_owned(),
        "--project-directory".into(),
        project_dir.to_string_lossy().into_owned(),
    ]
}

pub fn run_compose(project_dir: &Path, subcommand: &[&str]) -> Result<String, String> {
    let Some(file) = find_compose_file(project_dir) else {
        return Err(format!("No compose file in {}.", project_dir.display()));
    };
    let bin = require_bin()?;
    let mut args = compose_args(&file, project_dir);
    for s in subcommand {
        args.push((*s).to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = Command::new(bin)
        .args(&arg_refs)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not run {bin} compose: {e}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() {
        let msg = if stdout.is_empty() { stderr } else { stdout };
        Ok(msg.chars().take(400).collect())
    } else {
        let detail = if stderr.is_empty() { stdout } else { stderr };
        Err(format!(
            "docker compose {} failed: {}",
            subcommand.first().unwrap_or(&""),
            detail.chars().take(320).collect::<String>()
        ))
    }
}

/// `docker compose pull` then `up -d` for one CPN stack (volumes preserved).
pub fn refresh_compose_stack(stack_id: &str) -> Result<String, String> {
    let id = validate_stack_id(stack_id)?;
    let project_dir = compose_projects_root().join(&id);
    if !project_dir.is_dir() {
        return Err(format!(
            "Stack `{id}` was not found under the CPN docker compose root."
        ));
    }
    let _ = run_compose(&project_dir, &["pull"]);
    run_compose(&project_dir, &["up", "-d", "--remove-orphans"]).map(|detail| {
        let mut msg = format!(
            "Stack `{id}` refreshed (pull + up -d). Data under {} is unchanged.",
            compose_host_data_root().join(&id).join("data").display()
        );
        if !detail.is_empty() {
            msg.push_str(&format!(" ({detail})"));
        }
        msg
    })
}

/// Used by upgrade `--bypass` and panel refresh-all.
pub fn refresh_all_cpn_compose_projects() -> Vec<String> {
    let mut notes = Vec::new();
    let stacks = list_compose_stacks().unwrap_or_default();
    if stacks.is_empty() {
        notes.push(
            "no CPN compose projects found under the CPN docker compose root (user stacks left untouched)"
                .into(),
        );
        return notes;
    }
    for row in stacks {
        notes.push(format!(
            "bypass: refreshing CPN compose project {}",
            row.project_dir.display()
        ));
        match refresh_compose_stack(&row.id) {
            Ok(msg) => notes.push(msg),
            Err(e) => notes.push(format!("refresh FAIL for `{}`: {e}", row.id)),
        }
    }
    notes
}

pub struct CreateComposeStackRequest<'a> {
    pub stack_id: &'a str,
    pub image: &'a str,
    pub ports: &'a str,
    pub env: &'a str,
    pub container_data_path: &'a str,
    pub template: OfficialStackTemplate,
    pub owner: &'a str,
}

pub fn create_compose_stack(req: CreateComposeStackRequest<'_>) -> Result<String, String> {
    let id = validate_stack_id(req.stack_id)?;
    let image = crate::panel_ops_docker_images::validate_image_ref(req.image)?;
    let data_path = validate_container_data_path(req.container_data_path)?;
    let ports = crate::panel_ops_docker_images::parse_port_mappings(req.ports)?;
    let envs = crate::panel_ops_docker_images::parse_env_vars(req.env)?;

    let project_dir = compose_projects_root().join(&id);
    if project_dir.exists() {
        return Err(format!(
            "Stack `{id}` already exists. Use Pull & Recreate on the existing stack or pick another name."
        ));
    }
    let host_data = compose_host_data_root().join(&id).join("data");
    fs::create_dir_all(&host_data).map_err(|e| format!("Could not create data directory: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&host_data, fs::Permissions::from_mode(0o750));
    }

    fs::create_dir_all(&project_dir)
        .map_err(|e| format!("Could not create stack directory: {e}"))?;

    let compose_path = project_dir.join("compose.yml");
    let compose_body = render_compose_yml(&image, &id, &host_data, &data_path, &ports, req.owner);
    fs::write(&compose_path, compose_body)
        .map_err(|e| format!("Could not write compose.yml: {e}"))?;

    if !envs.is_empty() {
        let mut env_lines = Vec::new();
        for (k, v) in &envs {
            env_lines.push(format!("{k}={v}"));
        }
        fs::write(project_dir.join(".env"), env_lines.join("\n") + "\n")
            .map_err(|e| format!("Could not write .env: {e}"))?;
    }

    let tpl_note = req.template.label();
    let readme = format!(
        "# CPN stack `{id}`\n\nTemplate: {tpl_note}\nImage: `{image}`\n\nPersistent host data: `{}`\n\nUpgrade from the panel: Docker > Compose Stacks > Pull & Recreate (same as `docker compose pull` then `docker compose up -d`).\n",
        host_data.display()
    );
    let _ = fs::write(project_dir.join("README.txt"), readme);

    run_compose(&project_dir, &["up", "-d", "--remove-orphans"])?;

    Ok(format!(
        "Created stack `{id}` with upstream image `{image}`. Data persists on the host at `{}`. Use Pull & Recreate after image updates.",
        host_data.display()
    ))
}

fn render_compose_yml(
    image: &str,
    stack_id: &str,
    host_data: &Path,
    container_data: &str,
    ports: &[String],
    owner: &str,
) -> String {
    let owner = owner.trim();
    let owner = if owner.is_empty() { "CPN" } else { owner };
    let mut out = String::from(
        "# CPN-managed compose. Host data survives docker compose pull && docker compose up -d.\n\nservices:\n  app:\n",
    );
    out.push_str(&format!("    image: {image}\n"));
    out.push_str("    restart: unless-stopped\n");
    if !ports.is_empty() {
        out.push_str("    ports:\n");
        for p in ports {
            out.push_str(&format!("      - \"{p}\"\n"));
        }
    }
    out.push_str("    volumes:\n");
    out.push_str(&format!(
        "      - type: bind\n        source: {}\n        target: {container_data}\n",
        host_data.display()
    ));
    out.push_str("    labels:\n");
    out.push_str(&format!("      {CPN_MANAGED_LABEL}: \"1\"\n"));
    out.push_str(&format!("      com.cpn.stack: \"{stack_id}\"\n"));
    out.push_str(&format!("      com.cpn.owner: \"{owner}\"\n"));
    out
}

/// List compose project directories (for uninstall/teardown).
pub fn cpn_compose_project_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let root = data_dir.join("docker");
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && find_compose_file(p).is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_id_validation() {
        assert!(validate_stack_id("my-app").is_ok());
        assert!(validate_stack_id("Bad").is_err());
        assert!(validate_stack_id("-x").is_err());
    }

    #[test]
    fn compose_yml_has_bind_and_labels() {
        let yml = render_compose_yml(
            "nginx:alpine",
            "demo",
            Path::new("/var/lib/cpn/docker-data/demo/data"),
            "/usr/share/nginx/html",
            &["8080:80".into()],
            "admin",
        );
        assert!(yml.contains("nginx:alpine"));
        assert!(yml.contains("com.cpn.managed"));
        assert!(yml.contains("bind"));
    }

    #[test]
    fn official_templates_use_upstream_images() {
        assert_eq!(
            OfficialStackTemplate::Nginx.default_image(),
            Some("nginx:alpine")
        );
        assert!(
            OfficialStackTemplate::MariaDb
                .default_image()
                .unwrap()
                .starts_with("mariadb:")
        );
    }
}
