//! Host counters for the dashboard Activity Board (traffic, disk IO, CPU).

use std::fs;

#[derive(Debug, Clone)]
pub struct TrafficNic {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
}

#[derive(Debug, Clone)]
pub struct DiskIoRow {
    pub device: String,
    pub reads: u64,
    pub writes: u64,
    pub read_sectors: u64,
    pub write_sectors: u64,
}

#[derive(Debug, Clone)]
pub struct CpuActivity {
    pub percent: Option<u8>,
    pub detail: String,
    pub loadavg: String,
}

pub fn network_traffic() -> Vec<TrafficNic> {
    #[cfg(windows)]
    {
        return Vec::new();
    }
    #[cfg(not(windows))]
    {
        let raw = match fs::read_to_string("/proc/net/dev") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let mut rows = Vec::new();
        for line in raw.lines().skip(2) {
            let line = line.trim();
            let Some((name, rest)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim();
            if name == "lo" || name.is_empty() {
                continue;
            }
            let fields: Vec<&str> = rest.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }
            let rx_bytes = fields[0].parse().unwrap_or(0);
            let rx_packets = fields[1].parse().unwrap_or(0);
            let tx_bytes = fields[8].parse().unwrap_or(0);
            let tx_packets = fields[9].parse().unwrap_or(0);
            rows.push(TrafficNic {
                name: name.to_string(),
                rx_bytes,
                tx_bytes,
                rx_packets,
                tx_packets,
            });
        }
        rows.sort_by(|a, b| b.rx_bytes.cmp(&a.rx_bytes));
        rows.truncate(12);
        rows
    }
}

pub fn disk_io_snapshot() -> Vec<DiskIoRow> {
    #[cfg(windows)]
    {
        return Vec::new();
    }
    #[cfg(not(windows))]
    {
        let raw = match fs::read_to_string("/proc/diskstats") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let mut rows = Vec::new();
        for line in raw.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 14 {
                continue;
            }
            let name = fields[2];
            let is_disk = (name.starts_with("sd") && name.len() == 3)
                || (name.starts_with("vd") && name.len() == 3)
                || (name.starts_with("xvd") && name.len() == 4)
                || (name.starts_with("nvme") && name.contains('n') && !name.contains('p'))
                || name.starts_with("dm-")
                || (name.starts_with("mmcblk") && !name.contains('p'));
            if !is_disk {
                continue;
            }
            rows.push(DiskIoRow {
                device: name.to_string(),
                reads: fields[3].parse().unwrap_or(0),
                writes: fields[7].parse().unwrap_or(0),
                read_sectors: fields[5].parse().unwrap_or(0),
                write_sectors: fields[9].parse().unwrap_or(0),
            });
        }
        rows.sort_by(|a, b| {
            (b.reads + b.writes)
                .cmp(&(a.reads + a.writes))
                .then_with(|| a.device.cmp(&b.device))
        });
        rows.truncate(12);
        rows
    }
}

pub fn cpu_activity() -> CpuActivity {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let percent = fs::read_to_string("/proc/stat").ok().and_then(|raw| {
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
        let idle = values.get(3)? + values.get(4)?;
        (total > 0).then(|| (100 * total.saturating_sub(idle) / total) as u8)
    });
    let loadavg = fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|s| {
            s.split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unavailable".into());
    CpuActivity {
        percent,
        detail: format!("{cores} cores"),
        loadavg,
    }
}

pub fn format_bytes(n: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    let v = n as f64;
    if v >= TIB {
        format!("{:.2} TiB", v / TIB)
    } else if v >= GIB {
        format!("{:.2} GiB", v / GIB)
    } else if v >= MIB {
        format!("{:.1} MiB", v / MIB)
    } else if v >= KIB {
        format!("{:.0} KiB", v / KIB)
    } else {
        format!("{n} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_scales() {
        assert_eq!(format_bytes(500), "500 B");
        assert!(format_bytes(2048).contains("KiB"));
        assert!(format_bytes(5 * 1024 * 1024).contains("MiB"));
    }

    #[test]
    fn collectors_do_not_panic() {
        let _ = network_traffic();
        let _ = disk_io_snapshot();
        let _ = cpu_activity();
    }
}
