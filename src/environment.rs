use crate::model::EnvironmentInfo;
use std::{net::UdpSocket, process::Stdio};
use tokio::process::Command;

#[cfg(not(windows))]
use std::{env, fs, path::Path};

async fn output(program: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !result.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&result.stdout).trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn primary_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

#[cfg(windows)]
async fn addresses() -> Vec<String> {
    let mut values = Vec::new();
    if let Some(ip) = primary_ip() {
        values.push(ip);
    }
    // Best-effort: PowerShell enumeration of IPv4 addresses.
    if let Some(raw) = output(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -notlike '127.*' }).IPAddress",
        ],
    )
    .await
    {
        for item in raw.split_whitespace() {
            if !item.starts_with("127.") && !item.contains(':') && !values.contains(&item.to_string())
            {
                values.push(item.to_string());
            }
        }
    }
    values.sort();
    values.dedup();
    values
}

#[cfg(not(windows))]
async fn addresses() -> Vec<String> {
    let raw = output("hostname", &["-I"]).await.unwrap_or_default();
    let mut values = raw
        .split_whitespace()
        .filter(|value| !value.starts_with("127.") && !value.contains(':'))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if let Some(ip) = primary_ip()
        && !values.contains(&ip)
    {
        values.insert(0, ip);
    }
    values.sort();
    values.dedup();
    values
}

#[cfg(not(windows))]
async fn active_service(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", name])
        .status()
        .await
        .is_ok_and(|status| status.success())
}

#[cfg(not(windows))]
fn container_marker_present() -> bool {
    Path::new("/.dockerenv").exists()
        || Path::new("/run/.containerenv").exists()
        || env::var_os("container").is_some()
}

#[cfg_attr(windows, allow(dead_code))]
fn virt_is_container(kind: &str) -> bool {
    matches!(
        kind,
        "docker" | "podman" | "lxc" | "containerd" | "crio" | "systemd-nspawn" | "wsl"
    ) || kind.contains("container")
}

#[cfg(test)]
mod tests {
    use super::virt_is_container;

    #[test]
    fn detects_common_container_virt_kinds() {
        assert!(virt_is_container("docker"));
        assert!(virt_is_container("podman"));
        assert!(virt_is_container("container-other"));
        assert!(!virt_is_container("kvm"));
        assert!(!virt_is_container("none"));
    }
}

#[cfg(windows)]
pub async fn inspect(port: u16) -> EnvironmentInfo {
    let mut addresses = addresses().await;
    if addresses.is_empty()
        && let Some(ip) = primary_ip()
    {
        addresses.push(ip);
    }
    EnvironmentInfo {
        is_vps: false,
        is_container: false,
        virtualization: Some("windows".into()),
        firewall: None,
        port,
        addresses,
    }
}

#[cfg(not(windows))]
pub async fn inspect(port: u16) -> EnvironmentInfo {
    let virtualization = output("systemd-detect-virt", &[]).await;
    let dmi = fs::read_to_string("/sys/class/dmi/id/product_name")
        .unwrap_or_default()
        .to_lowercase();
    let cloud_markers = [
        "kvm",
        "xen",
        "vmware",
        "virtualbox",
        "openstack",
        "digitalocean",
        "amazon",
        "google",
    ];
    let is_container =
        container_marker_present() || virtualization.as_deref().is_some_and(virt_is_container);
    let is_vps = virtualization.as_deref().is_some_and(|kind| kind != "none")
        || cloud_markers.iter().any(|marker| dmi.contains(marker));
    let firewall = if active_service("firewalld").await {
        Some("firewalld".into())
    } else if Command::new("ufw")
        .arg("status")
        .output()
        .await
        .is_ok_and(|result| {
            String::from_utf8_lossy(&result.stdout)
                .to_lowercase()
                .contains("status: active")
        })
    {
        Some("ufw".into())
    } else {
        None
    };
    EnvironmentInfo {
        is_vps,
        is_container,
        virtualization,
        firewall,
        port,
        addresses: addresses().await,
    }
}

pub async fn open_installer_port(environment: &EnvironmentInfo) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = environment;
        // Operators should open TCP 2087 (or chosen port) in Windows Firewall manually
        // or via packaging/windows/Install-Cpn.ps1. Avoid silent netsh mutations here.
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let port = format!("{}/tcp", environment.port);
        match environment.firewall.as_deref() {
            Some("firewalld") => {
                // Temporary runtime rule only (issue #1). Do not add --permanent.
                let status = Command::new("firewall-cmd")
                    .args(["--add-port", port.as_str()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err("firewalld no permitió abrir el puerto del instalador".into());
                }
            }
            Some("ufw") => {
                let status = Command::new("ufw")
                    .args(["allow", port.as_str()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err("ufw no permitió abrir el puerto del instalador".into());
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub async fn close_installer_port(environment: &EnvironmentInfo) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _ = environment;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let port = format!("{}/tcp", environment.port);
        match environment.firewall.as_deref() {
            Some("firewalld") => {
                let status = Command::new("firewall-cmd")
                    .args(["--remove-port", port.as_str()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err("firewalld no pudo cerrar el puerto del instalador".into());
                }
            }
            Some("ufw") => {
                let status = Command::new("ufw")
                    .args(["delete", "allow", port.as_str()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err("ufw no pudo cerrar el puerto del instalador".into());
                }
            }
            _ => {}
        }
        Ok(())
    }
}
