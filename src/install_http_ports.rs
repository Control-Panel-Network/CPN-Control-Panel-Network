//! Detect and stop only HTTP stacks that own :80/:443 (or would reclaim them).

use crate::install_journal::{self, JournalAction};
use crate::installer::AppState;
use crate::model::ServerEngine;
use std::collections::BTreeSet;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitPriorState {
    pub unit: String,
    pub was_active: bool,
    pub was_enabled: bool,
}

fn conflict_units(selected: ServerEngine) -> &'static [&'static str] {
    match selected {
        ServerEngine::Openlitespeed => &["nginx", "httpd", "apache2", "caddy"],
        ServerEngine::Nginx => &["httpd", "apache2", "lshttpd", "lsws", "caddy"],
        ServerEngine::Caddy => &["nginx", "httpd", "apache2", "lshttpd", "lsws"],
    }
}

fn unit_file_exists(unit: &str) -> bool {
    let lib = format!("/usr/lib/systemd/system/{unit}.service");
    let etc = format!("/etc/systemd/system/{unit}.service");
    std::path::Path::new(&lib).exists() || std::path::Path::new(&etc).exists()
}

/// Parse `ss -ltnp` style lines for listeners on TCP 80/443 and collect PIDs.
pub fn parse_http_listener_pids(ss_output: &str) -> BTreeSet<u32> {
    let mut pids = BTreeSet::new();
    for line in ss_output.lines() {
        if !line_mentions_http_port(line) {
            continue;
        }
        for pid in extract_pids(line) {
            pids.insert(pid);
        }
    }
    pids
}

fn line_mentions_http_port(line: &str) -> bool {
    // Match *:80, 0.0.0.0:80, [::]:80, *:443, etc. Avoid :8080 / :8000 / :4430.
    let lower = line.to_ascii_lowercase();
    lower.contains(":80 ")
        || lower.contains(":80\t")
        || lower.ends_with(":80")
        || lower.contains(":443 ")
        || lower.contains(":443\t")
        || lower.ends_with(":443")
        || lower.contains("]:80 ")
        || lower.contains("]:443 ")
}

