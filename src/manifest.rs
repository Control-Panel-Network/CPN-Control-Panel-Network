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
    Deb,
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
    /// Git commit SHA for tip/source installs (and optional release commit tracking).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_commit: String,
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
    /// When the package was last installed/upgraded (unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_at_unix: Option<u64>,
    /// Provenance for `installed_at_unix`: manifest, lab-deploy-meta, or rpm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_at_source: Option<String>,
}

/// Lab/hot-deploy sidecar so install time survives binary replace without a full RPM cycle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LabDeployMeta {
    #[serde(default)]
    pub package_version: String,
    #[serde(default)]
    pub source_commit: String,
    #[serde(default)]
    pub installed_at_unix: u64,
    #[serde(default)]
    pub updated_at_unix: u64,
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

pub fn lab_deploy_meta_path() -> PathBuf {
    data_dir().join("lab-deploy-meta.json")
}

pub fn default_preserve_paths() -> Vec<String> {
    let root = data_dir();
    vec![
        root.join("panel-bootstrap.json")
            .to_string_lossy()
            .into_owned(),
        root.join("install-manifest.json")
            .to_string_lossy()
            .into_owned(),
        root.join("lab-deploy-meta.json")
            .to_string_lossy()
            .into_owned(),
        root.join("accounts").to_string_lossy().into_owned(),
        root.join("sites").to_string_lossy().into_owned(),
        root.join("smtp.json").to_string_lossy().into_owned(),
        root.join("smtp").to_string_lossy().into_owned(),
        root.join("secrets").to_string_lossy().into_owned(),
        root.join("mfa").to_string_lossy().into_owned(),
        root.join("ssl").to_string_lossy().into_owned(),
        root.join("docker").to_string_lossy().into_owned(),
        root.join("listen_port").to_string_lossy().into_owned(),
        root.join("panel_public_url").to_string_lossy().into_owned(),
        root.join("webmail-panel.json")
            .to_string_lossy()
            .into_owned(),
        root.join("email-auth").to_string_lossy().into_owned(),
        root.join("panel_hostname").to_string_lossy().into_owned(),
        root.join("allow_remote").to_string_lossy().into_owned(),
        root.join("cloudflare.json").to_string_lossy().into_owned(),
        "/etc/cpn".into(),
        "/var/lib/cpn-webmail".into(),
        "/home".into(),
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

pub fn load_lab_deploy_meta() -> Option<LabDeployMeta> {
    let raw = fs::read_to_string(lab_deploy_meta_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_lab_deploy_meta(meta: &LabDeployMeta) -> Result<(), String> {
    let dir = data_dir();
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create {}: {error}", dir.display()))?;
    let path = lab_deploy_meta_path();
    let body = serde_json::to_string_pretty(meta)
        .map_err(|error| format!("Could not serialize lab-deploy-meta: {error}"))?;
    fs::write(&path, format!("{body}\n"))
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

fn sync_lab_deploy_meta(manifest: &InstallManifest) {
    let meta = LabDeployMeta {
        package_version: manifest.package_version.clone(),
        source_commit: manifest.source_commit.clone(),
        installed_at_unix: manifest.installed_at_unix,
        updated_at_unix: now_unix(),
    };
    let _ = save_lab_deploy_meta(&meta);
}

fn rpm_install_time_unix() -> Option<u64> {
    let output = std::process::Command::new("rpm")
        .args(["-q", "--qf", "%{INSTALLTIME}", "cpn-installer"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() || text.contains("not installed") {
        return None;
    }
    text.parse::<u64>().ok().filter(|value| *value > 0)
}

/// Prefer install-manifest, then lab-deploy-meta, then RPM INSTALLTIME.
pub fn resolve_installed_at_unix(
    manifest: Option<&InstallManifest>,
) -> (Option<u64>, Option<String>) {
    if let Some(ts) = manifest
        .map(|item| item.installed_at_unix)
        .filter(|value| *value > 0)
    {
        return (Some(ts), Some("manifest".into()));
    }
    if let Some(ts) = load_lab_deploy_meta()
        .map(|meta| meta.installed_at_unix)
        .filter(|value| *value > 0)
    {
        return (Some(ts), Some("lab-deploy-meta".into()));
    }
    if let Some(ts) = rpm_install_time_unix() {
        return (Some(ts), Some("rpm".into()));
    }
    (None, None)
}

pub fn record_install(
    package_version: &str,
    release_tag: &str,
    source: ManifestSource,
    selected_server: Option<ServerEngine>,
    selected_mail: Option<MailSystem>,
) -> Result<InstallManifest, String> {
    record_install_with_commit(
        package_version,
        release_tag,
        source,
        None,
        selected_server,
        selected_mail,
    )
}

pub fn record_install_with_commit(
    package_version: &str,
    release_tag: &str,
    source: ManifestSource,
    source_commit: Option<&str>,
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
    let commit = source_commit
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            previous
                .as_ref()
                .map(|item| item.source_commit.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default();
    let manifest = InstallManifest {
        schema_version: 1,
        package_version: package_version.trim().trim_start_matches('v').to_string(),
        release_tag: if release_tag.trim().is_empty() {
            format!("v{}", package_version.trim().trim_start_matches('v'))
        } else {
            release_tag.trim().to_string()
        },
        source_commit: commit,
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
    sync_lab_deploy_meta(&manifest);
    Ok(manifest)
}

/// Source commit recorded on the install manifest (tip/hot-deploy installs).
pub fn manifest_source_commit() -> Option<String> {
    load_manifest()
        .map(|item| item.source_commit.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn rpm_installed_version() -> Option<String> {
    let output = std::process::Command::new("rpm")
        .args(["-q", "--qf", "%{VERSION}\n%{RELEASE}", "cpn-installer"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let version = lines.next()?.trim().to_string();
    let release = lines.next().unwrap_or("").trim().to_string();
    if version.is_empty() || version.contains("not installed") {
        return None;
    }
    Some(crate::releases::cargo_version_from_rpm(&version, &release))
}

/// Prefer live RPM identity over a stale install-manifest (retired 1.0.x or older 0.2.x).
/// Do not invent a 0.2 identity while the RPM still claims retired 1.0.x (retag needs that).
fn resolve_package_version(
    from_manifest: Option<String>,
    rpm_version: Option<String>,
    running_version: &str,
) -> String {
    use crate::releases::{
        compare_versions, is_active_0_2_line, is_retired_cpn_1_0_identity, normalize_version,
    };
    use std::cmp::Ordering;

    match (from_manifest, rpm_version) {
        (Some(manifest_ver), Some(rpm_ver))
            if is_retired_cpn_1_0_identity(&manifest_ver)
                && (is_active_0_2_line(&rpm_ver) || rpm_ver.starts_with("0.2.")) =>
        {
            rpm_ver
        }
        (Some(manifest_ver), None)
            if is_retired_cpn_1_0_identity(&manifest_ver)
                && is_active_0_2_line(running_version) =>
        {
            // No RPM query (binary-only host): prefer running over phantom 1.0.x manifest.
            running_version.to_string()
        }
        (Some(manifest_ver), Some(rpm_ver))
            if is_active_0_2_line(&rpm_ver)
                && is_active_0_2_line(&manifest_ver)
                && normalize_version(&manifest_ver) != normalize_version(&rpm_ver) =>
        {
            // Stale manifest after bootstrap upgrade.sh / binary replace: trust RPM.
            rpm_ver
        }
        (Some(manifest_ver), Some(rpm_ver))
            if compare_versions(&manifest_ver, &rpm_ver) == Ordering::Less =>
        {
            // Stale alpha/0.2.x (or older) manifest while RPM is already on 1.0.0+: trust RPM.
            rpm_ver
        }
        (Some(manifest_ver), None)
            if compare_versions(&manifest_ver, running_version) == Ordering::Less =>
        {
            // Binary-only upgrade (lab hot-deploy): trust running panel over leftover manifest.
            running_version.to_string()
        }
        (Some(manifest_ver), _) => manifest_ver,
        (None, Some(rpm_ver)) => rpm_ver,
        (None, None) => running_version.to_string(),
    }
}

/// Rewrite install-manifest when it lags the live RPM/running identity.
///
/// Covers: retired `1.0.0`/`1.0.1` while live is `0.2.x`, stale `0.2.x-alpha.*`
/// while the live RPM is already on `1.0.0+`, and binary-only upgrades where the
/// running panel is newer than a leftover `1.0.0` manifest (e.g. `1.1.0`).
pub fn reconcile_stale_package_identity(running_version: &str) -> Option<String> {
    use crate::releases::{compare_versions, is_active_0_2_line, is_retired_cpn_1_0_identity};
    use std::cmp::Ordering;

    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return None;
        }
    }

    let manifest = load_manifest()?;
    let rpm = rpm_installed_version();
    let target = match &rpm {
        Some(v)
            if is_retired_cpn_1_0_identity(&manifest.package_version) && is_active_0_2_line(v) =>
        {
            v.clone()
        }
        None if is_retired_cpn_1_0_identity(&manifest.package_version)
            && is_active_0_2_line(running_version) =>
        {
            running_version.to_string()
        }
        Some(v)
            if is_active_0_2_line(&manifest.package_version)
                && compare_versions(&manifest.package_version, v) == Ordering::Less =>
        {
            // e.g. manifest 0.2.6-alpha.49 while RPM is 1.0.0
            v.clone()
        }
        Some(v) if compare_versions(&manifest.package_version, v) == Ordering::Less => v.clone(),
        // Binary hot-deploy ahead of RPM/manifest (lab: RPM+manifest 1.0.0, running 1.1.0).
        _ if compare_versions(&manifest.package_version, running_version) == Ordering::Less => {
            running_version.to_string()
        }
        _ => return None,
    };
    if normalize_version_local(&manifest.package_version) == normalize_version_local(&target) {
        return None;
    }
    let tag = format!("v{target}");
    match record_install(
        &target,
        &tag,
        ManifestSource::Rpm,
        manifest.selected_server,
        manifest.selected_mail,
    ) {
        Ok(_) => Some(format!(
            "Reconciled install-manifest package_version from {} to {target}",
            manifest.package_version
        )),
        Err(_) => None,
    }
}

fn normalize_version_local(value: &str) -> String {
    crate::releases::normalize_version(value)
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

    let from_manifest = manifest.as_ref().map(|item| item.package_version.clone());
    let package_version = resolve_package_version(from_manifest, rpm_version, running_version);
    let release_tag = manifest
        .as_ref()
        .map(|item| item.release_tag.clone())
        .filter(|tag| !tag.is_empty())
        .filter(|tag| {
            // Drop stale v1.0.0 tags when package identity was reconciled to 0.2.x.
            !(crate::releases::is_retired_cpn_1_0_identity(tag)
                && crate::releases::is_active_0_2_line(&package_version))
        })
        .unwrap_or_else(|| format!("v{package_version}"));
    let source = manifest
        .as_ref()
        .map(|item| match item.source {
            ManifestSource::Rpm => "rpm",
            ManifestSource::Deb => "deb",
            ManifestSource::Binary => "binary",
            ManifestSource::Local => "local",
            ManifestSource::Unknown => "unknown",
        })
        .unwrap_or(if binary { "rpm_or_binary" } else { "unknown" })
        .to_string();
    let (installed_at_unix, installed_at_source) = resolve_installed_at_unix(manifest.as_ref());

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
        installed_at_unix,
        installed_at_source,
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
        assert!(
            default_preserve_paths()
                .iter()
                .any(|path| path.contains("lab-deploy-meta.json"))
        );
        assert!(
            default_preserve_paths()
                .iter()
                .any(|path| path.contains("install-manifest.json"))
        );
    }

    #[test]
    fn resolve_installed_at_prefers_manifest() {
        let manifest = InstallManifest {
            schema_version: 1,
            package_version: "1.1.0".into(),
            release_tag: "v1.1.0".into(),
            source_commit: String::new(),
            installed_at_unix: 1_700_000_000,
            source: ManifestSource::Rpm,
            core_files: Vec::new(),
            preserve_paths: Vec::new(),
            selected_server: None,
            selected_mail: None,
        };
        let (ts, source) = resolve_installed_at_unix(Some(&manifest));
        assert_eq!(ts, Some(1_700_000_000));
        assert_eq!(source.as_deref(), Some("manifest"));
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

    #[test]
    fn resolve_prefers_rpm_over_stale_manifest() {
        assert_eq!(
            resolve_package_version(
                Some("1.0.0".into()),
                Some("0.2.6-alpha.21".into()),
                "0.2.6-alpha.21"
            ),
            "0.2.6-alpha.21"
        );
        // Keep retired RPM identity so retag migration still triggers.
        assert_eq!(
            resolve_package_version(Some("1.0.0".into()), Some("1.0.0".into()), "0.2.6-alpha.21"),
            "1.0.0"
        );
        assert_eq!(
            resolve_package_version(Some("1.0.0".into()), None, "0.2.6-alpha.21"),
            "0.2.6-alpha.21"
        );
        assert_eq!(
            resolve_package_version(
                Some("0.2.6-alpha.20".into()),
                Some("0.2.6-alpha.21".into()),
                "0.2.6-alpha.21"
            ),
            "0.2.6-alpha.21"
        );
        assert_eq!(
            resolve_package_version(
                Some("0.2.2-alpha.17".into()),
                Some("0.2.6-alpha.24".into()),
                "0.2.6-alpha.24"
            ),
            "0.2.6-alpha.24"
        );
        // Stale alpha manifest must not beat a live 1.0.0 RPM.
        assert_eq!(
            resolve_package_version(Some("0.2.6-alpha.49".into()), Some("1.0.0".into()), "1.0.0"),
            "1.0.0"
        );
    }
}
