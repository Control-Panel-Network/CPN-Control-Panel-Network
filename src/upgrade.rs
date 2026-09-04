//! Upgrade, downgrade, and repair operations for an existing CPN install.

use crate::installer::AppState;
use crate::manifest::{
    self, ManifestSource, core_paths_for_repair, detect_existing_install, installer_bin,
    preserve_paths_for_repair, record_install,
};
use crate::model::{MaintenanceAction, MaintenancePlan, MaintenanceRequest};
use crate::releases::{self, CpnRelease, compare_versions, normalize_version};
use rand::{Rng, distr::Alphanumeric};
use std::cmp::Ordering;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;

pub fn build_plan(
    action: MaintenanceAction,
    target_version: Option<&str>,
    installed_version: &str,
    reset_data: bool,
) -> MaintenancePlan {
    let overwrite = core_paths_for_repair();
    let preserve = preserve_paths_for_repair(reset_data);
    let target = target_version
        .map(normalize_version)
        .unwrap_or_else(|| installed_version.to_string());
    let summary = match action {
        MaintenanceAction::Upgrade => format!(
            "Upgrade CPN core packages from {installed_version} toward {target} (or latest)."
        ),
        MaintenanceAction::Downgrade => {
            format!("Downgrade CPN core packages from {installed_version} to {target}.")
        }
        MaintenanceAction::Repair => {
            format!("Repair/overwrite CPN core files using release {target}.")
        }
        MaintenanceAction::ConfigOnly => {
            "Continue configuration only. No core files will be overwritten.".into()
        }
    };
    MaintenancePlan {
        action,
        target_version: target,
        overwrite_paths: overwrite,
        preserve_paths: preserve,
        reset_data,
        summary,
    }
}

fn require_root() -> Result<(), String> {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return Err("Run as root (sudo cpn-installer --upgrade|--repair)".into());
        }
    }
    Ok(())
}

fn ephemeral_path(filename: &str) -> Result<String, String> {
    let suffix: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();
    let dir = format!("/var/tmp/cpn-upgrade-{suffix}");
    std::fs::create_dir_all(&dir).map_err(|error| format!("Could not create temp dir: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(format!("{dir}/{filename}"))
}

async fn download_file(url: &str, destination: &str) -> Result<(), String> {
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--output",
            destination,
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|error| format!("Download failed: {error}"))?;
    if !status.success() {
        return Err("Download of package asset failed".into());
    }
    Ok(())
}

async fn install_rpm(path: &str, force: bool) -> Result<(), String> {
    let mut args = vec!["install", "-y"];
    if force {
        args.push("--setopt=install_weak_deps=False");
        // reinstall/replace broken packages when repairing
        args.push("--allowerasing");
    }
    args.push(path);
    let status = Command::new("dnf")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|error| format!("dnf install failed: {error}"))?;
    if status.success() {
        return Ok(());
    }
    // Fallback for older hosts / repair of same NEVRA.
    let mut rpm_args = vec!["-Uvh"];
    if force {
        rpm_args.push("--force");
    }
    rpm_args.push(path);
    let status = Command::new("rpm")
        .args(&rpm_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|error| format!("rpm upgrade failed: {error}"))?;
    if !status.success() {
        return Err("Package install failed (dnf/rpm)".into());
    }
    Ok(())
}

async fn install_binary(path: &str) -> Result<(), String> {
    let dest = installer_bin();
    std::fs::copy(path, dest).map_err(|error| format!("Could not replace {dest}: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

async fn resolve_target_release(
    action: MaintenanceAction,
    requested: Option<&str>,
    installed_version: &str,
) -> Result<CpnRelease, String> {
    match action {
        MaintenanceAction::Upgrade => {
            if let Some(version) = requested {
                releases::find_release(version).await
            } else {
                let list = releases::list_releases(20).await?;
                list.iter()
                    .find(|release| !release.prerelease)
                    .cloned()
                    .or_else(|| list.first().cloned())
                    .ok_or_else(|| {
                        "No GitHub release found. Publish a release with RPM/binary assets, or set CPN_GITHUB_REPO.".into()
                    })
            }
        }
        MaintenanceAction::Downgrade | MaintenanceAction::Repair => {
            let version = requested
                .map(str::to_string)
                .unwrap_or_else(|| installed_version.to_string());
            releases::find_release(&version).await
        }
        MaintenanceAction::ConfigOnly => Err("config_only does not resolve a release".into()),
    }
}

async fn apply_release(
    state: &AppState,
    release: &CpnRelease,
    force: bool,
) -> Result<ManifestSource, String> {
    if let Some(rpm) = &release.rpm_asset {
        state
            .progress("downloading", 20, format!("Downloading {}", rpm.name))
            .await;
        let path = ephemeral_path(&rpm.name)?;
        download_file(&rpm.browser_download_url, &path).await?;
        state
            .progress("installing", 60, "Installing RPM package")
            .await;
        install_rpm(&path, force).await?;
        let _ = std::fs::remove_file(&path);
        return Ok(ManifestSource::Rpm);
    }
    if let Some(bin) = &release.binary_asset {
        state
            .progress("downloading", 20, format!("Downloading {}", bin.name))
            .await;
        let path = ephemeral_path("cpn-installer.bin")?;
        download_file(&bin.browser_download_url, &path).await?;
        state
            .progress("installing", 60, "Replacing cpn-installer binary")
            .await;
        install_binary(&path).await?;
        let _ = std::fs::remove_file(&path);
        return Ok(ManifestSource::Binary);
    }
    Err(format!(
        "Release {} has no cpn-installer RPM or binary asset. Lab fallback: build from git and install the RPM locally (see to-do/UPGRADE-REPAIR.md).",
        release.tag_name
    ))
}

fn maybe_reset_data(reset_data: bool) -> Result<(), String> {
    if !reset_data {
        return Ok(());
    }
    let root = manifest::data_dir();
    for name in [
        "panel-bootstrap.json",
        "accounts",
        "sites",
        "smtp.json",
        "smtp",
        "secrets",
    ] {
        let path = root.join(name);
        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .map_err(|error| format!("Could not reset {}: {error}", path.display()))?;
        } else if path.is_file() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("Could not reset {}: {error}", path.display()))?;
        }
    }
    Ok(())
}