fn extract_pids(line: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(idx) = rest.find("pid=") {
        rest = &rest[idx + 4..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(pid) = digits.parse::<u32>() {
            out.push(pid);
        }
    }
    out
}

/// Map a PID to a systemd unit name when possible (best-effort).
pub fn unit_name_from_cgroup(cgroup: &str) -> Option<String> {
    for part in cgroup.split(['/', '\n']) {
        let part = part.trim();
        if let Some(name) = part.strip_suffix(".service") {
            if name.is_empty() {
                continue;
            }
            // Prefer the base name for template units (foo@bar -> foo).
            if let Some((base, _)) = name.split_once('@')
                && !base.is_empty()
            {
                return Some(base.to_string());
            }
            return Some(name.to_string());
        }
    }
    None
}

fn normalize_unit_name(name: &str) -> String {
    name.trim().trim_end_matches(".service").to_string()
}

async fn read_pid_cgroup(pid: u32) -> Option<String> {
    let path = format!("/proc/{pid}/cgroup");
    tokio::task::spawn_blocking(move || std::fs::read_to_string(path).ok())
        .await
        .ok()
        .flatten()
}

async fn unit_is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn unit_is_enabled(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-enabled", "--quiet", unit])
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn collect_listener_units() -> Result<BTreeSet<String>, String> {
    let output = Command::new("ss")
        .args(["-ltnp"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("ss -ltnp failed: {error}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut units = BTreeSet::new();
    for pid in parse_http_listener_pids(&text) {
        if let Some(cgroup) = read_pid_cgroup(pid).await
            && let Some(unit) = unit_name_from_cgroup(&cgroup)
        {
            units.insert(normalize_unit_name(&unit));
        }
    }
    Ok(units)
}

/// Stop/disable only units that own :80/:443, plus enabled conflict units that
/// would reclaim those ports after reboot. Records prior state for rollback notes.
pub async fn stop_conflicting_http_services(
    state: &AppState,
    selected: ServerEngine,
) -> Result<Vec<UnitPriorState>, String> {
    let conflicts: BTreeSet<&str> = conflict_units(selected).iter().copied().collect();
    let listeners = collect_listener_units().await.unwrap_or_default();
    let mut changed: Vec<UnitPriorState> = Vec::new();

    for unit in conflict_units(selected) {
        if !unit_file_exists(unit) {
            continue;
        }
        let was_active = unit_is_active(unit).await;
        let was_enabled = unit_is_enabled(unit).await;
        let owns_port = listeners.iter().any(|u| u == unit);

        // Active but not owning :80/:443: leave alone (other-port deployments).
        if was_active && !owns_port {
            state
                .progress(
                    "configuring",
                    2,
                    format!("Omitiendo {unit}: activo pero no es dueño de :80/:443"),
                )
                .await;
            continue;
        }

        // Stop confirmed owners. Disable enabled units that would reclaim ports.
        let should_stop = owns_port;
        let should_disable = owns_port || was_enabled;
        if !should_stop && !should_disable {
            continue;
        }

        let prior = UnitPriorState {
            unit: (*unit).to_string(),
            was_active,
            was_enabled,
        };

        if should_stop {
            state
                .progress(
                    "configuring",
                    2,
                    format!("Liberando :80/:443 deteniendo el servicio conflictivo {unit}"),
                )
                .await;
            let stop_status = Command::new("systemctl")
                .args(["stop", unit])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .status()
                .await
                .map_err(|error| format!("No se pudo detener {unit}: {error}"))?;
            if !stop_status.success() {
                return Err(format!(
                    "systemctl stop {unit} falló (código {:?}); libera :80/:443 manualmente e inténtalo de nuevo",
                    stop_status.code()
                ));
            }
        }

        if should_disable && was_enabled {
            let _ = Command::new("systemctl")
                .args(["disable", unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }

        install_journal::record(
            "server",
            JournalAction::Note,
            unit,
            None,
            Some(format!(
                "http-port-conflict prior_active={} prior_enabled={} stopped={} disabled={} selected={}",
                prior.was_active,
                prior.was_enabled,
                should_stop,
                should_disable && was_enabled,
                selected.label()
            )),
        )?;
        changed.push(prior);
    }

    // Unknown process still holding :80/:443 after known conflicts handled.
    let remaining = collect_listener_units().await.unwrap_or_default();
    let foreign: Vec<String> = remaining
        .into_iter()
        .filter(|u| !conflicts.contains(u.as_str()))
        .filter(|u| {
            // Selected engine unit is fine if already present.
            match selected {
                ServerEngine::Openlitespeed => u != "lsws" && u != "lshttpd",
                ServerEngine::Nginx => u != "nginx",
                ServerEngine::Caddy => u != "caddy",
            }
        })
        .collect();
    if !foreign.is_empty() {
        return Err(format!(
            "Puerto :80/:443 aún ocupado por unidad(es) no gestionada(s): {}. Detén el proceso o elige otro motor.",
            foreign.join(", ")
        ));
    }

    Ok(changed)
}

/// Best-effort restore of units stopped/disabled for a failed install.
pub async fn restore_http_units(priors: &[UnitPriorState]) {
    for prior in priors.iter().rev() {
        if prior.was_enabled {
            let _ = Command::new("systemctl")
                .args(["enable", &prior.unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
        if prior.was_active {
            let _ = Command::new("systemctl")
                .args(["start", &prior.unit])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ss_pids_for_80_and_443_only() {
        let sample = "\
State  Recv-Q Send-Q Local Address:Port  Peer Address:Port Process\n\
LISTEN 0      511          0.0.0.0:80         0.0.0.0:*    users:((\"nginx\",pid=111,fd=6))\n\
LISTEN 0      511          0.0.0.0:8080       0.0.0.0:*    users:((\"nginx\",pid=222,fd=7))\n\
LISTEN 0      511             [::]:443           [::]:*    users:((\"httpd\",pid=333,fd=8))\n\
LISTEN 0      511          0.0.0.0:4430       0.0.0.0:*    users:((\"other\",pid=444,fd=9))\n";
        let pids = parse_http_listener_pids(sample);
        assert_eq!(pids, BTreeSet::from([111, 333]));
    }

    #[test]
    fn cgroup_maps_to_unit_name() {
        let cgroup = "0::/system.slice/nginx.service\n";
        assert_eq!(unit_name_from_cgroup(cgroup).as_deref(), Some("nginx"));
    }

    #[test]
    fn conflict_lists_exclude_selected_engine() {
        assert!(!conflict_units(ServerEngine::Nginx).contains(&"nginx"));
        assert!(conflict_units(ServerEngine::Nginx).contains(&"httpd"));
        assert!(conflict_units(ServerEngine::Openlitespeed).contains(&"nginx"));
    }
}
