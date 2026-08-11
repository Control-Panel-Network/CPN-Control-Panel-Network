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
    }
}

pub async fn open_installer_port(environment: &EnvironmentInfo) -> Result<(), String> {
    let port = format!("{}/tcp", environment.port);
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
                    return Err("firewalld no permitió abrir el puerto del instalador".into());
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
                return Err("ufw no permitió abrir el puerto del instalador".into());
            }
        }
        _ => {}
    }
    Ok(())
}