pub async fn run_maintenance(
    state: Arc<AppState>,
    request: MaintenanceRequest,
) -> Result<(), String> {
    require_root()?;
    let existing = detect_existing_install(env!("CARGO_PKG_VERSION"));
    let installed = existing.package_version.clone();

    if matches!(request.action, MaintenanceAction::ConfigOnly) {
        state
            .progress("ready", 0, "Continuing configuration only")
            .await;
        let mut status = state.status.write().await;
        status.phase = if status
            .account
            .as_ref()
            .map(|a| a.configured)
            .unwrap_or(false)
            || status.server_ready
        {
            "completed"
        } else {
            "ready"
        };
        status.message = "Configuration mode: no package changes".into();
        status.error = None;
        let _ = state.events.send(crate::model::InstallerEvent::Completed {
            status: status.clone(),
        });
        return Ok(());
    }

    if matches!(request.action, MaintenanceAction::Downgrade) && !request.confirm_downgrade {
        return Err("Downgrade requires confirm_downgrade=true (or CLI --yes)".into());
    }

    let release =
        resolve_target_release(request.action, request.version.as_deref(), &installed).await?;

    if matches!(request.action, MaintenanceAction::Upgrade)
        && compare_versions(&release.version, &installed) == Ordering::Less
        && !request.confirm_downgrade
    {
        return Err(format!(
            "Target {} is older than installed {installed}. Use downgrade with confirmation.",
            release.version
        ));
    }

    let force = matches!(request.action, MaintenanceAction::Repair)
        || compare_versions(&release.version, &installed) != Ordering::Greater;

    state.log(
        format!(
            "Maintenance {:?}: installed={installed} target={}",
            request.action, release.tag_name
        ),
        "info",
    );
    state
        .progress("downloading", 5, format!("Preparing {}", release.tag_name))
        .await;

    maybe_reset_data(request.reset_data)?;
    let source = apply_release(&state, &release, force).await?;

    let status_snapshot = state.status.read().await.clone();
    record_install(
        &release.version,
        &release.tag_name,
        source,
        status_snapshot.selected_server.or(existing.selected_server),
        status_snapshot.selected_mail.or(existing.selected_mail),
    )?;

    // Best-effort: ensure binary path exists after package ops.
    if !Path::new(installer_bin()).exists() {
        state.log(
            format!(
                "Warning: {} missing after package operation",
                installer_bin()
            ),
            "error",
        );
    }

    state
        .progress("testing", 92, "Verifying installer binary")
        .await;
    let version_ok = Command::new(installer_bin())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false);
    if !version_ok {
        return Err("Post-maintenance version check failed".into());
    }

    let mut status = state.status.write().await;
    status.phase = "completed";
    status.progress = 100;
    status.error = None;
    status.message = format!(
        "Maintenance {:?} completed for {}",
        request.action, release.tag_name
    );
    if let Some(info) = status.maintenance.as_mut() {
        info.installed_version = release.version.clone();
        info.plan = Some(build_plan(
            request.action,
            Some(&release.version),
            &release.version,
            request.reset_data,
        ));
    }
    let _ = state.events.send(crate::model::InstallerEvent::Completed {
        status: status.clone(),
    });
    Ok(())
}

pub async fn spawn_maintenance(state: Arc<AppState>, request: MaintenanceRequest) {
    let label = format!("{:?}", request.action);
    let result = run_maintenance(state.clone(), request).await;
    if let Err(error) = result {
        let mut status = state.status.write().await;
        status.phase = "failed";
        status.error = Some(error.clone());
        status.message = format!("Maintenance {label} stopped safely");
        state.log(error, "error");
        let _ = state.events.send(crate::model::InstallerEvent::Error {
            status: status.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::build_plan;
    use crate::model::MaintenanceAction;

    #[test]
    fn plan_lists_preserve_bootstrap_by_default() {
        let plan = build_plan(MaintenanceAction::Repair, Some("0.2.0"), "0.2.0", false);
        assert!(
            plan.preserve_paths
                .iter()
                .any(|p| p.contains("panel-bootstrap"))
        );
        assert!(!plan.overwrite_paths.is_empty());
    }
}
