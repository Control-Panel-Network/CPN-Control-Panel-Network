//! RPM/binary package apply helpers for upgrade/repair/retag.

use std::process::Stdio;
use tokio::process::Command;

async fn rpm_query_nevra(path: &str) -> Option<String> {
    let output = Command::new("rpm")
        .args([
            "-qp",
            "--queryformat",
            "%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}",
            path,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let nevra = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if nevra.is_empty() {
        None
    } else {
        Some(nevra)
    }
}

async fn rpm_nevra_installed(nevra: &str) -> bool {
    Command::new("rpm")
        .args(["-q", nevra])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn rpm_query_vr(path: &str) -> Option<String> {
    let output = Command::new("rpm")
        .args(["-qp", "--qf", "%{VERSION}-%{RELEASE}", path])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let vr = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if vr.is_empty() {
        None
    } else {
        Some(vr)
    }
}

async fn rpm_installed_vr() -> Option<String> {
    let output = Command::new("rpm")
        .args(["-q", "--qf", "%{VERSION}-%{RELEASE}", "cpn-installer"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let vr = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if vr.is_empty() || vr.contains("not installed") {
        None
    } else {
        Some(vr)
    }
}

/// Install or replace the `cpn-installer` RPM.
///
/// When `allow_oldpackage` is true (retired `1.0.0`/`1.0.1` -> `0.2.x` retag),
/// uses `rpm -Uvh --oldpackage` with erase+install fallback.
pub async fn install_rpm(path: &str, force: bool, allow_oldpackage: bool) -> Result<(), String> {
    if let (Some(file_vr), Some(inst_vr)) = (rpm_query_vr(path).await, rpm_installed_vr().await)
        && file_vr == inst_vr
    {
        return Ok(());
    }
    if let Some(file_nevra) = rpm_query_nevra(path).await
        && rpm_nevra_installed(&file_nevra).await
    {
        return Ok(());
    }

    if allow_oldpackage {
        let status = Command::new("rpm")
            .args(["-Uvh", "--oldpackage", path])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .await
            .map_err(|error| format!("rpm --oldpackage failed: {error}"))?;
        if status.success() {
            return Ok(());
        }
        let _ = Command::new("rpm")
            .args(["-e", "--nodeps", "cpn-installer"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        let status = Command::new("rpm")
            .args(["-Uvh", path])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .await
            .map_err(|error| format!("rpm reinstall after erase failed: {error}"))?;
        if status.success() {
            return Ok(());
        }
        if let (Some(file_vr), Some(inst_vr)) =
            (rpm_query_vr(path).await, rpm_installed_vr().await)
            && file_vr == inst_vr
        {
            return Ok(());
        }
        return Err(
            "Package install failed (retag 1.0.x -> 0.2.x; rpm --oldpackage / erase+install)"
                .into(),
        );
    }

    let mut args = vec!["install", "-y"];
    if force {
        args.push("--setopt=install_weak_deps=False");
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
    if let (Some(file_vr), Some(inst_vr)) = (rpm_query_vr(path).await, rpm_installed_vr().await)
        && file_vr == inst_vr
    {
        return Ok(());
    }
    if let Some(file_nevra) = rpm_query_nevra(path).await
        && rpm_nevra_installed(&file_nevra).await
    {
        return Ok(());
    }
    let status = Command::new("rpm")
        .args(["-Uvh", "--force", path])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .await
        .map_err(|error| format!("rpm upgrade failed: {error}"))?;
    if status.success() {
        return Ok(());
    }
    if let (Some(file_vr), Some(inst_vr)) = (rpm_query_vr(path).await, rpm_installed_vr().await)
        && file_vr == inst_vr
    {
        return Ok(());
    }
    Err("Package install failed (dnf/rpm)".into())
}

pub async fn install_binary(path: &str, dest: &str) -> Result<(), String> {
    std::fs::copy(path, dest).map_err(|error| format!("Could not replace {dest}: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}
