//! System Repair: shared diagnostic + safe heal suite for owners and `cpn doctor`.
//!
//! Product name in the panel UI is **System Repair**. The CLI keeps `cpn doctor`
//! and accepts aliases `troubleshoot`, `repair`, and `system-repair`.

use serde::Serialize;

mod email;
mod host;
mod host_apps;
mod panel_core;

pub use email::EMAIL_HEAL_IDS;
pub use host::HOST_HEAL_IDS;
pub use panel_core::CORE_HEAL_IDS;

/// Business-facing product name (panel UI, report headers).
pub const PRODUCT_NAME: &str = "System Repair";

/// CLI primary command (also documented as doctor).
pub const CLI_COMMAND: &str = "doctor";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairCheck {
    pub id: String,
    pub category: String,
    pub title: String,
    pub status: CheckStatus,
    pub detail: String,
    /// When true, a failed check fails the overall CLI exit code.
    pub required: bool,
    /// Non-empty when a safe heal exists for this check id (or a parent heal id).
    pub heal_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealResult {
    pub heal_id: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairReport {
    pub product: String,
    pub checks: Vec<RepairCheck>,
    pub heals: Vec<HealResult>,
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
    pub required_failures: usize,
}

impl RepairReport {
    pub fn from_checks(checks: Vec<RepairCheck>, heals: Vec<HealResult>) -> Self {
        let mut pass = 0usize;
        let mut warn = 0usize;
        let mut fail = 0usize;
        let mut required_failures = 0usize;
        for c in &checks {
            match c.status {
                CheckStatus::Pass => pass += 1,
                CheckStatus::Warn => warn += 1,
                CheckStatus::Fail => {
                    fail += 1;
                    if c.required {
                        required_failures += 1;
                    }
                }
            }
        }
        Self {
            product: PRODUCT_NAME.into(),
            checks,
            heals,
            pass,
            warn,
            fail,
            required_failures,
        }
    }

    pub fn exit_code(&self) -> i32 {
        if self.required_failures == 0 { 0 } else { 1 }
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| format!("Could not encode System Repair JSON: {e}"))
    }
}

#[allow(clippy::too_many_arguments)]
fn push(
    checks: &mut Vec<RepairCheck>,
    id: &str,
    category: &str,
    title: &str,
    status: CheckStatus,
    detail: impl Into<String>,
    required: bool,
    heal_id: Option<&str>,
) {
    checks.push(RepairCheck {
        id: id.into(),
        category: category.into(),
        title: title.into(),
        status,
        detail: detail.into(),
        required,
        heal_id: heal_id.map(str::to_string),
    });
}

/// Run every System Repair check (read-only).
pub fn run_checks(filter_id: Option<&str>) -> Vec<RepairCheck> {
    let mut checks = Vec::new();
    panel_core::collect(&mut checks);
    email::collect(&mut checks);
    host::collect(&mut checks);
    if let Some(id) = filter_id.map(str::trim).filter(|s| !s.is_empty()) {
        checks.retain(|c| c.id == id || c.heal_id.as_deref() == Some(id) || c.category == id);
    }
    checks
}

/// Safe heals. Never clears MFA. When `heal_id` is None, runs all known heals that apply.
pub fn run_heals(heal_id: Option<&str>) -> Vec<HealResult> {
    let want = heal_id.map(str::trim).filter(|s| !s.is_empty());
    let mut out = Vec::new();
    for id in CORE_HEAL_IDS
        .iter()
        .chain(EMAIL_HEAL_IDS.iter())
        .chain(HOST_HEAL_IDS.iter())
    {
        if let Some(w) = want
            && w != *id
        {
            continue;
        }
        let result = match *id {
            "cli.local_override" => panel_core::heal_local_override(),
            "panel.service" => panel_core::heal_panel_service(),
            "email.stack" => email::heal_email_stack(),
            "email.firewall" => email::heal_email_firewall(),
            "firewall" => host::heal_firewall(),
            "phpmyadmin" => host::heal_phpmyadmin(),
            "docker.engine" => host::heal_docker_engine(),
            other => HealResult {
                heal_id: other.into(),
                ok: false,
                message: format!("Unknown heal id: {other}"),
            },
        };
        out.push(result);
    }
    if out.is_empty()
        && let Some(w) = want
    {
        out.push(HealResult {
            heal_id: w.into(),
            ok: false,
            message: format!("No heal registered for id `{w}`"),
        });
    }
    out
}

/// Heal (optional), then re-check. Used by CLI and panel.
pub fn run_suite(heal: bool, heal_id: Option<&str>, filter_id: Option<&str>) -> RepairReport {
    let heals = if heal { run_heals(heal_id) } else { Vec::new() };
    let checks = run_checks(filter_id);
    RepairReport::from_checks(checks, heals)
}

/// Human-readable CLI report. Returns process exit code.
pub fn print_human(report: &RepairReport) -> i32 {
    println!("{} report", PRODUCT_NAME);
    println!();
    for h in &report.heals {
        let mark = if h.ok { "heal-ok" } else { "heal-fail" };
        println!("[{mark}] {}: {}", h.heal_id, h.message);
    }
    if !report.heals.is_empty() {
        println!();
    }
    for c in &report.checks {
        let mark = match c.status {
            CheckStatus::Pass => "pass",
            CheckStatus::Warn => "warn",
            CheckStatus::Fail => "FAIL",
        };
        let heal = c
            .heal_id
            .as_ref()
            .map(|id| format!(" heal={id}"))
            .unwrap_or_default();
        println!("[{mark}] {}: {} ({}){heal}", c.id, c.title, c.detail);
    }
    println!();
    if report.required_failures == 0 {
        println!(
            "{}: PASS ({} pass, {} warn, {} fail)",
            PRODUCT_NAME, report.pass, report.warn, report.fail
        );
    } else {
        println!(
            "{}: FAIL ({} required; {} pass, {} warn, {} fail). Try: sudo cpn doctor --heal",
            PRODUCT_NAME, report.required_failures, report.pass, report.warn, report.fail
        );
    }
    report.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_name_is_system_repair() {
        assert_eq!(PRODUCT_NAME, "System Repair");
        assert!(!PRODUCT_NAME.contains('\u{2014}'));
        assert!(!PRODUCT_NAME.to_lowercase().contains("cyberpanel"));
    }

    #[test]
    fn report_counts_statuses() {
        let checks = vec![
            RepairCheck {
                id: "a".into(),
                category: "x".into(),
                title: "A".into(),
                status: CheckStatus::Pass,
                detail: "ok".into(),
                required: true,
                heal_id: None,
            },
            RepairCheck {
                id: "b".into(),
                category: "x".into(),
                title: "B".into(),
                status: CheckStatus::Fail,
                detail: "bad".into(),
                required: true,
                heal_id: Some("panel.service".into()),
            },
            RepairCheck {
                id: "c".into(),
                category: "x".into(),
                title: "C".into(),
                status: CheckStatus::Warn,
                detail: "meh".into(),
                required: false,
                heal_id: None,
            },
        ];
        let report = RepairReport::from_checks(checks, vec![]);
        assert_eq!(report.pass, 1);
        assert_eq!(report.warn, 1);
        assert_eq!(report.fail, 1);
        assert_eq!(report.required_failures, 1);
        assert_eq!(report.exit_code(), 1);
        let json = report.to_json().unwrap();
        assert!(json.contains("System Repair"));
        assert!(json.contains("\"fail\""));
    }
}
