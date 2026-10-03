//! Site-jailed Node.js and Python app config, versions, start/stop.

use crate::account::{data_dir, now_unix};
use crate::panel_ops_docker_probe::command_output_with_timeout;
use crate::panel_site_apps::jail_rel_under_home;
use crate::sites::{SiteRecord, site_home_from_record};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const PROBE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Node,
    Python,
}

impl RuntimeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Python => "python",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Node => "Node",
            Self::Python => "Python",
        }
    }

    pub fn default_rel(self) -> &'static str {
        match self {
            Self::Node => "apps/node",
            Self::Python => "apps/python",
        }
    }

    pub fn default_entry(self) -> &'static str {
        match self {
            Self::Node => "app.js",
            Self::Python => "app.py",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub version_bin: String,
    pub app_rel: String,
    pub entry: String,
}

impl RuntimeConfig {
    fn defaults(kind: RuntimeKind) -> Self {
        Self {
            version_bin: String::new(),
            app_rel: kind.default_rel().into(),
            entry: kind.default_entry().into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SiteRuntimeFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    node: Option<RuntimeConfig>,
    #[serde(default)]
    python: Option<RuntimeConfig>,
}

#[derive(Debug, Clone)]
pub struct RuntimeStatus {
    pub kind: RuntimeKind,
    pub config: RuntimeConfig,
    pub versions: Vec<(String, String)>,
    pub running: bool,
    pub detail: String,
}

fn state_path(domain: &str) -> PathBuf {
    data_dir().join("site-apps").join(format!("{domain}.json"))
}

fn load_file(domain: &str) -> SiteRuntimeFile {
    let Ok(raw) = fs::read_to_string(state_path(domain)) else {
        return SiteRuntimeFile {
            schema_version: 1,
            node: None,
            python: None,
        };
    };
    serde_json::from_str(&raw).unwrap_or(SiteRuntimeFile {
        schema_version: 1,
        node: None,
        python: None,
    })
}

fn save_file(domain: &str, file: &SiteRuntimeFile) -> Result<(), String> {
    let path = state_path(domain);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create site-apps dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(file)
        .map_err(|e| format!("Could not serialize site app state: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write site app state: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let _ = now_unix();
    Ok(())
}

fn unit_name(kind: RuntimeKind, domain: &str) -> String {
    let mut h: u32 = 2166136261;
    for b in domain.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(16777619);
    }
    format!("cpn-{}-{:08x}", kind.as_str(), h)
}

fn probe_bin(bin: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    let out = command_output_with_timeout(cmd, PROBE, bin).ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        None
    } else {
        Some(line.chars().take(48).collect())
    }
}

fn discover_versions(kind: RuntimeKind) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let candidates: &[&str] = match kind {
        RuntimeKind::Node => &[
            "/usr/bin/node",
            "/usr/bin/nodejs",
            "/usr/local/bin/node",
            "node",
        ],
        RuntimeKind::Python => &[
            "/usr/bin/python3",
            "/usr/bin/python3.12",
            "/usr/bin/python3.11",
            "/usr/bin/python3.9",
            "/usr/local/bin/python3",
            "python3",
        ],
    };
    let flag = match kind {
        RuntimeKind::Node => "--version",
        RuntimeKind::Python => "--version",
    };
    for bin in candidates {
        if let Some(ver) = probe_bin(bin, &[flag]) {
            if out.iter().any(|(p, _)| p == bin) {
                continue;
            }
            out.push(((*bin).to_string(), ver));
        }
    }
    out
}

fn unit_active(name: &str) -> bool {
    crate::service_detect::systemd_unit_active(name)
}

pub fn load_runtime(site: &SiteRecord, kind: RuntimeKind) -> RuntimeStatus {
    let file = load_file(&site.domain);
    let config = match kind {
        RuntimeKind::Node => file.node.unwrap_or_else(|| RuntimeConfig::defaults(kind)),
        RuntimeKind::Python => file.python.unwrap_or_else(|| RuntimeConfig::defaults(kind)),
    };
    let versions = discover_versions(kind);
    let running = unit_active(&unit_name(kind, &site.domain));
    let host = if versions.is_empty() {
        format!("{} is not installed on this host.", kind.label())
    } else {
        format!(
            "Host {} binaries: {}.",
            kind.label(),
            versions
                .iter()
                .map(|(p, v)| format!("{v} ({p})"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let detail = if running {
        format!("{} app is running. {host}", kind.label())
    } else {
        format!("{} app is stopped. {host}", kind.label())
    };
    RuntimeStatus {
        kind,
        config,
        versions,
        running,
        detail,
    }
}

pub fn save_runtime(
    site: &SiteRecord,
    kind: RuntimeKind,
    version_bin: &str,
    app_rel: &str,
    entry: &str,
) -> Result<String, String> {
    let home = site_home_from_record(site);
    let _ = jail_rel_under_home(&home, app_rel)?;
    if entry.trim().is_empty()
        || entry.contains("..")
        || entry.contains('/')
        || entry.contains('\\')
    {
        return Err("Entry file must be a single name under the app path".into());
    }
    let versions = discover_versions(kind);
    let bin = version_bin.trim();
    if !bin.is_empty() && !versions.iter().any(|(p, _)| p == bin) {
        return Err(format!(
            "{} binary is not available on this host",
            kind.label()
        ));
    }
    let mut file = load_file(&site.domain);
    let cfg = RuntimeConfig {
        version_bin: bin.to_string(),
        app_rel: app_rel.trim().to_string(),
        entry: entry.trim().to_string(),
    };
    match kind {
        RuntimeKind::Node => file.node = Some(cfg),
        RuntimeKind::Python => file.python = Some(cfg),
    }
    file.schema_version = 1;
    save_file(&site.domain, &file)?;
    Ok(format!("Saved {} app path for this site.", kind.label()))
}

fn resolve_bin(kind: RuntimeKind, preferred: &str) -> Result<String, String> {
    let versions = discover_versions(kind);
    if versions.is_empty() {
        return Err(format!(
            "{} is not installed on this host. Install it on the server, then return here.",
            kind.label()
        ));
    }
    if !preferred.is_empty() {
        if versions.iter().any(|(p, _)| p == preferred) {
            return Ok(preferred.to_string());
        }
    }
    Ok(versions[0].0.clone())
}

fn ensure_app_dir(
    site: &SiteRecord,
    kind: RuntimeKind,
    cfg: &RuntimeConfig,
) -> Result<PathBuf, String> {
    let home = site_home_from_record(site);
    let dir = jail_rel_under_home(&home, &cfg.app_rel)?;
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create app path: {e}"))?;
    let entry = dir.join(&cfg.entry);
    if !entry.is_file() {
        let stub = match kind {
            RuntimeKind::Node => {
                "const http = require('http');\nconst port = process.env.PORT || 3000;\nhttp.createServer((_, res) => { res.writeHead(200, {'Content-Type':'text/plain'}); res.end('CPN Node app');\n}).listen(port);\n"
            }
            RuntimeKind::Python => "print('CPN Python app ready')\n",
        };
        fs::write(&entry, stub).map_err(|e| format!("Could not write default entry: {e}"))?;
    }
    Ok(dir)
}

pub fn start_runtime(site: &SiteRecord, kind: RuntimeKind) -> Result<String, String> {
    let st = load_runtime(site, kind);
    if st.running {
        return Ok(format!("{} app is already running.", kind.label()));
    }
    let bin = resolve_bin(kind, &st.config.version_bin)?;
    let dir = ensure_app_dir(site, kind, &st.config)?;
    let unit = unit_name(kind, &site.domain);
    let _ = Command::new("systemctl")
        .args(["stop", &unit])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let mut cmd = Command::new("systemd-run");
    cmd.args([
        "--unit",
        &unit,
        "--collect",
        "--working-directory",
        &dir.display().to_string(),
        "--",
        &bin,
        &st.config.entry,
    ]);
    if kind == RuntimeKind::Python {
        let venv_py = dir.join("venv").join("bin").join("python");
        if venv_py.is_file() {
            cmd = Command::new("systemd-run");
            cmd.args([
                "--unit",
                &unit,
                "--collect",
                "--working-directory",
                &dir.display().to_string(),
                "--",
                &venv_py.display().to_string(),
                &st.config.entry,
            ]);
        }
    }
    let status = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Could not start {}: {e}", kind.label()))?;
    if !status.success() {
        return Err(format!(
            "Could not start {} (systemd-run failed). Check that systemd is available.",
            kind.label()
        ));
    }
    Ok(format!("Started {} app for this site.", kind.label()))
}

pub fn stop_runtime(site: &SiteRecord, kind: RuntimeKind) -> Result<String, String> {
    let unit = unit_name(kind, &site.domain);
    let status = Command::new("systemctl")
        .args(["stop", &unit])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not stop {}: {e}", kind.label()))?;
    if !status.success() {
        return Err(format!(
            "Could not stop {} (unit may already be inactive).",
            kind.label()
        ));
    }
    Ok(format!("Stopped {} app for this site.", kind.label()))
}

pub fn ensure_python_venv(site: &SiteRecord) -> Result<String, String> {
    let st = load_runtime(site, RuntimeKind::Python);
    let bin = resolve_bin(RuntimeKind::Python, &st.config.version_bin)?;
    let dir = ensure_app_dir(site, RuntimeKind::Python, &st.config)?;
    let venv = dir.join("venv");
    if venv.join("bin").join("python").is_file() {
        return Ok(format!("Python venv already exists at {}", venv.display()));
    }
    let status = Command::new(&bin)
        .args(["-m", "venv", &venv.display().to_string()])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Could not create venv: {e}"))?;
    if !status.success() {
        return Err("Could not create a Python venv under the site home.".into());
    }
    Ok(format!("Created Python venv at {}", venv.display()))
}

pub fn versions_for(kind: RuntimeKind) -> Vec<(String, String)> {
    discover_versions(kind)
}
