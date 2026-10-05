//! RPM/binary package apply helpers for upgrade/repair/retag.

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use tokio::process::Command;

fn cmd_failure_detail(tool: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = if !stderr.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    // Collapse whitespace so the Version Management UI stays readable.
    let flat: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = flat.chars().take(420).collect();
    if short.is_empty() {
        format!("Package install failed ({tool})")
    } else {
        format!("Package install failed ({tool}): {short}")
    }
}

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
    if nevra.is_empty() { None } else { Some(nevra) }
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
    if vr.is_empty() { None } else { Some(vr) }
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
        let output = Command::new("rpm")
            .args(["-Uvh", "--oldpackage", path])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|error| format!("rpm --oldpackage failed: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        let _ = Command::new("rpm")
            .args(["-e", "--nodeps", "cpn-installer"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        let output = Command::new("rpm")
            .args(["-Uvh", path])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|error| format!("rpm reinstall after erase failed: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        if let (Some(file_vr), Some(inst_vr)) = (rpm_query_vr(path).await, rpm_installed_vr().await)
            && file_vr == inst_vr
        {
            return Ok(());
        }
        return Err(format!(
            "{} (retag 1.0.x -> 0.2.x; rpm --oldpackage / erase+install)",
            cmd_failure_detail("rpm", &output)
        ));
    }

    let mut args = vec!["install", "-y"];
    if force {
        args.push("--setopt=install_weak_deps=False");
        args.push("--allowerasing");
    }
    args.push(path);
    let dnf_output = Command::new("dnf")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("dnf install failed: {error}"))?;
    if dnf_output.status.success() {
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
    let rpm_output = Command::new("rpm")
        .args(["-Uvh", "--force", path])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("rpm upgrade failed: {error}"))?;
    if rpm_output.status.success() {
        return Ok(());
    }
    if let (Some(file_vr), Some(inst_vr)) = (rpm_query_vr(path).await, rpm_installed_vr().await)
        && file_vr == inst_vr
    {
        return Ok(());
    }
    // Prefer dnf's message (usually the real conflict); fall back to rpm.
    let dnf_err = cmd_failure_detail("dnf", &dnf_output);
    let rpm_err = cmd_failure_detail("rpm", &rpm_output);
    if dnf_err.contains(':') {
        Err(format!("{dnf_err}; also {rpm_err}"))
    } else {
        Err(rpm_err)
    }
}

pub async fn install_binary(path: &str, dest: &str) -> Result<(), String> {
    if path == dest {
        return Ok(());
    }
    let dest_path = Path::new(dest);
    let name = dest_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cpn-bin");
    let parent = dest_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("/usr/bin"));
    let tmp: PathBuf = parent.join(format!(".{name}.cpn-new"));
    std::fs::copy(path, &tmp).map_err(|error| format!("Could not stage {dest}: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }
    // Rename over a running ELF. Copy-truncate of /usr/bin/cpn-installer
    // while it is executing produces "Exec format error" on the next start.
    std::fs::rename(&tmp, dest).map_err(|error| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not replace {dest}: {error}")
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::install_binary;
    use std::fs;
    use std::path::PathBuf;

    #[tokio::test]
    async fn atomic_replace_does_not_truncate_in_place() {
        let dir = std::env::temp_dir().join(format!("cpn-bin-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let src = dir.join("src.bin");
        let dest = dir.join("dest.bin");
        fs::write(&src, b"new-payload-bytes").unwrap();
        fs::write(&dest, b"old").unwrap();
        install_binary(src.to_str().unwrap(), dest.to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new-payload-bytes");
        let leftovers: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .contains("cpn-new")
            })
            .collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
