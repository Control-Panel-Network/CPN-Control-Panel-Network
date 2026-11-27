//! Guaranteed panel slice and service drop-in (all installs, idempotent).

use crate::panel_service::UNIT_NAME;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const PANEL_SLICE_NAME: &str = "cpn-panel.slice";
pub const PANEL_SLICE_PATH: &str = "/etc/systemd/system/cpn-panel.slice";
pub const PANEL_SERVICE_DROPIN_DIR: &str = "/etc/systemd/system/cpn-installer.service.d";
pub const PANEL_SERVICE_SLICE_DROPIN: &str =
    "/etc/systemd/system/cpn-installer.service.d/20-panel-resource-reserve.conf";

/// Documented defaults: CPUWeight 200 vs default 100, MemoryMin 256M for the slice.
pub const PANEL_SLICE_UNIT: &str = "\
[Unit]
Description=CPN panel reserved resources
Documentation=https://github.com/Control-Panel-Network/CPN-Control-Panel-Network
Before=slices.target

[Slice]
CPUWeight=200
IOWeight=200
MemoryMin=256M
";

pub const PANEL_SERVICE_SLICE_DROPIN_BODY: &str = "\
# Managed by CPN. Panel stays responsive under site load.
[Service]
Slice=cpn-panel.slice
CPUWeight=200
";

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

fn write_mode(path: &Path, body: &str, mode: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }
    use std::io::Write;
    let mut file = options
        .open(path)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.write_all(body.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
    Ok(())
}

fn file_matches(path: &Path, expected: &str) -> bool {
    fs::read_to_string(path)
        .map(|s| s.trim() == expected.trim())
        .unwrap_or(false)
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

/// Idempotent: slice unit + service drop-in + daemon-reload (Linux root only).
pub fn ensure_panel_resource_reserve() -> Result<String, String> {
    if cfg!(windows) {
        return Ok("panel resource reserve skipped on Windows".into());
    }
    if !is_root() {
        return Err("panel resource reserve requires root".into());
    }
    let slice_path = Path::new(PANEL_SLICE_PATH);
    let dropin_path = Path::new(PANEL_SERVICE_SLICE_DROPIN);
    let mut changed = false;
    if !file_matches(slice_path, PANEL_SLICE_UNIT) {
        write_mode(slice_path, PANEL_SLICE_UNIT, 0o644)?;
        changed = true;
    }
    if !file_matches(dropin_path, PANEL_SERVICE_SLICE_DROPIN_BODY) {
        write_mode(dropin_path, PANEL_SERVICE_SLICE_DROPIN_BODY, 0o644)?;
        changed = true;
    }
    if changed {
        systemctl_daemon_reload()?;
    }
    let active = Command::new("systemctl")
        .args(["is-active", "--quiet", PANEL_SLICE_NAME])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let unit_active = Command::new("systemctl")
        .args(["is-active", "--quiet", UNIT_NAME])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    Ok(format!(
        "panel slice {PANEL_SLICE_NAME} configured{}; slice active={active}; {UNIT_NAME} active={unit_active}",
        if changed { " (updated)" } else { " (ok)" },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_slice_has_no_em_dash() {
        assert!(PANEL_SLICE_UNIT.contains("CPUWeight"));
        assert!(PANEL_SERVICE_SLICE_DROPIN_BODY.contains("cpn-panel.slice"));
        assert!(!PANEL_SLICE_UNIT.contains('\u{2014}'));
        assert!(!PANEL_SERVICE_SLICE_DROPIN_BODY.contains('\u{2014}'));
    }
}