use crate::model::EnvironmentInfo;
use std::{fs, net::UdpSocket, process::Stdio};
use tokio::process::Command;

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

async fn active_service(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", name])
        .status()
        .await
        .is_ok_and(|status| status.success())
}

pub async fn inspect(port: u16, remote_access: bool) -> EnvironmentInfo {
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
        virtualization,
        firewall,
        port,
        addresses: addresses().await,
        remote_access,
    }
}

pub struct FirewallGuard {
    firewall: Option<String>,
    port: u16,
    added: bool,
}

impl FirewallGuard {
    pub async fn cleanup(self) {
        if !self.added {
            return;
        }
        let port = format!("{}/tcp", self.port);
        match self.firewall.as_deref() {
            Some("firewalld") => {
                let _ = Command::new("firewall-cmd")
                    .args(["--remove-port", &port])
                    .status()
                    .await;
            }
            Some("ufw") => {
                let _ = Command::new("ufw")
                    .args(["delete", "allow", &port])
                    .status()
                    .await;
            }
            _ => {}
        }
    }
}

pub async fn open_installer_port(environment: &EnvironmentInfo) -> Result<FirewallGuard, String> {
    let port = format!("{}/tcp", environment.port);
    let mut added = false;
    match environment.firewall.as_deref() {
        Some("firewalld") => {
            let existed = Command::new("firewall-cmd")
                .args(["--query-port", &port])
                .status()
                .await
                .is_ok_and(|status| status.success());
            if !existed {
                added = true;
                let args = vec!["--add-port", port.as_str()];
                let status = Command::new("firewall-cmd")
                    .args(args)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err("firewalld no permitió abrir el puerto del instalador".into());
                }
            }
        }
        Some("ufw") => {
            let listing = output("ufw", &["status"]).await.unwrap_or_default();
            if !listing
                .lines()
                .any(|line| line.split_whitespace().next() == Some(port.as_str()))
            {
                added = true;
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
        }
        _ => {}
    }
    Ok(FirewallGuard {
        firewall: environment.firewall.clone(),
        port: environment.port,
        added,
    })
}

pub async fn open_web_services(environment: &EnvironmentInfo) -> Result<(), String> {
    match environment.firewall.as_deref() {
        Some("firewalld") => {
            for service in ["http", "https"] {
                for args in [
                    vec!["--add-service", service],
                    vec!["--permanent", "--add-service", service],
                ] {
                    let status = Command::new("firewall-cmd")
                        .args(args)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .await
                        .map_err(|error| error.to_string())?;
                    if !status.success() {
                        return Err(format!("firewalld no permitió habilitar {service}"));
                    }
                }
            }
        }
        Some("ufw") => {
            for port in ["80/tcp", "443/tcp"] {
                let status = Command::new("ufw")
                    .args(["allow", port])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err(format!("ufw no permitió habilitar {port}"));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub async fn open_persistent_port(
    environment: &EnvironmentInfo,
    port_number: u16,
) -> Result<(), String> {
    let port = format!("{port_number}/tcp");
    match environment.firewall.as_deref() {
        Some("firewalld") => {
            for args in [
                vec!["--add-port", port.as_str()],
                vec!["--permanent", "--add-port", port.as_str()],
            ] {
                let status = Command::new("firewall-cmd")
                    .args(args)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await
                    .map_err(|error| error.to_string())?;
                if !status.success() {
                    return Err(format!("firewalld no permitió habilitar {port}"));
                }
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
                return Err(format!("ufw no permitió habilitar {port}"));
            }
        }
        _ => {}
    }
    Ok(())
}
