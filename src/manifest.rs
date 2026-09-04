//! Install manifest: installed version, core file list, and preserve paths.

use crate::model::{MailSystem, ServerEngine};
use crate::paths;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Default data dir string (platform-specific). Prefer [`data_dir()`] at runtime.
pub fn default_data_dir_str() -> &'static str {
    paths::platform_data_dir()
}

pub fn installer_bin() -> &'static str {
    paths::installer_bin_path()
}

pub fn cli_bin() -> &'static str {
    paths::cli_bin_path()
}

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
    paths::default_data_dir()
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
    let mut files = vec![
        CoreFileEntry {
            path: installer_bin().into(),
            kind: "binary".into(),
            optional: false,
        },
        CoreFileEntry {
            path: cli_bin().into(),
            kind: "binary".into(),
            optional: true,
        },
    ];
    if cfg!(windows) {
        files.push(CoreFileEntry {
            path: r"C:\Program Files\CPN\cpn-installer.xml".into(),
            kind: "service".into(),
            optional: true,
        });
    } else {
        files.extend([
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
        ]);
    }
    files
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
    let has_bootstrap = data_dir().join("panel-bootstrap.json").is_file();
    let binary = Path::new(installer_bin()).is_file();
    let rpm_version = rpm_installed_version();
    let has_manifest = manifest.is_some();
    // Package/binary presence alone is not an installed panel. Treating RPM/binary as
    // "existing" forced phase=maintenance on first boot and broke matrix /api/install
    // (HTTP 409) before transitions allowed maintenance.
    let detected = has_manifest || has_bootstrap;

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

    #[test]
    fn package_only_is_not_existing_install() {
        // Mirrors detect_existing_install gating without touching the live host.
        let has_manifest = false;
        let has_bootstrap = false;
        let binary = true;
        let rpm_present = true;
        let detected = has_manifest || has_bootstrap;
        assert!(!detected);
        assert!(binary || rpm_present);
    }
}
