//! Host summary for the System Processes Monitor (CPU, memory, swap, load).
//!
//! One-shot `/proc` reads only. Never poll on a 1s timer from the panel.

use std::fs;

#[derive(Debug, Clone, Default)]
pub struct CpuInfo {
    pub cores: u32,
    pub frequency_mhz: String,
    pub model: String,
    pub cache_kb: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessStats {
    pub total: u32,
    pub running: u32,
    pub sleeping: u32,
    pub zombie: u32,
}

#[derive(Debug, Clone, Default)]
pub struct LoadAverage {
    pub one: String,
    pub five: String,
    pub fifteen: String,
    pub current_pct: String,
}

#[derive(Debug, Clone, Default)]
pub struct MemorySummary {
    pub total_mb: u64,
    pub free_mb: u64,
    pub used_mb: u64,
    pub buff_cache_mb: u64,
}

#[derive(Debug, Clone, Default)]
pub struct SwapSummary {
    pub total_mb: u64,
    pub free_mb: u64,
    pub used_mb: u64,
    pub available_mb: u64,
}

#[derive(Debug, Clone, Default)]
pub struct CpuTimeDist {
    pub iowait_pct: String,
    pub idle_pct: String,
    pub hard_irq_pct: String,
    pub soft_irq_pct: String,
}

#[derive(Debug, Clone, Default)]
pub struct HostProcessSummary {
    pub cpu: CpuInfo,
    pub processes: ProcessStats,
    pub load: LoadAverage,
    pub memory: MemorySummary,
    pub swap: SwapSummary,
    pub cpu_time: CpuTimeDist,
}

#[cfg(not(windows))]
fn meminfo_kb(raw: &str, key: &str) -> Option<u64> {
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix(key) {
            let num = rest.split_whitespace().next()?.parse::<u64>().ok()?;
            return Some(num);
        }
    }
    None
}

#[cfg(not(windows))]
fn kb_to_mb(kb: u64) -> u64 {
    kb / 1024
}

#[cfg_attr(windows, allow(dead_code))]
fn pct(part: u64, total: u64) -> String {
    if total == 0 {
        return "0.0".into();
    }
    format!("{:.1}", (part as f64) * 100.0 / (total as f64))
}

#[cfg(not(windows))]
fn read_cpu_info() -> CpuInfo {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    let raw = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let mut model = String::new();
    let mut frequency_mhz = String::new();
    let mut cache_kb = String::new();
    for line in raw.lines() {
        if model.is_empty()
            && let Some(v) = line
                .strip_prefix("model name")
                .or_else(|| line.strip_prefix("Model Name"))
        {
            model = v.trim().trim_start_matches(':').trim().to_string();
        }
        if frequency_mhz.is_empty()
            && let Some(v) = line
                .strip_prefix("cpu MHz")
                .or_else(|| line.strip_prefix("CPU MHz"))
        {
            frequency_mhz = v.trim().trim_start_matches(':').trim().to_string();
        }
        if cache_kb.is_empty()
            && let Some(v) = line.strip_prefix("cache size")
        {
            cache_kb = v
                .trim()
                .trim_start_matches(':')
                .trim()
                .trim_end_matches("KB")
                .trim()
                .to_string();
        }
        if !model.is_empty() && !frequency_mhz.is_empty() && !cache_kb.is_empty() {
            break;
        }
    }
    if model.is_empty() {
        model = "Unavailable".into();
    }
    if frequency_mhz.is_empty() {
        frequency_mhz = "-".into();
    }
    if cache_kb.is_empty() {
        cache_kb = "-".into();
    }
    CpuInfo {
        cores,
        frequency_mhz,
        model,
        cache_kb,
    }
}

#[cfg(not(windows))]
fn read_load(current_pct: Option<u8>) -> LoadAverage {
    let parts: Vec<String> = fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|s| s.split_whitespace().take(3).map(str::to_string).collect())
        .unwrap_or_default();
    LoadAverage {
        one: parts.first().cloned().unwrap_or_else(|| "-".into()),
        five: parts.get(1).cloned().unwrap_or_else(|| "-".into()),
        fifteen: parts.get(2).cloned().unwrap_or_else(|| "-".into()),
        current_pct: current_pct
            .map(|p| format!("{p}.0"))
            .unwrap_or_else(|| "-".into()),
    }
}

