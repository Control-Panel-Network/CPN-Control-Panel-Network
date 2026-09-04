//! Read-only package manager status (dnf/apt).

use std::process::Command;

#[derive(Debug, Clone)]
pub struct PkgMgrStatus {
    pub tool: Option<String>,
    pub detail: String,
    pub sample: Vec<String>,
}

pub fn package_manager_status(query: &str) -> PkgMgrStatus {
    let query = query.trim();
    if let Ok(out) = Command::new("dnf").args(["--version"]).output()
        && out.status.success()
    {
        let sample = if query.is_empty() {
            Command::new("dnf")
                .args(["list", "installed"])
                .output()
                .ok()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .skip(1)
                        .take(30)
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            // Search only; never install from this panel path yet.
            Command::new("dnf")
                .args(["search", query])
                .output()
                .ok()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .take(40)
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };
        return PkgMgrStatus {
            tool: Some("dnf".into()),
            detail: "Read-only listing/search. Install allowlist comes in a later release.".into(),
            sample,
        };
    }
    if let Ok(out) = Command::new("apt-cache").args(["--version"]).output()
        && out.status.success()
    {
        return PkgMgrStatus {
            tool: Some("apt".into()),
            detail: "apt detected. Read-only search stub; installs are not enabled yet.".into(),
            sample: vec![],
        };
    }
    PkgMgrStatus {
        tool: None,
        detail: "No supported package manager (dnf/apt) detected".into(),
        sample: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_runs() {
        let s = package_manager_status("");
        assert!(!s.detail.is_empty());
    }
}
