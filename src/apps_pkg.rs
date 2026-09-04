//! Shared dnf/apt and systemd helpers for host app recipes.

use std::process::Command;

pub fn package_manager() -> Result<&'static str, String> {
    if Command::new("dnf").arg("--version").status().is_ok() {
        return Ok("dnf");
    }
    if Command::new("apt-get").arg("--version").status().is_ok() {
        return Ok("apt");
    }
    Err("No supported package manager found (need dnf or apt-get).".into())
}

pub fn run_pkg(args: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    let status = if pm == "dnf" {
        Command::new("dnf")
            .args(args)
            .status()
            .map_err(|error| format!("Could not start dnf: {error}"))?
    } else {
        if args.first() == Some(&"install") || args.first() == Some(&"remove") {
            let update = Command::new("apt-get")
                .args(["update", "-y"])
                .status()
                .map_err(|error| format!("Could not start apt-get update: {error}"))?;
            if !update.success() {
                return Err("apt-get update failed".into());
            }
        }
        let mut apt_args: Vec<&str> = Vec::new();
        match args.first().copied() {
            Some("install") => {
                apt_args.push("install");
                apt_args.push("-y");
                apt_args.extend_from_slice(&args[1..]);
            }
            Some("remove") => {
                apt_args.push("remove");
                apt_args.push("-y");
                apt_args.extend_from_slice(&args[1..]);
            }
            _ => {
                apt_args.extend_from_slice(args);
            }
        }
        Command::new("apt-get")
            .args(&apt_args)
            .status()
            .map_err(|error| format!("Could not start apt-get: {error}"))?
    };
    if !status.success() {
        return Err(format!("{pm} {} failed", args.join(" ")));
    }
    Ok(())
}

pub fn enable_now(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let status = Command::new("systemctl")
            .args(["enable", "--now", unit])
            .status()
            .map_err(|error| format!("Could not start systemctl for {unit}: {error}"))?;
        if !status.success() {
            return Err(format!("systemctl enable --now {unit} failed"));
        }
    }
    Ok(())
}

pub fn disable_now(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let _ = Command::new("systemctl")
            .args(["disable", "--now", unit])
            .status();
    }
    Ok(())
}

pub fn start_units(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let status = Command::new("systemctl")
            .args(["start", unit])
            .status()
            .map_err(|error| format!("Could not start systemctl for {unit}: {error}"))?;
        if !status.success() {
            return Err(format!("systemctl start {unit} failed"));
        }
    }
    Ok(())
}

pub fn stop_units(units: &[&str]) -> Result<(), String> {
    for unit in units {
        let _ = Command::new("systemctl").args(["stop", unit]).status();
    }
    Ok(())
}

pub fn install_packages_dnf_or_apt(dnf_pkgs: &[&str], apt_pkgs: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    if pm == "dnf" {
        let mut args = vec!["install", "-y"];
        args.extend_from_slice(dnf_pkgs);
        run_pkg(&args)
    } else {
        let mut args = vec!["install"];
        args.extend_from_slice(apt_pkgs);
        run_pkg(&args)
    }
}

pub fn remove_packages_dnf_or_apt(dnf_pkgs: &[&str], apt_pkgs: &[&str]) -> Result<(), String> {
    let pm = package_manager()?;
    if pm == "dnf" {
        let mut args = vec!["remove", "-y"];
        args.extend_from_slice(dnf_pkgs);
        run_pkg(&args)
    } else {
        let mut args = vec!["remove"];
        args.extend_from_slice(apt_pkgs);
        run_pkg(&args)
    }
}

pub fn rpm_or_dpkg_installed(names: &[&str]) -> bool {
    use std::process::Stdio;
    for name in names {
        let rpm = Command::new("rpm")
            .args(["-q", name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if rpm {
            return true;
        }
        let dpkg = Command::new("dpkg-query")
            .args(["-W", "-f=${Status}", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()
            .map(|out| {
                let text = String::from_utf8_lossy(&out.stdout);
                text.contains("install ok installed")
            })
            .unwrap_or(false);
        if dpkg {
            return true;
        }
    }
    false
}