#[cfg(not(windows))]
fn read_cpu_time() -> (CpuTimeDist, Option<u8>) {
    let raw = match fs::read_to_string("/proc/stat") {
        Ok(s) => s,
        Err(_) => return (CpuTimeDist::default(), None),
    };
    let values: Vec<u64> = match raw.lines().next() {
        Some(line) => line
            .split_whitespace()
            .skip(1)
            .take(8)
            .filter_map(|v| v.parse().ok())
            .collect(),
        None => return (CpuTimeDist::default(), None),
    };
    if values.len() < 8 {
        return (CpuTimeDist::default(), None);
    }
    let total: u64 = values.iter().sum();
    let idle = values[3] + values[4];
    let iowait = values[4];
    let hard_irq = values[5];
    let soft_irq = values[6];
    let used_pct = (100u64.saturating_mul(total.saturating_sub(idle)))
        .checked_div(total)
        .map(|v| v as u8);
    (
        CpuTimeDist {
            iowait_pct: pct(iowait, total),
            idle_pct: pct(idle, total),
            hard_irq_pct: pct(hard_irq, total),
            soft_irq_pct: pct(soft_irq, total),
        },
        used_pct,
    )
}

#[cfg(not(windows))]
fn read_memory() -> (MemorySummary, SwapSummary) {
    let raw = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let total = meminfo_kb(&raw, "MemTotal:").unwrap_or(0);
    let free = meminfo_kb(&raw, "MemFree:").unwrap_or(0);
    let available = meminfo_kb(&raw, "MemAvailable:").unwrap_or(free);
    let buffers = meminfo_kb(&raw, "Buffers:").unwrap_or(0);
    let cached = meminfo_kb(&raw, "Cached:").unwrap_or(0);
    let s_reclaim = meminfo_kb(&raw, "SReclaimable:").unwrap_or(0);
    let buff_cache = buffers + cached + s_reclaim;
    let used = total.saturating_sub(free + buff_cache);
    let swap_total = meminfo_kb(&raw, "SwapTotal:").unwrap_or(0);
    let swap_free = meminfo_kb(&raw, "SwapFree:").unwrap_or(0);
    let swap_used = swap_total.saturating_sub(swap_free);
    (
        MemorySummary {
            total_mb: kb_to_mb(total),
            free_mb: kb_to_mb(free),
            used_mb: kb_to_mb(used),
            buff_cache_mb: kb_to_mb(buff_cache),
        },
        SwapSummary {
            total_mb: kb_to_mb(swap_total),
            free_mb: kb_to_mb(swap_free),
            used_mb: kb_to_mb(swap_used),
            available_mb: kb_to_mb(available.saturating_sub(total.saturating_sub(swap_total))),
        },
    )
}

/// Count process states from `/proc/*/stat` (cheap single pass; no `ps`).
pub fn count_process_states() -> ProcessStats {
    #[cfg(windows)]
    {
        ProcessStats::default()
    }
    #[cfg(not(windows))]
    {
        let mut stats = ProcessStats::default();
        let Ok(entries) = fs::read_dir("/proc") else {
            return stats;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            // Format: pid (comm) state ...
            let Some(after) = stat.rfind(')') else {
                continue;
            };
            let rest = stat[after + 1..].trim_start();
            let state = rest.chars().next().unwrap_or('?');
            stats.total = stats.total.saturating_add(1);
            match state {
                'R' => stats.running = stats.running.saturating_add(1),
                'S' | 'I' | 'D' => stats.sleeping = stats.sleeping.saturating_add(1),
                'Z' => stats.zombie = stats.zombie.saturating_add(1),
                _ => {}
            }
        }
        stats
    }
}

/// Collect a one-shot host summary for the Top Processes page.
pub fn snapshot_host_process_summary() -> HostProcessSummary {
    #[cfg(windows)]
    {
        HostProcessSummary {
            cpu: CpuInfo {
                cores: 0,
                frequency_mhz: "-".into(),
                model: "Linux hosts only".into(),
                cache_kb: "-".into(),
            },
            ..Default::default()
        }
    }
    #[cfg(not(windows))]
    {
        let cpu = read_cpu_info();
        let (cpu_time, used_pct) = read_cpu_time();
        let load = read_load(used_pct);
        let (memory, mut swap) = read_memory();
        // Available swap field is often unused on VPS; keep free as the useful number.
        if swap.available_mb == 0 {
            swap.available_mb = swap.free_mb;
        }
        let processes = count_process_states();
        HostProcessSummary {
            cpu,
            processes,
            load,
            memory,
            swap,
            cpu_time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct_handles_zero_total() {
        assert_eq!(pct(5, 0), "0.0");
        assert_eq!(pct(25, 100), "25.0");
    }

    #[test]
    fn snapshot_returns_without_panic() {
        let s = snapshot_host_process_summary();
        assert!(!s.cpu.model.is_empty());
    }
}
