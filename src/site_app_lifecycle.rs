//! Version discovery and backup-first lifecycle for website Apps.

use crate::apps::{AppId, AppStateKind, detect_app, install_app};
use crate::apps_pkg::{package_manager, run_pkg};
use crate::panel_site_apps_cmsms::{
    CMSMS_INSTALLER_URL, CMSMS_TARGET, detect_cmsms, install_cmsms_version,
};
use crate::panel_site_apps_runtime::{RuntimeKind, load_runtime, runtime_app_dir};
use crate::site_app_backups::{
    AppBackup, create_app_backup, create_host_app_backup, list_app_backups, list_host_app_backups,
    restore_app_backup, restore_host_app_backup,
};
use crate::sites::SiteRecord;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

type VersionCache =
    std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, Vec<String>)>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteAppId {
    Cmsms,
    Redis,
    Node,
    Python,
}

impl SiteAppId {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cmsms" | "cms-made-simple" => Ok(Self::Cmsms),
            "redis" => Ok(Self::Redis),
            "node" | "nodejs" => Ok(Self::Node),
            "python" | "python3" => Ok(Self::Python),
            _ => Err("Unknown site app. Use: cmsms, redis, node, python".into()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cmsms => "cmsms",
            Self::Redis => "redis",
            Self::Node => "node",
            Self::Python => "python",
        }
    }

    fn package(self) -> Option<&'static str> {
        match self {
            Self::Redis => Some("redis"),
            Self::Node => Some("nodejs"),
            Self::Python => Some("python3"),
            Self::Cmsms => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LifecycleStatus {
    pub app: SiteAppId,
    pub installed_version: String,
    pub latest_version: String,
    pub available_versions: Vec<String>,
    pub backups: Vec<AppBackup>,
    pub source: String,
}

fn output(bin: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    crate::panel_ops_docker_probe::command_output_with_timeout(cmd, timeout, bin)
        .ok()
        .filter(|result| result.status.success())
        .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn package_installed_version(package: &str) -> String {
    output(
        "rpm",
        &["-q", "--qf", "%{VERSION}-%{RELEASE}", package],
        Duration::from_secs(3),
    )
    .or_else(|| {
        output(
            "dpkg-query",
            &["-W", "-f=${Version}", package],
            Duration::from_secs(3),
        )
    })
    .unwrap_or_default()
}

fn package_versions(package: &str) -> Vec<String> {
    static CACHE: std::sync::OnceLock<VersionCache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some((saved, versions)) = guard.get(package)
        && saved.elapsed() < Duration::from_secs(300)
    {
        return versions.clone();
    }
    let mut versions = if package_manager().ok() == Some("dnf") {
        output(
            "dnf",
            &[
                "-q",
                "repoquery",
                "--available",
                "--latest-limit=10",
                "--qf",
                "%{VERSION}-%{RELEASE}",
                package,
            ],
            Duration::from_secs(5),
        )
    } else {
        output("apt-cache", &["madison", package], Duration::from_secs(4)).map(|raw| {
            raw.lines()
                .filter_map(|line| line.split('|').nth(1))
                .map(str::trim)
                .collect::<Vec<_>>()
                .join("\n")
        })
    }
    .unwrap_or_default()
    .lines()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
    .collect::<Vec<_>>();
    versions.sort_by(|a, b| version_cmp(b, a));
    versions.dedup();
    if let Ok(mut guard) = cache.lock() {
        guard.insert(
            package.to_string(),
            (std::time::Instant::now(), versions.clone()),
        );
    }
    versions
}

fn version_parts(value: &str) -> Vec<u64> {
    value
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

pub fn version_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    version_parts(left).cmp(&version_parts(right))
}

fn verify_cmsms_source() -> Result<(), String> {
    let status = Command::new("curl")
        .args([
            "-fsSIL",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            CMSMS_INSTALLER_URL,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not start curl: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("The official CMS Made Simple download source is currently unavailable".into())
    }
}

pub fn status(site: &SiteRecord, app: SiteAppId) -> LifecycleStatus {
    match app {
        SiteAppId::Cmsms => {
            let cms = detect_cmsms(site);
            LifecycleStatus {
                app,
                installed_version: cms.version,
                latest_version: CMSMS_TARGET.into(),
                available_versions: vec![CMSMS_TARGET.into()],
                backups: list_app_backups(site, app.as_str()).unwrap_or_default(),
                source: "Official CMS Made Simple installer".into(),
            }
        }
        SiteAppId::Redis | SiteAppId::Node | SiteAppId::Python => {
            let package = app.package().unwrap_or_default();
            let versions = package_versions(package);
            LifecycleStatus {
                app,
                installed_version: package_installed_version(package),
                latest_version: versions.first().cloned().unwrap_or_default(),
                available_versions: versions,
                backups: list_host_app_backups(app.as_str()).unwrap_or_default(),
                source: "Operating system package repositories".into(),
            }
        }
    }
}

fn runtime_source(site: &SiteRecord, app: SiteAppId) -> Result<std::path::PathBuf, String> {
    match app {
        SiteAppId::Node => runtime_app_dir(site, RuntimeKind::Node),
        SiteAppId::Python => runtime_app_dir(site, RuntimeKind::Python),
        SiteAppId::Cmsms => Ok(Path::new(&site.docroot).to_path_buf()),
        SiteAppId::Redis => Err("Redis uses host-level backups".into()),
    }
}

fn backup_before_change(site: &SiteRecord, app: SiteAppId) -> Result<AppBackup, String> {
    let state = status(site, app);
    match app {
        SiteAppId::Redis => {
            let _ = Command::new("redis-cli")
                .arg("SAVE")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            create_host_app_backup(
                app.as_str(),
                &state.installed_version,
                &["/etc/redis", "/etc/redis.conf", "/var/lib/redis"],
            )
        }
        SiteAppId::Node | SiteAppId::Python => {
            let source = runtime_source(site, app)?;
            if !source.is_dir() {
                std::fs::create_dir_all(&source)
                    .map_err(|e| format!("Could not create app directory before backup: {e}"))?;
            }
            create_app_backup(site, app.as_str(), &state.installed_version, &source)
        }
        SiteAppId::Cmsms => create_app_backup(
            site,
            app.as_str(),
            &state.installed_version,
            &runtime_source(site, app)?,
        ),
    }
}

fn install_package_version(
    app: SiteAppId,
    version: Option<&str>,
    downgrade: bool,
) -> Result<(), String> {
    let package = app
        .package()
        .ok_or_else(|| "This app does not use an OS package".to_string())?;
    let selected = version.map(str::trim).filter(|value| !value.is_empty());
    let mut owned = Vec::new();
    let spec = if let Some(version) = selected {
        if !package_versions(package).iter().any(|item| item == version) {
            return Err(format!(
                "{package} version {version} is not available from configured repositories"
            ));
        }
        if package_manager()? == "dnf" {
            format!("{package}-{version}")
        } else {
            format!("{package}={version}")
        }
    } else {
        package.to_string()
    };
    owned.push(spec);
    let args = if package_manager()? == "dnf" {
        vec![
            if downgrade { "downgrade" } else { "install" },
            "-y",
            owned[0].as_str(),
        ]
    } else if downgrade {
        vec!["install", "--allow-downgrades", owned[0].as_str()]
    } else {
        vec!["install", owned[0].as_str()]
    };
    run_pkg(&args)
}

pub fn install(site: &SiteRecord, app: SiteAppId, version: Option<&str>) -> Result<String, String> {
    match app {
        SiteAppId::Cmsms => {
            verify_cmsms_source()?;
            install_cmsms_version(site, version.unwrap_or(CMSMS_TARGET))
        }
        SiteAppId::Redis if version.map(str::trim).filter(|v| !v.is_empty()).is_some() => {
            install_package_version(app, version, false)?;
            let _ = crate::apps_redis::start_redis();
            Ok("Installed Redis from the selected operating system package source.".into())
        }
        SiteAppId::Redis => install_app(AppId::Redis),
        SiteAppId::Node | SiteAppId::Python => {
            install_package_version(app, version, false)?;
            Ok(format!(
                "Installed {} from the operating system package repository.",
                app.as_str()
            ))
        }
    }
}

pub fn update(site: &SiteRecord, app: SiteAppId, version: Option<&str>) -> Result<String, String> {
    change_version(site, app, version, false)
}

pub fn downgrade(site: &SiteRecord, app: SiteAppId, version: &str) -> Result<String, String> {
    change_version(site, app, Some(version), true)
}

fn change_version(
    site: &SiteRecord,
    app: SiteAppId,
    version: Option<&str>,
    downgrade: bool,
) -> Result<String, String> {
    let current = status(site, app);
    let target = version
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.latest_version);
    if target.is_empty() {
        return Err("No available version was returned by the original package source".into());
    }
    let backup = backup_before_change(site, app)?;
    let changed = match app {
        SiteAppId::Cmsms => {
            verify_cmsms_source()?;
            install_cmsms_version(site, target)?
        }
        SiteAppId::Redis | SiteAppId::Node | SiteAppId::Python => {
            install_package_version(app, Some(target), downgrade)?;
            format!("Installed {} {target}.", app.as_str())
        }
    };
    Ok(format!("{changed} Pre-update backup: {}", backup.id))
}

pub fn restore(site: &SiteRecord, app: SiteAppId, backup: Option<&str>) -> Result<String, String> {
    match app {
        SiteAppId::Redis => {
            let restored = restore_host_app_backup(app.as_str(), backup)?;
            let _ = Command::new("systemctl")
                .args(["restart", "redis"])
                .status();
            Ok(format!("Restored Redis backup {}.", restored.id))
        }
        SiteAppId::Cmsms | SiteAppId::Node | SiteAppId::Python => {
            let target = runtime_source(site, app)?;
            let was_running = match app {
                SiteAppId::Node => load_runtime(site, RuntimeKind::Node).running,
                SiteAppId::Python => load_runtime(site, RuntimeKind::Python).running,
                _ => false,
            };
            if was_running {
                let kind = if app == SiteAppId::Node {
                    RuntimeKind::Node
                } else {
                    RuntimeKind::Python
                };
                let _ = crate::panel_site_apps_runtime::stop_runtime(site, kind);
            }
            let restored = restore_app_backup(site, app.as_str(), backup, &target)?;
            if was_running {
                let kind = if app == SiteAppId::Node {
                    RuntimeKind::Node
                } else {
                    RuntimeKind::Python
                };
                crate::panel_site_apps_runtime::start_runtime(site, kind)?;
            }
            Ok(format!("Restored {} backup {}.", app.as_str(), restored.id))
        }
    }
}

pub fn has_update(value: &LifecycleStatus) -> bool {
    !value.installed_version.is_empty()
        && !value.latest_version.is_empty()
        && version_cmp(&value.latest_version, &value.installed_version).is_gt()
}

pub fn installed(site: &SiteRecord, app: SiteAppId) -> bool {
    match app {
        SiteAppId::Cmsms => detect_cmsms(site).installed,
        SiteAppId::Redis => detect_app(AppId::Redis).state != AppStateKind::NotInstalled,
        SiteAppId::Node => !load_runtime(site, RuntimeKind::Node).versions.is_empty(),
        SiteAppId::Python => !load_runtime(site, RuntimeKind::Python).versions.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(version_cmp("2.2.23", "2.2.9").is_gt());
        assert!(version_cmp("20.1.0-2", "18.20.4-1").is_gt());
        assert!(version_cmp("3.12.1", "3.12.1").is_eq());
    }

    #[test]
    fn app_ids_are_strict() {
        assert_eq!(SiteAppId::parse("cmsms").unwrap(), SiteAppId::Cmsms);
        assert!(SiteAppId::parse("../cmsms").is_err());
    }
}
