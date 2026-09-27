//! Container inspect, stats, processes, and extended lifecycle for Docker detail UI.

use crate::panel_ops_docker::{docker_bin, list_containers_detailed, DockerContainerRow};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DockerContainerDetail {
    pub row: DockerContainerRow,
    pub short_id: String,
    pub port_mappings: String,
    pub restart_policy: String,
    pub start_on_boot: String,
    pub cpu_percent: String,
    pub mem_usage: String,
    pub mem_percent: String,
    pub mem_limit: String,
    pub paused: bool,
}

fn validate_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains(' ')
        || name.contains(';')
    {
        return Err("Invalid container name.".into());
    }
    Ok(name)
}

fn inspect_field(bin: &str, name: &str, format: &str) -> String {
    let output = Command::new(bin)
        .args(["inspect", "--format", format, name])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if v.is_empty() || v == "<no value>" {
                "Unknown".into()
            } else {
                v
            }
        }
        _ => "Unknown".into(),
    }
}

fn format_memory_limit(bytes: &str) -> String {
    let Ok(n) = bytes.trim().parse::<u64>() else {
        return if bytes == "Unknown" {
            "Unknown".into()
        } else {
            bytes.to_string()
        };
    };
    if n == 0 {
        "Unlimited".into()
    } else if n >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", n as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if n >= 1024 * 1024 {
        format!("{} MB", n / (1024 * 1024))
    } else {
        format!("{} KB", n / 1024)
    }
}

fn port_mappings(bin: &str, name: &str) -> String {
    let output = Command::new(bin).args(["port", name]).output();
    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if text.is_empty() {
                "(none published)".into()
            } else {
                text.lines().take(12).collect::<Vec<_>>().join(", ")
            }
        }
        _ => {
            let raw = inspect_field(
                bin,
                name,
                r#"{{range $p, $conf := .NetworkSettings.Ports}}{{range $conf}}{{$p}} -> {{.HostIp}}:{{.HostPort}} {{end}}{{end}}"#,
            );
            if raw == "Unknown" || raw.is_empty() {
                "(none published)".into()
            } else {
                raw
            }
        }
    }
}

fn container_stats(bin: &str, name: &str) -> (String, String, String) {
    let output = Command::new(bin)
        .args([
            "stats",
            "--no-stream",
            "--format",
            "{{.CPUPerc}}\t{{.MemUsage}}\t{{.MemPerc}}",
            name,
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let line = String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                (
                    parts[0].trim().to_string(),
                    parts[1].trim().to_string(),
                    parts[2].trim().to_string(),
                )
            } else {
                ("0.00%".into(), "Unknown".into(), "0.00%".into())
            }
        }
        _ => ("0.00%".into(), "Unknown".into(), "0.00%".into()),
    }
}

pub fn find_container_row(name: &str) -> Result<DockerContainerRow, String> {
    let name = validate_name(name)?;
    let rows = list_containers_detailed()?;
    rows.into_iter()
        .find(|r| r.name == name)
        .ok_or_else(|| format!("Container `{name}` was not found on this host."))
}

pub fn load_container_detail(name: &str) -> Result<DockerContainerDetail, String> {
    let name = validate_name(name)?;
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    let row = find_container_row(name)?;
    let short_id = inspect_field(bin, name, "{{.Id}}");
    let short_id = if short_id.len() > 12 {
        format!("{}…", &short_id[..12])
    } else {
        short_id
    };
    let restart_policy = inspect_field(bin, name, "{{.HostConfig.RestartPolicy.Name}}");
    let start_on_boot = match restart_policy.to_ascii_lowercase().as_str() {
        "always" | "unless-stopped" => "Enabled".into(),
        "no" | "" => "Disabled".into(),
        other => other.to_string(),
    };
    let mem_limit_raw = inspect_field(bin, name, "{{.HostConfig.Memory}}");
    let mem_limit = format_memory_limit(&mem_limit_raw);
    let (cpu_percent, mem_usage, mem_percent) = if row.running {
        container_stats(bin, name)
    } else {
        ("0.00%".into(), "Not running".into(), "0.00%".into())
    };
    let state = inspect_field(bin, name, "{{.State.Status}}");
    let paused = state.eq_ignore_ascii_case("paused");
    Ok(DockerContainerDetail {
        row,
        short_id,
        port_mappings: port_mappings(bin, name),
        restart_policy,
        start_on_boot,
        cpu_percent,
        mem_usage,
        mem_percent,
        mem_limit,
        paused,
    })
}

pub fn container_processes(name: &str) -> Result<String, String> {
    let name = validate_name(name)?;
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    let output = Command::new(bin)
        .args(["top", name])
        .output()
        .map_err(|e| format!("Could not list processes: {e}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&output.stderr).to_string();
    }
    if text.is_empty() {
        text = "(no process data)".into();
    }
    Ok(text.chars().take(8000).collect())
}

pub fn container_exec_command(name: &str, command: &str) -> Result<String, String> {
    let name = validate_name(name)?;
    let cmd = command.trim();
    if cmd.is_empty() || cmd.len() > 200 {
        return Err("Command must be 1 to 200 characters.".into());
    }
    if cmd.contains(';') || cmd.contains('&') || cmd.contains('|') || cmd.contains('`') {
        return Err("Command contains disallowed characters.".into());
    }
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    let output = Command::new(bin)
        .args(["exec", name, "sh", "-c", cmd])
        .output()
        .map_err(|e| format!("Exec failed: {e}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.stderr.is_empty() {
        let err = String::from_utf8_lossy(&output.stderr);
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    if text.is_empty() {
        text = if output.status.success() {
            "(command completed with no output)".into()
        } else {
            "Command failed.".into()
        };
    }
    Ok(text.chars().take(8000).collect())
}

pub fn container_export_tar(name: &str) -> Result<Vec<u8>, String> {
    let name = validate_name(name)?;
    let Some(bin) = docker_bin() else {
        return Err("Docker/Podman CLI not found.".into());
    };
    let output = Command::new(bin)
        .args(["export", name])
        .output()
        .map_err(|e| format!("Export failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Export failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(200)
                .collect::<String>()
        ));
    }
    if output.stdout.is_empty() {
        return Err("Export returned no data.".into());
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_limit_format() {
        assert_eq!(format_memory_limit("0"), "Unlimited");
        assert_eq!(format_memory_limit("1048576"), "1 MB");
    }
}
