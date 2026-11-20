//! Installer-facing migration hints when another panel is already on this host.

use super::catalog::spec_by_id;
use super::detect::{detect_at, host_root, Detection};

/// Panels detected on the live host root (not dry-run).
pub fn detect_foreign_panels() -> Vec<Detection> {
    detect_at(&host_root())
}

pub fn format_detection_banner(panels: &[Detection]) -> Option<String> {
    if panels.is_empty() {
        return None;
    }
    let mut lines = vec![
        String::from(
            "Another hosting panel was detected on this server. CPN can migrate in place instead of overwriting stacks.",
        ),
    ];
    for row in panels {
        let port = spec_by_id(&row.id)
            .map(|spec| spec.listen_port.to_string())
            .unwrap_or_else(|| "?".into());
        lines.push(format!(
            "  - id={} name={} tier={} (typical port {})",
            row.id, row.name, row.tier, port
        ));
    }
    if panels.iter().any(|p| p.id == "classic") {
        lines.push(String::from(
            "CyberPanel maps to id=classic. After CPN finishes, run: cpn panel-migrate checklist && cpn panel-migrate detect && cpn panel-migrate plan --id classic && cpn panel-migrate apply --id classic --yes",
        ));
        lines.push(String::from(
            "Install CPN on a different listen port if CyberPanel still uses 8090, then migrate before wipe.",
        ));
    } else {
        lines.push(String::from(
            "Run: cpn panel-migrate detect   then plan/apply for the listed id.",
        ));
    }
    Some(lines.join("\n"))
}

pub fn print_install_migration_notice() {
    if let Some(banner) = format_detection_banner(&detect_foreign_panels()) {
        eprintln!("\n=== CPN panel migration ===\n{banner}\n");
    }
}