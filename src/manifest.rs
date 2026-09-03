//! Install manifest: installed version, core file list, and preserve paths.

use crate::model::{MailSystem, ServerEngine};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const DATA_DIR: &str = "/var/lib/cpn";
pub const BOOTSTRAP_FILE: &str = "/var/lib/cpn/panel-bootstrap.json";
pub const INSTALLER_BIN: &str = "/usr/bin/cpn-installer";
pub const CPN_CLI_BIN: &str = "/usr/bin/cpn";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestSource {
    Rpm,
    Binary,
    Local,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreFileEntry {
    pub path: String,
    pub kind: String,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallManifest {
    pub schema_version: u32,
    pub package_version: String,
    #[serde(default)]
    pub release_tag: String,
    pub installed_at_unix: u64,
    pub source: ManifestSource,
    pub core_files: Vec<CoreFileEntry>,
    #[serde(default)]
    pub preserve_paths: Vec<String>,
    #[serde(default)]
    pub selected_server: Option<ServerEngine>,
    #[serde(default)]
    pub selected_mail: Option<MailSystem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExistingInstall {
    pub detected: bool,
    pub package_version: String,
    pub release_tag: String,
    pub source: String,
    pub has_manifest: bool,
    pub has_bootstrap: bool,
    pub binary_present: bool,
    pub selected_server: Option<ServerEngine>,
    pub selected_mail: Option<MailSystem>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("CPN_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DATA_DIR))
}

pub fn manifest_path() -> PathBuf {
    data_dir().join("install-manifest.json")
}

pub fn default_preserve_paths() -> Vec<String> {
    let root = data_dir();
    vec![
        root.join("panel-bootstrap.json")
            .to_string_lossy()
            .into_owned(),
        root.join("accounts").to_string_lossy().into_owned(),
        root.join("sites").to_string_lossy().into_owned(),
        root.join("smtp.json").to_string_lossy().into_owned(),
        root.join("smtp").to_string_lossy().into_owned(),
        root.join("secrets").to_string_lossy().into_owned(),
    ]
}

pub fn default_core_files() -> Vec<CoreFileEntry> {
    vec![
        CoreFileEntry {
            path: INSTALLER_BIN.into(),
            kind: "binary".into(),
            optional: false,
        },
        CoreFileEntry {
            path: CPN_CLI_BIN.into(),
            kind: "binary".into(),
            optional: true,
        },
        CoreFileEntry {
            path: "/usr/lib/systemd/system/cpn-installer.service".into(),
            kind: "unit".into(),
            optional: true,
        },
        CoreFileEntry {
            path: "/etc/systemd/system/cpn-installer.service".into(),
            kind: "unit".into(),
            optional: true,
        },
        CoreFileEntry {
            path: "/etc/systemd/system/cpn-webmail.service".into(),
            kind: "unit".into(),
            optional: true,
        },
        CoreFileEntry {
            path: "/etc/systemd/system/openlitespeed.service".into(),
            kind: "unit".into(),
            optional: true,
        },
    ]
}

pub fn load_manifest() -> Option<InstallManifest> {
    let raw = fs::read_to_string(manifest_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_manifest(manifest: &InstallManifest) -> Result<(), String> {
    let dir = data_dir();
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create {}: {error}", dir.display()))?;
    let path = manifest_path();
    let body = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("Could not serialize install manifest: {error}"))?;
    fs::write(&path, format!("{body}\n"))
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

pub fn record_install(
    package_version: &str,
    release_tag: &str,
    source: ManifestSource,
    selected_server: Option<ServerEngine>,
    selected_mail: Option<MailSystem>,
) -> Result<InstallManifest, String> {
    let previous = load_manifest();
    let mut core_files = previous
        .as_ref()
        .map(|item| item.core_files.clone())
        .unwrap_or_else(default_core_files);
    if core_files.is_empty() {
        core_files = default_core_files();
    }
    let preserve_paths = previous
        .as_ref()
        .map(|item| item.preserve_paths.clone())
        .filter(|paths| !paths.is_empty())
        .unwrap_or_else(default_preserve_paths);
    let manifest = InstallManifest {
        schema_version: 1,
        package_version: package_version.trim().trim_start_matches('v').to_string(),
        release_tag: if release_tag.trim().is_empty() {
            format!("v{}", package_version.trim().trim_start_matches('v'))
        } else {
            release_tag.trim().to_string()
        },
        installed_at_unix: now_unix(),
        source,
        core_files,
        preserve_paths,
        selected_server: selected_server
            .or_else(|| previous.as_ref().and_then(|item| item.selected_server)),
        selected_mail: selected_mail
            .or_else(|| previous.as_ref().and_then(|item| item.selected_mail)),
    };
    save_manifest(&manifest)?;
    Ok(manifest)
}

fn rpm_installed_version() -> Option<String> {
    let output = std::process::Command::new("rpm")
        .args(["-q", "--qf", "%{VERSION}", "cpn-installer"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() || version.contains("not installed") {
        None
    } else {
        Some(version)
    }
}

pub fn detect_existing_install(running_version: &str) -> ExistingInstall {
    let manifest = load_manifest();
    let has_bootstrap =
        Path::new(BOOTSTRAP_FILE).is_file() || data_dir().join("panel-bootstrap.json").is_file();
    let binary = Path::new(INSTALLER_BIN).is_file();
    let rpm_version = rpm_installed_version();
    let has_manifest = manifest.is_some();
    let detected = has_manifest || has_bootstrap || binary || rpm_version.is_some();

    let package_version = manifest
        .as_ref()
        .map(|item| item.package_version.clone())
        .or(rpm_version)
        .unwrap_or_else(|| running_version.to_string());
    let release_tag = manifest
        .as_ref()
        .map(|item| item.release_tag.clone())
        .filter(|tag| !tag.is_empty())
        .unwrap_or_else(|| format!("v{package_version}"));
    let source = manifest
        .as_ref()
        .map(|item| match item.source {
            ManifestSource::Rpm => "rpm",
            ManifestSource::Binary => "binary",
            ManifestSource::Local => "local",
            ManifestSource::Unknown => "unknown",
        })
        .unwrap_or(if binary { "rpm_or_binary" } else { "unknown" })
        .to_string();

    ExistingInstall {
        detected,
        package_version,
        release_tag,
        source,
        has_manifest,
        has_bootstrap,
        binary_present: binary,
        selected_server: manifest.as_ref().and_then(|item| item.selected_server),
        selected_mail: manifest.as_ref().and_then(|item| item.selected_mail),
    }
}

pub fn preserve_paths_for_repair(reset_data: bool) -> Vec<String> {
    if reset_data {
        Vec::new()
    } else {
        load_manifest()
            .map(|item| item.preserve_paths)
            .filter(|paths| !paths.is_empty())
            .unwrap_or_else(default_preserve_paths)
    }
}

pub fn core_paths_for_repair() -> Vec<String> {
    load_manifest()
        .map(|item| {
            item.core_files
                .into_iter()
                .map(|entry| entry.path)
                .collect()
        })
        .unwrap_or_else(|| {
            default_core_files()
                .into_iter()
                .map(|entry| entry.path)
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_lists_are_non_empty() {
        assert!(!default_core_files().is_empty());
        assert!(!default_preserve_paths().is_empty());
        assert!(
            default_preserve_paths()
                .iter()
                .any(|path| path.contains("panel-bootstrap"))
        );
    }
}
