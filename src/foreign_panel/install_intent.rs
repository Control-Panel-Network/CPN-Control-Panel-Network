//! Operator choice during install: fresh stack vs migrate from a detected foreign panel.

use super::detect::{Detection, detect_at, host_root};
use super::install_notice::detect_foreign_panels;
use crate::http_helpers::now_unix;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallPathMode {
    Fresh,
    Migrate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPathRecord {
    pub mode: InstallPathMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_panel_id: Option<String>,
    #[serde(default)]
    pub chosen_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ForeignPanelPublic {
    pub id: String,
    pub name: String,
    pub tier: String,
    #[serde(default)]
    pub website_count: usize,
}

pub fn install_path_file() -> PathBuf {
    paths::join_data("foreign_panel_install.json")
}

fn write_json_600(path: &PathBuf, value: &serde_json::Value) -> Result<(), String> {
    let dir = paths::default_data_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let serialized = serde_json::to_vec_pretty(value)
        .map_err(|e| format!("Could not encode install path: {e}"))?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(&serialized)
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn load_install_path() -> Option<InstallPathRecord> {
    let path = install_path_file();
    let raw = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_install_path(record: &InstallPathRecord) -> Result<(), String> {
    let path = install_path_file();
    let value =
        serde_json::to_value(record).map_err(|e| format!("Could not encode install path: {e}"))?;
    write_json_600(&path, &value)
}

pub fn foreign_panels_public() -> Vec<ForeignPanelPublic> {
    detect_foreign_panels()
        .into_iter()
        .map(|row| ForeignPanelPublic {
            id: row.id,
            name: row.name,
            tier: row.tier,
            website_count: row.inventory.sites.len(),
        })
        .collect()
}

pub fn parse_install_path_flag(args: &[String]) -> Option<InstallPathMode> {
    let migrate = args
        .iter()
        .any(|arg| arg == "--migrate" || arg == "--install-migrate");
    let fresh = args
        .iter()
        .any(|arg| arg == "--fresh" || arg == "--install-fresh");
    if migrate && fresh {
        return None;
    }
    if migrate {
        return Some(InstallPathMode::Migrate);
    }
    if fresh {
        return Some(InstallPathMode::Fresh);
    }
    None
}

pub fn apply_cli_flags(args: &[String]) -> Result<(), String> {
    let Some(mode) = parse_install_path_flag(args) else {
        return Ok(());
    };
    let panels = detect_foreign_panels();
    let target = if mode == InstallPathMode::Migrate {
        default_migrate_target(&panels)
    } else {
        None
    };
    save_install_path(&InstallPathRecord {
        mode,
        target_panel_id: target,
        chosen_at_unix: now_unix(),
    })
}

pub fn default_migrate_target(panels: &[Detection]) -> Option<String> {
    panels
        .iter()
        .find(|row| row.id == "classic")
        .or_else(|| panels.first())
        .map(|row| row.id.clone())
}

pub fn migrate_apply_command(target_id: &str) -> String {
    format!(
        "cpn panel-migrate checklist && cpn panel-migrate detect && cpn panel-migrate plan --id {target_id} && cpn panel-migrate apply --id {target_id} --yes"
    )
}

pub fn post_install_hint(mode: InstallPathMode, target_id: Option<&str>) -> Option<String> {
    match mode {
        InstallPathMode::Fresh => None,
        InstallPathMode::Migrate => {
            let id = target_id.unwrap_or("classic");
            Some(format!(
                "Migration install path selected. After login, open Server → Panel migrate in the UI, or run:\n  {}",
                migrate_apply_command(id)
            ))
        }
    }
}

pub fn prompt_cli_install_path(panels: &[Detection]) -> Result<InstallPathRecord, String> {
    if panels.is_empty() {
        return Ok(InstallPathRecord {
            mode: InstallPathMode::Fresh,
            target_panel_id: None,
            chosen_at_unix: now_unix(),
        });
    }
    println!("\nAnother hosting panel was detected on this server.");
    for row in panels {
        println!(
            "  - id={} name={} tier={} (sites mapped: {})",
            row.id,
            row.name,
            row.tier,
            row.inventory.sites.len()
        );
    }
    println!("\nHow should CPN install on this host?");
    println!("  1) Fresh install   (CPN stack only; may replace overlapping packages)");
    println!(
        "  2) Migrate path    (keep LiteSpeed/postfix/Dovecot; run panel-migrate after CPN is ready)"
    );
    loop {
        let raw =
            prompt_trimmed("Enter choice [1/2] (default: 2 when CyberPanel/classic is present): ")?;
        let default_migrate = panels.iter().any(|p| p.id == "classic");
        let choice = if raw.is_empty() {
            if default_migrate { "2" } else { "1" }
        } else {
            raw.as_str()
        };
        match choice {
            "1" | "fresh" | "f" => {
                return Ok(InstallPathRecord {
                    mode: InstallPathMode::Fresh,
                    target_panel_id: None,
                    chosen_at_unix: now_unix(),
                });
            }
            "2" | "migrate" | "m" => {
                let target = pick_migrate_target_cli(panels)?;
                return Ok(InstallPathRecord {
                    mode: InstallPathMode::Migrate,
                    target_panel_id: Some(target),
                    chosen_at_unix: now_unix(),
                });
            }
            other => eprintln!("error: Unknown choice `{other}`. Enter 1 (fresh) or 2 (migrate)."),
        }
    }
}

fn pick_migrate_target_cli(panels: &[Detection]) -> Result<String, String> {
    if panels.len() == 1 {
        return Ok(panels[0].id.clone());
    }
    if let Some(classic) = panels.iter().find(|p| p.id == "classic") {
        println!(
            "Default migrate target: classic ({}) [Enter to accept]",
            classic.name
        );
        let raw = prompt_trimmed("Panel id to migrate from: ")?;
        if raw.is_empty() {
            return Ok(classic.id.clone());
        }
        if panels.iter().any(|p| p.id == raw) {
            return Ok(raw);
        }
        return Err(format!(
            "Unknown panel id `{raw}`. Run cpn panel-migrate detect for ids."
        ));
    }
    println!("Available panel ids:");
    for row in panels {
        println!("  {} ({})", row.id, row.name);
    }
    loop {
        let raw = prompt_trimmed("Panel id to migrate from: ")?;
        if panels.iter().any(|p| p.id == raw) {
            return Ok(raw);
        }
        eprintln!("error: Unknown panel id `{raw}`.");
    }
}

fn prompt_trimmed(prompt: &str) -> Result<String, String> {
    use std::io::{self, Write};
    eprint!("{prompt}");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("Failed to read input: {e}"))?;
    Ok(line.trim().to_string())
}

/// Foreign panels on the live root for dry-run tests (`CPN_FOREIGN_PANEL_ROOT`).
pub fn detected_panels() -> Vec<Detection> {
    detect_at(&host_root())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_parse_rejects_both() {
        let args = vec!["cpn-installer".into(), "--migrate".into(), "--fresh".into()];
        assert!(parse_install_path_flag(&args).is_none());
    }

    #[test]
    fn migrate_hint_includes_panel_migrate() {
        let hint = post_install_hint(InstallPathMode::Migrate, Some("classic")).unwrap();
        assert!(hint.contains("panel-migrate apply --id classic"));
    }
}
