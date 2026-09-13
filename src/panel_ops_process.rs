//! Top processes snapshot for the Server hub.

#[cfg(not(windows))]
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub user: String,
    pub pid: String,
    pub cpu: String,
    pub mem: String,
    pub command: String,
}

pub fn snapshot_top_processes(limit: usize) -> Result<Vec<ProcessRow>, String> {
    let limit = limit.clamp(1, 100);
    #[cfg(windows)]
    {
        let _ = limit;
        Err("Process snapshot is supported on Linux hosts only".into())
    }
    #[cfg(not(windows))]
    {
        let out = Command::new("ps")
            .args(["-eo", "user,pid,pcpu,pmem,args", "--sort=-pcpu"])
            .output()
            .map_err(|e| format!("Failed to run ps: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "ps failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut rows = Vec::new();
        for (i, line) in text.lines().enumerate() {
            if i == 0 {
                continue;
            }
            if rows.len() >= limit {
                break;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 5 {
                continue;
            }
            rows.push(ProcessRow {
                user: parts[0].to_string(),
                pid: parts[1].to_string(),
                cpu: parts[2].to_string(),
                mem: parts[3].to_string(),
                command: parts[4..].join(" "),
            });
        }
        Ok(rows)
    }
}

/// Parse a `ps` percent field (`pcpu` / `pmem`) into an f64 (0 on parse failure).
pub fn parse_percent(value: &str) -> f64 {
    value.trim().parse::<f64>().unwrap_or(0.0)
}

/// CSS heat class for CPU: hot (>= 80), warm (>= 40), or empty.
pub fn cpu_heat_class(cpu: &str) -> &'static str {
    let value = parse_percent(cpu);
    if value >= 80.0 {
        "proc-hot"
    } else if value >= 40.0 {
        "proc-warm"
    } else {
        ""
    }
}

/// Truncate a command for table display; full string stays in the `title` tooltip.
pub fn truncate_command(cmd: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(4);
    let count = cmd.chars().count();
    if count <= max_chars {
        return cmd.to_string();
    }
    let keep = max_chars.saturating_sub(3);
    let head: String = cmd.chars().take(keep).collect();
    format!("{head}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_runs_or_explains() {
        match snapshot_top_processes(5) {
            Ok(rows) => assert!(rows.len() <= 5),
            Err(msg) => assert!(!msg.is_empty()),
        }
    }

    #[test]
    fn truncate_command_keeps_short_and_clips_long() {
        assert_eq!(truncate_command("bash", 48), "bash");
        let long = "a".repeat(60);
        let clipped = truncate_command(&long, 20);
        assert!(clipped.ends_with("..."));
        assert_eq!(clipped.chars().count(), 20);
    }

    #[test]
    fn cpu_heat_class_thresholds() {
        assert_eq!(cpu_heat_class("12.5"), "");
        assert_eq!(cpu_heat_class("40.0"), "proc-warm");
        assert_eq!(cpu_heat_class("93.4"), "proc-hot");
        assert_eq!(cpu_heat_class("n/a"), "");
    }
}
