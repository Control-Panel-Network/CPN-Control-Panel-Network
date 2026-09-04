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
}
