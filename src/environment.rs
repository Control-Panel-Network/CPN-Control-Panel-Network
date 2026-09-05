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

#[cfg(not(windows))]
fn installer_firewall_marker(port: u16) -> std::path::PathBuf {
    crate::paths::join_data(format!("installer-firewall-{port}.owner"))
}

#[cfg(not(windows))]
fn read_firewall_owner(port: u16) -> Option<String> {
    fs::read_to_string(installer_firewall_marker(port))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(not(windows))]
fn write_firewall_owner(port: u16, owner: &str) -> Result<(), String> {
    let path = installer_firewall_marker(port);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create firewall marker directory: {error}"))?;
    }
    fs::write(&path, format!("{owner}\n"))
        .map_err(|error| format!("Could not record CPN firewall ownership: {error}"))
}

#[cfg(not(windows))]
fn clear_firewall_owner(port: u16) {
    let _ = fs::remove_file(installer_firewall_marker(port));
}

#[cfg(not(windows))]
fn ufw_status_allows_port(status: &str, port: u16) -> bool {
    let target = format!("{port}/tcp");
    status.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with(&target)
            && trimmed
                .split_whitespace()
                .any(|part| part.eq_ignore_ascii_case("ALLOW") || part.eq_ignore_ascii_case("ALLOW IN"))
    })
}

#[cfg(not(windows))]
async fn ufw_port_allowed(port: u16) -> bool {
    Command::new("ufw")
        .arg("status")
        .output()
        .await
        .is_ok_and(|result| ufw_status_allows_port(&String::from_utf8_lossy(&result.stdout), port))
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
                let already_open = Command::new("firewall-cmd")
                    .args(["--query-port", port.as_str()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);
                if already_open {
                    // An existing rule without our marker belongs to the operator.
                    return Ok(());
                }

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
                write_firewall_owner(environment.port, "firewalld")?;
            }
            Some("ufw") => {
                if ufw_port_allowed(environment.port).await {
                    return Ok(());
                }
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
                write_firewall_owner(environment.port, "ufw")?;
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
        let Some(owner) = read_firewall_owner(environment.port) else {
            // No ownership marker means the rule was pre-existing or no rule was added.
            return Ok(());
        };

        match owner.as_str() {
            "firewalld" => {
                let still_open = Command::new("firewall-cmd")
                    .args(["--query-port", port.as_str()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);
                if still_open {
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
                clear_firewall_owner(environment.port);
            }
            "ufw" => {
                if ufw_port_allowed(environment.port).await {
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
                clear_firewall_owner(environment.port);
            }
            _ => {
                return Err(format!(
                    "Unknown CPN firewall ownership marker for port {}: {owner}",
                    environment.port
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::virt_is_container;

    #[cfg(not(windows))]
    use super::ufw_status_allows_port;

    #[test]
    fn detects_common_container_virt_kinds() {
        assert!(virt_is_container("docker"));
        assert!(virt_is_container("podman"));
        assert!(virt_is_container("container-other"));
        assert!(!virt_is_container("kvm"));
        assert!(!virt_is_container("none"));
    }

    #[cfg(not(windows))]
    #[test]
    fn detects_existing_ufw_rule_without_matching_other_ports() {
        let status = "Status: active\n\nTo                         Action      From\n--                         ------      ----\n2087/tcp                   ALLOW       Anywhere\n2087/tcp (v6)              ALLOW       Anywhere (v6)\n";
        assert!(ufw_status_allows_port(status, 2087));
        assert!(!ufw_status_allows_port(status, 2088));
    }
}
