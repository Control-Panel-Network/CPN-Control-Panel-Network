//! Ring buffer of host CPU/memory samples for Manage Overview charts.

use crate::panel_website_resources::{CpuCounters, host_resource_snapshot, unix_now};
use crate::paths::join_data;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const MAX_SAMPLES: usize = 90;
pub const SAMPLE_MIN_GAP_SECS: u64 = 8;
pub const WINDOW_SECS: u64 = 15 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub t: u64,
    pub cpu: f32,
    pub mem: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RingFile {
    schema_version: u32,
    last_cpu_total: u64,
    last_cpu_idle: u64,
    samples: Vec<MetricSample>,
}

fn ring_path() -> PathBuf {
    join_data("host-metrics-ring.json")
}

fn load_ring() -> RingFile {
    let Ok(raw) = fs::read_to_string(ring_path()) else {
        return RingFile {
            schema_version: 1,
            ..Default::default()
        };
    };
    serde_json::from_str(&raw).unwrap_or(RingFile {
        schema_version: 1,
        ..Default::default()
    })
}

fn save_ring(file: &RingFile) -> Result<(), String> {
    if let Some(parent) = ring_path().parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data dir: {e}"))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let raw = serde_json::to_string_pretty(file)
        .map_err(|e| format!("Could not serialize metrics ring: {e}"))?;
    let mut out = options
        .open(ring_path())
        .map_err(|e| format!("Could not write metrics ring: {e}"))?;
    out.write_all(raw.as_bytes())
        .map_err(|e| format!("Could not save metrics ring: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(ring_path(), fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn prune(samples: &mut Vec<MetricSample>, now: u64) {
    let cutoff = now.saturating_sub(WINDOW_SECS);
    samples.retain(|s| s.t >= cutoff);
    if samples.len() > MAX_SAMPLES {
        let drop = samples.len() - MAX_SAMPLES;
        samples.drain(0..drop);
    }
}

/// Record a host sample when enough time has passed; always returns the live window.
pub fn record_host_sample() -> Vec<MetricSample> {
    let mut file = load_ring();
    let now = unix_now();
    let prev = if file.last_cpu_total > 0 {
        Some(CpuCounters {
            total: file.last_cpu_total,
            idle: file.last_cpu_idle,
        })
    } else {
        None
    };
    let snap = host_resource_snapshot(prev);
    if let Some(c) = snap.cpu_counters {
        file.last_cpu_total = c.total;
        file.last_cpu_idle = c.idle;
    }
    let should_push = file
        .samples
        .last()
        .map(|s| now.saturating_sub(s.t) >= SAMPLE_MIN_GAP_SECS)
        .unwrap_or(true);
    if should_push {
        let cpu = snap.cpu_pct.unwrap_or(0.0).clamp(0.0, 100.0);
        let mem = snap.mem_pct.unwrap_or(0.0).clamp(0.0, 100.0);
        file.samples.push(MetricSample { t: now, cpu, mem });
    }
    prune(&mut file.samples, now);
    file.schema_version = 1;
    let _ = save_ring(&file);
    file.samples
}

pub fn load_samples() -> Vec<MetricSample> {
    let mut file = load_ring();
    prune(&mut file.samples, unix_now());
    file.samples
}

pub fn stats_for(values: &[f32]) -> (Option<f32>, Option<f32>, Option<f32>) {
    if values.is_empty() {
        return (None, None, None);
    }
    let current = values[values.len() - 1];
    let sum: f32 = values.iter().sum();
    let avg = sum / values.len() as f32;
    let peak = values.iter().cloned().fold(f32::MIN, f32::max);
    (Some(current), Some(avg), Some(peak))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_empty() {
        assert_eq!(stats_for(&[]), (None, None, None));
    }

    #[test]
    fn stats_basic() {
        let (c, a, p) = stats_for(&[10.0, 20.0, 30.0]);
        assert_eq!(c, Some(30.0));
        assert!((a.unwrap() - 20.0).abs() < 0.01);
        assert_eq!(p, Some(30.0));
    }

    #[test]
    fn prune_keeps_window() {
        let now = 1_000_000u64;
        let mut samples = vec![
            MetricSample {
                t: now - WINDOW_SECS - 10,
                cpu: 1.0,
                mem: 1.0,
            },
            MetricSample {
                t: now - 30,
                cpu: 2.0,
                mem: 2.0,
            },
        ];
        prune(&mut samples, now);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].cpu, 2.0);
    }
}
