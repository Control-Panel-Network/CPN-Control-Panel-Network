//! In-memory maintenance "already in progress" lock healing.
//!
//! Version UI busy state lives in the panel process. A 90s page refresh looks
//! idle while cargo still runs (lock is valid). If cargo/rustc died, the lock
//! must not block retry forever.

use crate::installer::AppState;
use std::collections::HashMap;
use std::fs;

const BUILDER_NAMES: &[&str] = &[
    "cargo",
    "rustc",
    "rustc.exe",
    "npm",
    "node",
    "node.exe",
];

/// Parse `/proc/<pid>/stat` into (comm, ppid).
pub fn parse_proc_stat(stat: &str) -> Option<(String, u32)> {
    let start = stat.find('(')?;
    let end = stat.rfind(')')?;
    if end <= start {
        return None;
    }
    let comm = stat[start + 1..end].to_string();
    let mut rest = stat[end + 2..].split_whitespace();
    let _state = rest.next()?;
    let ppid = rest.next()?.parse().ok()?;
    Some((comm, ppid))
}

#[cfg(unix)]
fn load_proc_table() -> (HashMap<u32, Vec<u32>>, HashMap<u32, String>) {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut names: HashMap<u32, String> = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return (children, names);
    };
    for entry in entries.flatten() {
        let pid: u32 = match entry.file_name().to_string_lossy().parse() {
            Ok(pid) => pid,
            Err(_) => continue,
        };
        let stat = match fs::read_to_string(entry.path().join("stat")) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let Some((comm, ppid)) = parse_proc_stat(&stat) else {
            continue;
        };
        names.insert(pid, comm);
        children.entry(ppid).or_default().push(pid);
    }
    (children, names)
}

#[cfg(unix)]
fn walk_descendants(pid: u32, children: &HashMap<u32, Vec<u32>>, out: &mut Vec<u32>) {
    let Some(kids) = children.get(&pid) else {
        return;
    };
    for child in kids {
        out.push(*child);
        walk_descendants(*child, children, out);
    }
}

/// True when this process still has cargo/rustc/npm descendants (tip source build).
pub fn builder_descendant_running() -> bool {
    #[cfg(unix)]
    {
        let me = std::process::id();
        let (children, names) = load_proc_table();
        let mut desc = Vec::new();
        walk_descendants(me, &children, &mut desc);
        desc.iter().any(|pid| {
            names
                .get(pid)
                .map(|name| BUILDER_NAMES.iter().any(|want| name == want))
                .unwrap_or(false)
        })
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn is_busy_phase(phase: &str) -> bool {
    matches!(
        phase,
        "configuring" | "downloading" | "installing" | "testing" | "verifying"
    )
}

/// If phase is busy but no compiler child remains, mark failed so the operator can retry.
pub fn heal_orphaned_busy(state: &AppState) -> bool {
    let phase = {
        let status = state.status.read().unwrap_or_else(|e| e.into_inner());
        status.phase.clone()
    };
    if !is_busy_phase(&phase) {
        return false;
    }
    if builder_descendant_running() {
        return false;
    }
    let msg = "Previous upgrade/repair worker is no longer running. The in-progress lock was cleared so you can retry.";
    crate::upgrade_tip_log::log_failure(msg, None);
    {
        let mut status = state.status.write().unwrap_or_else(|e| e.into_inner());
        status.phase = "failed";
        status.error = Some(msg.to_string());
        status.message = "Maintenance lock cleared (orphaned worker)".into();
        status.restart_scheduled = false;
    }
    let _ = crate::panel_maintenance_mode::clear_with_reason("orphaned-busy");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat_comm_and_ppid() {
        let line = "33870 (cargo) S 32335 33870 33870 0 -1 4194304 1 0 0 0 0 0 0 0 20 0 4 0 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0";
        let (comm, ppid) = parse_proc_stat(line).expect("parse");
        assert_eq!(comm, "cargo");
        assert_eq!(ppid, 32335);
    }

    #[test]
    fn busy_phases_match_maintenance_api() {
        assert!(is_busy_phase("downloading"));
        assert!(is_busy_phase("installing"));
        assert!(is_busy_phase("verifying"));
        assert!(!is_busy_phase("completed"));
        assert!(!is_busy_phase("failed"));
        assert!(!is_busy_phase("ready"));
    }
}
