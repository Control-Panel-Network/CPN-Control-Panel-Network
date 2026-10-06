//! In-memory maintenance "already in progress" lock healing.
//!
//! Version UI busy state lives in the panel process. The upgrade/repair worker
//! is a tokio task in this process (plus optional child tools). A status poll
//! must not treat RPM/DEB apply or post-apply verify as dead just because
//! cargo/rustc is absent. Clear the lock only when the in-process job is gone
//! and no upgrade child remains.

use crate::installer::AppState;
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const WORKER_NAMES: &[&str] = &[
    "cargo",
    "rustc",
    "rustc.exe",
    "npm",
    "node",
    "node.exe",
    "rpm",
    "dnf",
    "yum",
    "apt",
    "apt-get",
    "dpkg",
    "dpkg-deb",
    "cpn-installer",
];

static JOB_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static LAST_HEARTBEAT_UNIX: AtomicU64 = AtomicU64::new(0);

/// RAII flag: the maintenance tokio/CLI task is still running (even with no cargo).
pub struct JobGuard;

impl JobGuard {
    pub fn enter() -> Self {
        mark_job_started();
        JobGuard
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        mark_job_finished();
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn mark_job_started() {
    JOB_IN_FLIGHT.store(true, Ordering::SeqCst);
    touch_job_heartbeat();
}

pub fn mark_job_finished() {
    JOB_IN_FLIGHT.store(false, Ordering::SeqCst);
}

pub fn job_in_flight() -> bool {
    JOB_IN_FLIGHT.load(Ordering::SeqCst)
}

pub fn touch_job_heartbeat() {
    LAST_HEARTBEAT_UNIX.store(now_unix(), Ordering::SeqCst);
}

/// True when `/proc` comm is an upgrade/repair child (package apply or source build).
pub fn worker_comm_is_live(comm: &str) -> bool {
    let name = comm
        .strip_suffix(".exe")
        .unwrap_or(comm)
        .to_ascii_lowercase();
    WORKER_NAMES.iter().any(|want| {
        let want = want.strip_suffix(".exe").unwrap_or(want);
        name == *want || name.starts_with(&format!("{want}-"))
    })
}

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

fn pid_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        fs::metadata(format!("/proc/{pid}")).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn registered_child_alive(state: &AppState) -> bool {
    let Ok(pids) = state.active_child_pids.lock() else {
        return false;
    };
    pids.iter().copied().any(pid_is_alive)
}

/// True when this process still has cargo/rpm/dpkg (or similar) descendants.
pub fn worker_descendant_running() -> bool {
    #[cfg(unix)]
    {
        let me = std::process::id();
        let (children, names) = load_proc_table();
        let mut desc = Vec::new();
        walk_descendants(me, &children, &mut desc);
        desc.iter().any(|pid| {
            names
                .get(pid)
                .map(|name| worker_comm_is_live(name))
                .unwrap_or(false)
        })
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Back-compat alias used by the Version conflict hint.
pub fn builder_descendant_running() -> bool {
    worker_descendant_running()
}

pub fn is_busy_phase(phase: &str) -> bool {
    matches!(
        phase,
        "configuring" | "downloading" | "installing" | "testing" | "verifying"
    )
}

/// Policy: clear only when the UI lock is busy, the tokio/CLI job is gone, and
/// no package/source child is still running. RPM repair has no cargo.
/// `heartbeat_fresh` covers the tiny window after phase=busy and before JobGuard.
pub fn should_clear_orphaned_lock(
    busy: bool,
    in_flight: bool,
    worker_or_child_alive: bool,
    heartbeat_fresh: bool,
) -> bool {
    busy && !in_flight && !worker_or_child_alive && !heartbeat_fresh
}

fn heartbeat_is_fresh(max_age_secs: u64) -> bool {
    let last = LAST_HEARTBEAT_UNIX.load(Ordering::SeqCst);
    if last == 0 {
        return false;
    }
    now_unix().saturating_sub(last) <= max_age_secs
}

/// If phase is busy but the worker is gone, mark failed so the operator can retry.
pub fn heal_orphaned_busy(state: &AppState) -> bool {
    let phase = {
        let status = state.status.read().unwrap_or_else(|e| e.into_inner());
        status.phase
    };
    let child_alive = worker_descendant_running() || registered_child_alive(state);
    if !should_clear_orphaned_lock(
        is_busy_phase(phase),
        job_in_flight(),
        child_alive,
        heartbeat_is_fresh(5),
    ) {
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

    #[test]
    fn package_apply_comms_count_as_live_workers() {
        assert!(worker_comm_is_live("rpm"));
        assert!(worker_comm_is_live("dnf"));
        assert!(worker_comm_is_live("dnf-3"));
        assert!(worker_comm_is_live("yum"));
        assert!(worker_comm_is_live("dpkg"));
        assert!(worker_comm_is_live("dpkg-deb"));
        assert!(worker_comm_is_live("apt-get"));
        assert!(worker_comm_is_live("cpn-installer"));
        assert!(worker_comm_is_live("cargo"));
        assert!(!worker_comm_is_live("sshd"));
        assert!(!worker_comm_is_live("chrome"));
    }

    #[test]
    fn orphan_heal_skips_live_rpm_job_without_cargo() {
        assert!(
            !should_clear_orphaned_lock(true, true, false, false),
            "in-process repair/verify must not look orphaned when cargo is absent"
        );
        assert!(
            !should_clear_orphaned_lock(true, false, true, false),
            "rpm/dnf child still running"
        );
        assert!(
            !should_clear_orphaned_lock(true, false, false, true),
            "fresh heartbeat: job just started or still logging"
        );
        assert!(
            should_clear_orphaned_lock(true, false, false, false),
            "busy lock with no job and no child is a real orphan"
        );
        assert!(!should_clear_orphaned_lock(false, false, false, false));
    }
}
