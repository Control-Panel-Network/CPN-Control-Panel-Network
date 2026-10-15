//! One-click Docker app stacks (n8n, Hermes Agent) with optional FQDN reverse-proxy + LE SSL.

use crate::panel_ops_docker_compose::{
    CreateComposeStackRequest, OfficialStackTemplate, compose_host_data_root,
    compose_projects_root, create_compose_stack, find_compose_file, list_compose_stacks,
    refresh_compose_stack, run_compose, validate_stack_id,
};
use crate::panel_ops_ssl_le::issue_lets_encrypt;
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OneclickAppRecord {
    pub stack_id: String,
    pub template: String,
    pub fqdn: String,
    pub host_port: u16,
    pub owner: String,
    pub proxy_configured: bool,
    pub ssl_configured: bool,
}

fn registry_path() -> PathBuf {
    join_data("docker-oneclick.json")
}

pub fn load_oneclick_registry() -> Vec<OneclickAppRecord> {
    let Ok(raw) = fs::read_to_string(registry_path()) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_oneclick_registry(rows: &[OneclickAppRecord]) -> Result<(), String> {
    if let Some(parent) = registry_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create data dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(rows).map_err(|e| e.to_string())?;
    let tmp = registry_path().with_extension("json.tmp");
    fs::write(&tmp, raw).map_err(|e| format!("Cannot write one-click registry: {e}"))?;
    fs::rename(&tmp, registry_path()).map_err(|e| format!("Cannot finalize registry: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(registry_path(), fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn parse_host_port(ports: &str) -> u16 {
    ports
        .split(',')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn sanitize_fqdn(raw: &str) -> Result<String, String> {
    let f = raw.trim().to_ascii_lowercase();
    if f.is_empty() {
        return Ok(String::new());
    }
    if f.len() > 253
        || !f
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        || f.starts_with('.')
        || f.ends_with('.')
        || f.contains("..")
    {
        return Err("FQDN must be a hostname (letters, digits, dots, hyphens).".into());
    }
    Ok(f)
}

/// Write a reverse-proxy snippet under CPN data for operators / OLS include.
fn write_proxy_snippet(fqdn: &str, host_port: u16) -> Result<PathBuf, String> {
    let dir = join_data("docker-proxy");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create proxy dir: {e}"))?;
    let path = dir.join(format!("{fqdn}.conf"));
    let body = format!(
        "# CPN one-click Docker proxy for {fqdn} -> 127.0.0.1:{host_port}\n\
         # Include this from the site vhost or OpenLiteSpeed external app mapping.\n\
         # Backend: http://127.0.0.1:{host_port}\n\
         proxy_pass http://127.0.0.1:{host_port};\n\
         proxy_set_header Host $host;\n\
         proxy_set_header X-Real-IP $remote_addr;\n\
         proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n\
         proxy_set_header X-Forwarded-Proto $scheme;\n\
         proxy_http_version 1.1;\n\
         proxy_set_header Upgrade $http_upgrade;\n\
         proxy_set_header Connection \"upgrade\";\n"
    );
    fs::write(&path, body).map_err(|e| format!("Cannot write proxy snippet: {e}"))?;
    Ok(path)
}

pub struct OneclickDeployRequest<'a> {
    pub template: OfficialStackTemplate,
    pub stack_id: &'a str,
    pub fqdn: &'a str,
    pub owner: &'a str,
}

pub fn deploy_oneclick_app(req: OneclickDeployRequest<'_>) -> Result<String, String> {
    if !req.template.is_oneclick_app() {
        return Err("Template is not a CPN one-click Docker app.".into());
    }
    let id = validate_stack_id(req.stack_id)?;
    let fqdn = sanitize_fqdn(req.fqdn)?;
    let image = req
        .template
        .default_image()
        .ok_or_else(|| "Missing default image for one-click app.".to_string())?;
    let ports = req.template.default_ports();
    let data_path = req.template.default_container_data_path();
    let env = match req.template {
        OfficialStackTemplate::N8n => {
            if fqdn.is_empty() {
                "N8N_HOST=0.0.0.0\nN8N_PORT=5678\nN8N_PROTOCOL=http"
            } else {
                // WEBHOOK_URL set after deploy when FQDN known; basic listen env here.
                "N8N_HOST=0.0.0.0\nN8N_PORT=5678\nN8N_PROTOCOL=https"
            }
        }
        OfficialStackTemplate::HermesAgent => "HERMES_HOST=0.0.0.0\nHERMES_PORT=8780",
        _ => "",
    };

    let msg = create_compose_stack(CreateComposeStackRequest {
        stack_id: &id,
        image,
        ports,
        env,
        container_data_path: data_path,
        template: req.template,
        owner: req.owner,
    })?;

    let host_port = parse_host_port(ports);
    let mut proxy_configured = false;
    let mut ssl_configured = false;
    let mut notes = vec![msg];

    if !fqdn.is_empty() && host_port > 0 {
        match write_proxy_snippet(&fqdn, host_port) {
            Ok(path) => {
                proxy_configured = true;
                notes.push(format!(
                    "Reverse-proxy snippet written to {} (map {fqdn} to 127.0.0.1:{host_port}).",
                    path.display()
                ));
            }
            Err(e) => notes.push(format!("Proxy snippet skipped: {e}")),
        }
        match issue_lets_encrypt(&fqdn) {
            Ok(ssl_msg) => {
                ssl_configured = true;
                notes.push(ssl_msg);
            }
            Err(e) => notes.push(format!(
                "Let's Encrypt for {fqdn} deferred: {e}. Issue SSL from Websites when DNS is ready."
            )),
        }
    }

    let mut rows = load_oneclick_registry();
    rows.retain(|r| r.stack_id != id);
    rows.push(OneclickAppRecord {
        stack_id: id.clone(),
        template: match req.template {
            OfficialStackTemplate::N8n => "n8n".into(),
            OfficialStackTemplate::HermesAgent => "hermes-agent".into(),
            _ => "custom".into(),
        },
        fqdn,
        host_port,
        owner: req.owner.trim().to_string(),
        proxy_configured,
        ssl_configured,
    });
    save_oneclick_registry(&rows)?;

    Ok(notes.join(" "))
}

pub fn oneclick_stack_action(stack_id: &str, action: &str) -> Result<String, String> {
    let id = validate_stack_id(stack_id)?;
    let project_dir = compose_projects_root().join(&id);
    if !project_dir.is_dir() || find_compose_file(&project_dir).is_none() {
        return Err(format!("Stack `{id}` was not found."));
    }
    match action.trim().to_ascii_lowercase().as_str() {
        "start" | "up" => run_compose(&project_dir, &["up", "-d", "--remove-orphans"])
            .map(|_| format!("Stack `{id}` started.")),
        "stop" | "down" => run_compose(&project_dir, &["stop"]).map(|_| format!("Stack `{id}` stopped.")),
        "refresh" => refresh_compose_stack(&id),
        _ => Err("Unknown action. Use start, stop, or refresh.".into()),
    }
}

pub fn open_url_for_stack(stack_id: &str) -> Option<String> {
    let id = stack_id.trim();
    load_oneclick_registry()
        .into_iter()
        .find(|r| r.stack_id == id)
        .and_then(|r| {
            if !r.fqdn.is_empty() {
                Some(format!("https://{}", r.fqdn))
            } else if r.host_port > 0 {
                Some(format!("http://127.0.0.1:{}", r.host_port))
            } else {
                None
            }
        })
}

pub fn ensure_docker_host_ready() -> Result<(), String> {
    if crate::panel_ops_docker::docker_bin().is_some() {
        return Ok(());
    }
    crate::panel_ops_docker::install_docker_engine().map(|_| ())
}

pub fn list_oneclick_with_live() -> Vec<(OneclickAppRecord, bool)> {
    let live: std::collections::HashSet<String> = list_compose_stacks()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.id)
        .collect();
    load_oneclick_registry()
        .into_iter()
        .map(|r| {
            let present = live.contains(&r.stack_id);
            (r, present)
        })
        .collect()
}

/// Best-effort compose provider check used by one-click UI.
pub fn compose_provider_hint() -> String {
    let candidates = ["docker", "podman"];
    for bin in candidates {
        if which(bin) {
            let out = Command::new(bin)
                .args(["compose", "version"])
                .output()
                .ok();
            if out.as_ref().map(|o| o.status.success()).unwrap_or(false) {
                return format!("{bin} compose available");
            }
        }
    }
    if which("docker-compose") || which("podman-compose") {
        return "legacy compose binary available".into();
    }
    format!(
        "Compose provider missing. Host data root: {}",
        compose_host_data_root().display()
    )
}

fn which(bin: &str) -> bool {
    #[cfg(windows)]
    {
        Command::new("where")
            .arg(bin)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Command::new("sh")
            .args(["-c", &format!("command -v {bin} >/dev/null 2>&1")])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fqdn_validation() {
        assert!(sanitize_fqdn("").unwrap().is_empty());
        assert_eq!(sanitize_fqdn("App.Example.COM").unwrap(), "app.example.com");
        assert!(sanitize_fqdn("bad host").is_err());
        assert!(sanitize_fqdn("../etc").is_err());
    }

    #[test]
    fn host_port_parse() {
        assert_eq!(parse_host_port("5678:5678"), 5678);
        assert_eq!(parse_host_port(""), 0);
    }
}
