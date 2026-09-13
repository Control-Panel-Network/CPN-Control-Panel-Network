//! Resource snapshots for Manage Overview (host-level when site metrics missing).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cheap approximate disk usage (bytes) with a file walk cap.
pub fn approx_dir_bytes(root: &Path, max_files: usize) -> Option<u64> {
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
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
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
        assert_eq!(format_bytes(512), "512 B");
        assert!(format_bytes(2048).contains("KB"));
        assert!(format_bytes(5_000_000).contains("MB"));
    }

    #[test]
    fn host_snapshot_does_not_panic() {
        let _ = host_resource_snapshot(None);
    }
}
