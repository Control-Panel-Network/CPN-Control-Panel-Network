//! Top processes snapshot and kill helpers for the Server hub.

#[cfg(not(windows))]
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub user: String,
    pub pid: String,
    pub state: String,
    pub cpu: String,
    pub mem: String,
    pub virt: String,
    pub res: String,
    pub time: String,
    pub command: String,
}

pub(crate) fn map_state(code: &str) -> String {
    match code.chars().next().unwrap_or('?') {
        'R' => "Running".into(),
        'S' | 'I' => "Sleeping".into(),
        'D' => "Disk sleep".into(),
        'Z' => "Zombie".into(),
        'T' | 't' => "Stopped".into(),
        'X' | 'x' => "Dead".into(),
        other => other.to_string(),
    }
}

pub fn snapshot_top_processes(limit: usize) -> Result<Vec<ProcessRow>, String> {
    let limit = limit.clamp(1, 200);
    #[cfg(windows)]
    {
        let _ = limit;
        Err("Process snapshot is supported on Linux hosts only".into())
    }
    #[cfg(not(windows))]
    {
        // user:32 avoids the short default USER column that truncates as "newst39+".
        let out = Command::new("ps")
            .args([
                "-eo",
                "user:32,pid,state,pcpu,pmem,vsz,rss,etime,args",
                "--sort=-pcpu",
            ])
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
            // user pid state pcpu pmem vsz rss etime args...
            if parts.len() < 9 {
                continue;
            }
            rows.push(ProcessRow {
                user: parts[0].to_string(),
                pid: parts[1].to_string(),
                state: map_state(parts[2]),
                cpu: parts[3].to_string(),
                mem: parts[4].to_string(),
                virt: format_kb_field(parts[5]),
                res: format_kb_field(parts[6]),
                time: parts[7].to_string(),
                command: parts[8..].join(" "),
            });
        }
        Ok(rows)
    }
}

#[cfg(not(windows))]
pub(crate) fn format_kb_field(raw: &str) -> String {
    let kb: u64 = raw.trim().parse().unwrap_or(0);
    if kb >= 1024 * 1024 {
        format!("{:.1}G", kb as f64 / (1024.0 * 1024.0))
    } else if kb >= 1024 {
        format!("{:.1}M", kb as f64 / 1024.0)
    } else {
        format!("{kb}K")
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

/// Validate a PID string (digits only, not 1, within u32 range).
pub fn parse_kill_pid(raw: &str) -> Result<u32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Err("PID must be a positive number.".into());
    }
    let pid: u32 = trimmed
        .parse()
        .map_err(|_| "PID is out of range.".to_string())?;
    if pid <= 1 {
        return Err("Refusing to signal PID 1 (init/systemd).".into());
    }
    Ok(pid)
}

/// Send SIGTERM (default) or SIGKILL to a PID. Admin gate belongs in the route.
pub fn kill_process(pid: u32, force: bool) -> Result<String, String> {
    #[cfg(windows)]
    {
        let _ = (pid, force);
        Err("Process kill is supported on Linux hosts only".into())
    }
    #[cfg(not(windows))]
    {
        let signal = if force { "KILL" } else { "TERM" };
        let out = Command::new("kill")
            .args([format!("-{signal}"), pid.to_string()])
            .output()
            .map_err(|e| format!("Failed to run kill: {e}"))?;
        if out.status.success() {
            return Ok(format!("Sent SIG{signal} to PID {pid}."));
        }
        let err = String::from_utf8_lossy(&out.stderr);
        let safe: String = err.chars().take(160).collect();
        Err(if safe.trim().is_empty() {
            format!("kill failed for PID {pid}.")
        } else {
            format!("kill failed for PID {pid}: {}", safe.trim())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_runs_or_explains() {
        match snapshot_top_processes(5) {
            Ok(rows) => {
                assert!(rows.len() <= 5);
                if let Some(row) = rows.first() {
                    assert!(!row.pid.is_empty());
                    assert!(!row.state.is_empty());
                }
            }
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

    #[test]
    fn parse_kill_pid_rejects_unsafe() {
        assert!(parse_kill_pid("1").is_err());
        assert!(parse_kill_pid("0").is_err());
        assert!(parse_kill_pid("abc").is_err());
        assert_eq!(parse_kill_pid("42").unwrap(), 42);
    }

    #[test]
    fn map_state_labels() {
        assert_eq!(map_state("R"), "Running");
        assert_eq!(map_state("S"), "Sleeping");
        assert_eq!(map_state("Z"), "Zombie");
    }
}
