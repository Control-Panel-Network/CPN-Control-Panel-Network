//! Apply package CPU/memory limits to website Unix accounts (systemd user slices).

use crate::package_limits::is_unlimited;
use crate::packages::package_for_account;
use crate::panel_subusers::resource_owner_for;
use crate::sites::{SiteRecord, list_sites};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const DROPIN_MARKER: &str = "# CPN package host resource limits";

fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn unix_uid(username: &str) -> Option<u32> {
    if username.trim().is_empty() {
        return None;
    }
    let output = Command::new("id")
        .args(["-u", username.trim()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()
}

fn dropin_path_for_uid(uid: u32) -> PathBuf {
    PathBuf::from(format!(
        "/etc/systemd/system/user-{uid}.slice.d/cpn-host-resources.conf"
    ))
}

fn build_dropin_body(cpu_percent: i64, memory_mb: i64) -> String {
    let mut lines = vec![DROPIN_MARKER.to_string(), "[Slice]".to_string()];
    if !is_unlimited(cpu_percent) && cpu_percent > 0 {
        lines.push(format!("CPUQuota={cpu_percent}%"));
    }
    if !is_unlimited(memory_mb) && memory_mb > 0 {
        lines.push(format!("MemoryMax={memory_mb}M"));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn write_dropin(uid: u32, body: &str) -> Result<(), String> {
    let path = dropin_path_for_uid(uid);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644);
    }
    use std::io::Write;
    let mut file = options
        .open(&path)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.write_all(body.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

fn remove_dropin(uid: u32) -> Result<bool, String> {
    let path = dropin_path_for_uid(uid);
    if !path.is_file() {
        return Ok(false);
    }
    fs::remove_file(&path).map_err(|e| format!("remove {}: {e}", path.display()))?;
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
    Ok(true)
}

fn limits_active(cpu: i64, mem: i64) -> bool {
    (!is_unlimited(cpu) && cpu > 0) || (!is_unlimited(mem) && mem > 0)
}

fn systemctl_daemon_reload() -> Result<(), String> {
    let status = Command::new("systemctl")
        .args(["daemon-reload"])
        .status()
        .map_err(|e| format!("systemctl daemon-reload: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("systemctl daemon-reload failed".into())
    }
}

/// Limits from the package assigned to `account` (parent owner for sub-users).
pub fn host_limits_for_account(account: &str) -> Result<(i64, i64), String> {
    let owner = resource_owner_for(account);
    let pkg = package_for_account(&owner)?;
    Ok((pkg.host_cpu_percent, pkg.host_memory_mb))
}

fn apply_unix_user_limits(username: &str, cpu: i64, mem: i64) -> Result<String, String> {
    let Some(uid) = unix_uid(username) else {
        return Ok(format!("skip `{username}`: no Unix uid"));
    };
    let reload = if limits_active(cpu, mem) {
        let body = build_dropin_body(cpu, mem);
        write_dropin(uid, &body)?;
        true
    } else if remove_dropin(uid)? {
        true
    } else {
        false
    };
    if reload {
        systemctl_daemon_reload()?;
    }
    if limits_active(cpu, mem) {
        Ok(format!(
            "applied CPU={cpu}% MemoryMax={mem}M to user-{uid}.slice (`{username}`)"
        ))
    } else {
        Ok(format!("cleared limits for user-{uid}.slice (`{username}`)"))
    }
}

/// Apply package limits for one site's Unix account (owner package).
pub fn apply_site_package_resources(site: &SiteRecord) -> Result<String, String> {
    if cfg!(windows) || !is_root() {
        return Ok("skipped (not Linux root)".into());
    }
    let Some(user) = site
        .unix_user
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    else {
        return Ok(format!("skip {}: no website Unix user", site.domain));
    };
    let (cpu, mem) = host_limits_for_account(&site.owner)?;
    let msg = apply_unix_user_limits(user, cpu, mem)?;
    let _ = crate::panel_site_vhost_wire::refresh_site_php_resource_limits(site, cpu, mem);
    Ok(msg)
}

/// Re-apply limits for every site owned by accounts on this package.
pub fn apply_package_resources_for_id(package_id: &str) -> Result<Vec<String>, String> {
    let accounts = crate::packages::accounts_assigned_to(package_id)?;
    let sites = list_sites().unwrap_or_default();
    let mut out = Vec::new();
    for site in sites {
        let owner = resource_owner_for(&site.owner);
        if !accounts.iter().any(|a| a.eq_ignore_ascii_case(&owner)) {
            continue;
        }
        match apply_site_package_resources(&site) {
            Ok(msg) => out.push(format!("{}: {msg}", site.domain)),
            Err(e) => out.push(format!("{}: {e}", site.domain)),
        }
    }
    Ok(out)
}

/// Heal path: panel reserve + sync all registered sites.
pub fn sync_all_site_package_resources() -> Result<String, String> {
    if cfg!(windows) || !is_root() {
        return Err("package resource sync requires Linux root".into());
    }
    let sites = list_sites()?;
    let mut ok = 0usize;
    let mut notes = Vec::new();
    for site in &sites {
        match apply_site_package_resources(site) {
            Ok(msg) => {
                ok += 1;
                if !msg.starts_with("skip") {
                    notes.push(format!("{}: {msg}", site.domain));
                }
            }
            Err(e) => notes.push(format!("{}: {e}", site.domain)),
        }
    }
    Ok(format!(
        "synced {ok}/{} site(s). {}",
        sites.len(),
        if notes.is_empty() {
            "no changes".into()
        } else {
            notes.join("; ")
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_limits::UNLIMITED;

    #[test]
    fn dropin_includes_quota_when_limited() {
        let body = build_dropin_body(150, 512);
        assert!(body.contains("CPUQuota=150%"));
        assert!(body.contains("MemoryMax=512M"));
        assert!(body.contains(DROPIN_MARKER));
        assert!(!body.contains('\u{2014}'));
    }

    #[test]
    fn dropin_clears_when_unlimited() {
        assert!(!limits_active(UNLIMITED, UNLIMITED));
        let body = build_dropin_body(UNLIMITED, 256);
        assert!(!body.contains("CPUQuota"));
        assert!(body.contains("MemoryMax=256M"));
    }
}