//! Resource snapshots for Manage Overview (host-level when site metrics missing).

use crate::sites::{SiteRecord, list_sites, site_home_from_record};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cheap approximate disk usage (bytes) with a file walk cap.
pub fn approx_dir_bytes(root: &Path, max_files: usize) -> Option<u64> {
    approx_dir_bytes_skip(root, &[], max_files)
}

fn path_is_skipped(path: &Path, skip: &[PathBuf]) -> bool {
    skip.iter().any(|other| path == other)
}

/// Walk `root`, skipping nested directories listed in `skip` (other site homes).
pub fn approx_dir_bytes_skip(root: &Path, skip: &[PathBuf], max_files: usize) -> Option<u64> {
    if !root.exists() {
        return None;
    }
    let mut total = 0u64;
    let mut count = 0usize;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path_is_skipped(&path, skip) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
            count += 1;
            if count >= max_files {
                return Some(total);
            }
        }
    }
    Some(total)
}

pub fn format_bytes(bytes: u64) -> String {
    crate::panel_storage_fmt::format_bytes_auto(bytes)
}

/// Used bytes for this site home, excluding nested other-site homes.
pub fn site_used_bytes(site: &SiteRecord, max_files: usize) -> Option<u64> {
    let home = site_home_from_record(site);
    let skip: Vec<PathBuf> = list_sites()
        .unwrap_or_default()
        .into_iter()
        .filter(|other| !other.domain.eq_ignore_ascii_case(&site.domain))
        .map(|other| site_home_from_record(&other))
        .filter(|path| path.starts_with(&home) && path != &home)
        .collect();
    let root = if home.exists() {
        home
    } else {
        PathBuf::from(&site.docroot)
    };
    approx_dir_bytes_skip(&root, &skip, max_files)
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy)]
pub struct CpuCounters {
    pub total: u64,
    pub idle: u64,
}

#[derive(Debug, Clone)]
pub struct HostSnapshot {
    pub cpu_pct: Option<f32>,
    pub mem_pct: Option<f32>,
    pub cpu_counters: Option<CpuCounters>,
    pub detail: String,
}

/// Live host CPU/memory (not per-site). CPU uses delta when previous counters are supplied.
pub fn host_resource_snapshot(prev: Option<CpuCounters>) -> HostSnapshot {
    #[cfg(windows)]
    {
        let _ = prev;
        HostSnapshot {
            cpu_pct: None,
            mem_pct: None,
            cpu_counters: None,
            detail: "Host gauges are available on Linux panel hosts.".into(),
        }
    }
    #[cfg(not(windows))]
    {
        let mem_pct = read_mem_pct();
        let (cpu_pct, cpu_counters) = read_cpu_pct(prev);
        HostSnapshot {
            cpu_pct,
            mem_pct,
            cpu_counters,
            detail: "Host live metrics (not per-site). Site-level CPU metering ships later.".into(),
        }
    }
}

#[cfg(not(windows))]
fn read_mem_pct() -> Option<f32> {
    let raw = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = 0u64;
    let mut available = 0u64;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = parse_kb(rest)?;
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = parse_kb(rest)?;
        }
    }
    if total == 0 {
        return None;
    }
    let used = total.saturating_sub(available);
    Some(((used as f64 / total as f64) * 100.0) as f32)
}

#[cfg(not(windows))]
fn parse_kb(rest: &str) -> Option<u64> {
    rest.split_whitespace().next()?.parse().ok()
}

#[cfg(not(windows))]
fn read_cpu_counters() -> Option<CpuCounters> {
    let raw = std::fs::read_to_string("/proc/stat").ok()?;
    let values: Vec<u64> = raw
        .lines()
        .next()?
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let total: u64 = values.iter().sum();
    let idle = *values.get(3)? + values.get(4).copied().unwrap_or(0);
    (total > 0).then_some(CpuCounters { total, idle })
}

#[cfg(not(windows))]
fn read_cpu_pct(prev: Option<CpuCounters>) -> (Option<f32>, Option<CpuCounters>) {
    let Some(now) = read_cpu_counters() else {
        return (None, None);
    };
    let pct = match prev {
        Some(prev) if now.total > prev.total => {
            let dt = now.total - prev.total;
            let di = now.idle.saturating_sub(prev.idle);
            let busy = dt.saturating_sub(di);
            Some(((busy as f64 / dt as f64) * 100.0).clamp(0.0, 100.0) as f32)
        }
        _ => {
            // First sample: since-boot average from /proc/stat (same as dashboard).
            let busy = now.total.saturating_sub(now.idle);
            Some(((busy as f64 / now.total as f64) * 100.0).clamp(0.0, 100.0) as f32)
        }
    };
    (pct, Some(now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert!(format_bytes(512).ends_with(" KB"));
        assert!(!format_bytes(512).ends_with(" B"));
        assert!(format_bytes(2048).contains("KB"));
        assert!(format_bytes(5_000_000).contains("MB"));
    }

    #[test]
    fn host_snapshot_does_not_panic() {
        let _ = host_resource_snapshot(None);
    }
}
