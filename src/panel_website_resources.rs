//! Resource snapshots for Manage Overview (honest server-level when site metrics missing).

use std::path::Path;

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

#[derive(Debug, Clone)]
pub struct HostSnapshot {
    pub cpu_pct: Option<f32>,
    pub mem_pct: Option<f32>,
    pub detail: String,
}

/// Best-effort host CPU/memory snapshot (not per-site).
pub fn host_resource_snapshot() -> HostSnapshot {
    #[cfg(windows)]
    {
        HostSnapshot {
            cpu_pct: None,
            mem_pct: None,
            detail: "Host gauges are available on Linux panel hosts.".into(),
        }
    }
    #[cfg(not(windows))]
    {
        let mem_pct = read_mem_pct();
        let cpu_pct = read_load_as_cpu_hint();
        HostSnapshot {
            cpu_pct,
            mem_pct,
            detail:
                "Server snapshot (not per-site). Site-level CPU/bandwidth metering ships later."
                    .into(),
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
fn read_load_as_cpu_hint() -> Option<f32> {
    let raw = std::fs::read_to_string("/proc/loadavg").ok()?;
    let load1: f32 = raw.split_whitespace().next()?.parse().ok()?;
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get() as f32)
        .unwrap_or(1.0)
        .max(1.0);
    Some(((load1 / cpus) * 100.0).min(100.0))
}

/// Simple SVG sparkline from a value (0-100) for Overview charts.
pub fn sparkline_svg(pct: Option<f32>, stroke: &str) -> String {
    let value = pct.unwrap_or(0.0).clamp(0.0, 100.0);
    // Synthetic gentle curve ending at current value (visual only).
    let points = [
        8.0,
        18.0,
        14.0,
        28.0,
        22.0,
        35.0,
        30.0,
        (value * 0.7),
        value,
    ];
    let mut d = String::from("M 0 80");
    let n = points.len().max(1) as f32;
    for (i, p) in points.iter().enumerate() {
        let x = (i as f32 / (n - 1.0)) * 320.0;
        let y = 80.0 - (*p / 100.0) * 70.0;
        d.push_str(&format!(" L {x:.1} {y:.1}"));
    }
    format!(
        r#"<svg viewBox="0 0 320 88" role="img" aria-label="Resource chart">
  <path d="{d}" fill="none" stroke="{stroke}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"#
    )
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
    fn sparkline_renders() {
        let svg = sparkline_svg(Some(42.0), "#3b82f6");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("#3b82f6"));
    }
}
