//! `cpn doctor` / System Repair CLI entry (aliases: troubleshoot, repair, system-repair).

use crate::system_repair::{self, PRODUCT_NAME, RepairReport};

/// Run System Repair. `heal` runs safe heals first. Optional `id` filters check or heal.
pub fn run(heal: bool, json: bool, id: Option<&str>) -> Result<(), String> {
    if heal {
        if let Some(note) = crate::panel_maintenance_mode::heal_stuck() {
            if !json {
                println!("heal: {note}");
            }
        }
    }
    let filter = id.map(str::trim).filter(|s| !s.is_empty());
    let report = system_repair::run_suite(heal, filter, filter);
    emit(&report, json)
}

/// Explicit check subcommand.
pub fn run_check(json: bool, id: Option<&str>) -> Result<(), String> {
    let filter = id.map(str::trim).filter(|s| !s.is_empty());
    let report = system_repair::run_suite(false, None, filter);
    emit(&report, json)
}

/// Explicit heal subcommand (then re-check).
pub fn run_heal(json: bool, id: Option<&str>) -> Result<(), String> {
    if let Some(note) = crate::panel_maintenance_mode::heal_stuck() {
        if !json {
            println!("heal: {note}");
        }
    }
    let filter = id.map(str::trim).filter(|s| !s.is_empty());
    let report = system_repair::run_suite(true, filter, None);
    emit(&report, json)
}

fn emit(report: &RepairReport, json: bool) -> Result<(), String> {
    if json {
        println!("{}", report.to_json()?);
        if report.exit_code() != 0 {
            return Err(format!("{PRODUCT_NAME} reported required failures"));
        }
        return Ok(());
    }
    let code = system_repair::print_human(report);
    if code != 0 {
        return Err(format!("{PRODUCT_NAME} reported required failures"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_name_stable() {
        assert_eq!(PRODUCT_NAME, "System Repair");
    }
}
