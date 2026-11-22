//! CyberPanel-style **light** email heal for install and upgrade.
//!
//! Safe, idempotent steps only (Email Start equivalent + webmail heal + config
//! validate before reload + hosted map sync + soft Cloudflare DNS).
//!
//! This path intentionally does **not** run a heavy Reset Email:
//! no blind DKIM key regen for all domains, no full Postfix/Dovecot template
//! overwrite, and no mailbox password rotation.

use crate::apps_email::{email_packages_installed, start_email_stack};
use crate::model::MailSystem;
use crate::panel_feature_gate;
use crate::service_detect::systemd_unit_active;
use std::path::Path;
use std::process::Command;

const POSTFIX_MAIN_CF: &str = "/etc/postfix/main.cf";
const DOVECOT_CONF: &str = "/etc/dovecot/dovecot.conf";

/// Pure gating inputs (unit-test friendly).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightHealGate {
    pub packages_ok: bool,
    pub postfix_main_cf: bool,
    pub dovecot_conf: bool,
    pub postfix_or_dovecot_active: bool,
    pub selected_mail_implies_local_stack: bool,
    pub webmail_installed: bool,
}

impl LightHealGate {
    pub fn should_run_mail_heal(self) -> bool {
        self.packages_ok
            || self.postfix_main_cf
            || self.dovecot_conf
            || self.postfix_or_dovecot_active
            || self.selected_mail_implies_local_stack
    }

    pub fn should_run_webmail_heal(self) -> bool {
        self.webmail_installed
    }
}

/// True when selected mail is a local IMAP/SMTP (+ webmail) path, not desktop-only.
pub fn selected_mail_implies_local_stack(mail: Option<MailSystem>) -> bool {
    matches!(
        mail,
        Some(
            MailSystem::Snappymail
                | MailSystem::Tachyon
                | MailSystem::Roundcube
                | MailSystem::Sogo
                | MailSystem::Nextsnapmail
        )
    )
}

fn selected_mail_from_manifest() -> Option<MailSystem> {
    crate::manifest::load_manifest().and_then(|m| m.selected_mail)
}

/// Live host signals for light heal gating.
pub fn detect_light_heal_gate() -> LightHealGate {
    LightHealGate {
        packages_ok: email_packages_installed(),
        postfix_main_cf: Path::new(POSTFIX_MAIN_CF).is_file(),
        dovecot_conf: Path::new(DOVECOT_CONF).is_file(),
        postfix_or_dovecot_active: systemd_unit_active("postfix") || systemd_unit_active("dovecot"),
        selected_mail_implies_local_stack: selected_mail_implies_local_stack(
            selected_mail_from_manifest(),
        ),
        webmail_installed: panel_feature_gate::webmail_installed(),
    }
}

pub fn should_run_light_email_heal() -> bool {
    detect_light_heal_gate().should_run_mail_heal()
}

pub fn should_run_webmail_heal() -> bool {
    detect_light_heal_gate().should_run_webmail_heal()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidateResult {
    pub ok: bool,
    pub detail: String,
}

/// Run `postfix check` when Postfix is present on the host.
pub fn validate_postfix_config() -> ConfigValidateResult {
    if !Path::new(POSTFIX_MAIN_CF).is_file() && !crate::panel_ops_security::which_exists("postfix")
    {
        return ConfigValidateResult {
            ok: true,
            detail: "postfix not present; skipped postfix check".into(),
        };
    }
    let output = Command::new("postfix").args(["check"]).output();
    match output {
        Ok(out) if out.status.success() => ConfigValidateResult {
            ok: true,
            detail: "postfix check ok".into(),
        },
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            let out_s = String::from_utf8_lossy(&out.stdout);
            let detail = format!("postfix check failed: {} {}", err.trim(), out_s.trim())
                .trim()
                .to_string();
            ConfigValidateResult {
                ok: false,
                detail: if detail.is_empty() {
                    "postfix check failed".into()
                } else {
                    detail
                },
            }
        }
        Err(e) => ConfigValidateResult {
            ok: false,
            detail: format!("postfix check could not run: {e}"),
        },
    }
}

/// Run `doveconf -n` when Dovecot config/binary is present.
pub fn validate_dovecot_config() -> ConfigValidateResult {
    if !Path::new(DOVECOT_CONF).is_file() && !crate::panel_ops_security::which_exists("doveconf") {
        return ConfigValidateResult {
            ok: true,
            detail: "dovecot not present; skipped doveconf -n".into(),
        };
    }
    let output = Command::new("doveconf").args(["-n"]).output();
    match output {
        Ok(out) if out.status.success() => ConfigValidateResult {
            ok: true,
            detail: "doveconf -n ok".into(),
        },
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            ConfigValidateResult {
                ok: false,
                detail: format!(
                    "doveconf -n failed: {}",
                    err.trim().chars().take(400).collect::<String>()
                ),
            }
        }
        Err(e) => ConfigValidateResult {
            ok: false,
            detail: format!("doveconf -n could not run: {e}"),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailReloadOutcome {
    pub postfix_reloaded: bool,
    pub dovecot_reloaded: bool,
    pub postfix_validate: ConfigValidateResult,
    pub dovecot_validate: ConfigValidateResult,
}

impl MailReloadOutcome {
    pub fn validate_ok(&self) -> bool {
        self.postfix_validate.ok && self.dovecot_validate.ok
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.postfix_validate.detail.clone());
        parts.push(self.dovecot_validate.detail.clone());
        if self.postfix_reloaded {
            parts.push("postfix reloaded".into());
        } else if !self.postfix_validate.ok {
            parts.push("postfix reload aborted".into());
        }
        if self.dovecot_reloaded {
            parts.push("dovecot reloaded".into());
        } else if !self.dovecot_validate.ok {
            parts.push("dovecot reload aborted".into());
        }
        parts.join("; ")
    }
}

/// Validate configs, then reload only the services that passed (CP preflight spirit).
pub fn reload_mail_services_validated() -> MailReloadOutcome {
    let postfix_validate = validate_postfix_config();
    let dovecot_validate = validate_dovecot_config();
    let mut postfix_reloaded = false;
    let mut dovecot_reloaded = false;

    if postfix_validate.ok && Path::new(POSTFIX_MAIN_CF).is_file() {
        let ok = Command::new("systemctl")
            .args(["reload", "postfix"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        postfix_reloaded = ok;
    }
    if dovecot_validate.ok && (Path::new(DOVECOT_CONF).is_file() || systemd_unit_active("dovecot"))
    {
        let ok = Command::new("timeout")
            .args(["8", "systemctl", "reload", "dovecot"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        dovecot_reloaded = ok;
    }

    MailReloadOutcome {
        postfix_reloaded,
        dovecot_reloaded,
        postfix_validate,
        dovecot_validate,
    }
}

#[derive(Debug, Clone, Default)]
pub struct LightHealReport {
    pub ran_mail: bool,
    pub ran_webmail: bool,
    pub skipped: bool,
    pub skip_reason: Option<String>,
    pub notes: Vec<String>,
    pub validate_ok: bool,
    pub ok: bool,
}

impl LightHealReport {
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.skipped {
            lines.push(format!(
                "light email heal skipped: {}",
                self.skip_reason.as_deref().unwrap_or("no mail stack")
            ));
            return lines;
        }
        lines.extend(self.notes.iter().cloned());
        lines
    }
}

/// Soft-publish Cloudflare mail DNS when a token is present (never hard-fails).
fn soft_publish_cloudflare_mail_dns() -> String {
    crate::panel_ops_mail_dns::provision_mail_dns_for_known_domains()
}

fn heal_webmail_fail_soft() -> String {
    match crate::install_webmail_heal::heal_webmail_loopback_config() {
        Ok(()) => "webmail loopback/prefs/sieve heal ok".into(),
        Err(e) => format!("webmail heal warning: {e}"),
    }
}

/// Run the light heal path. Fail-soft for optional steps; never rotates passwords;
/// never runs heavy Reset Email. Suitable for install, upgrade_verify, and panel start.
pub fn run_light_email_heal(context: &str) -> LightHealReport {
    let gate = detect_light_heal_gate();
    if !gate.should_run_mail_heal() && !gate.should_run_webmail_heal() {
        return LightHealReport {
            skipped: true,
            skip_reason: Some("mail stack and webmail not present".into()),
            ok: true,
            validate_ok: true,
            ..LightHealReport::default()
        };
    }

    let mut report = LightHealReport {
        ok: true,
        validate_ok: true,
        ..LightHealReport::default()
    };
    report.notes.push(format!("light email heal ({context})"));

    if gate.should_run_mail_heal() {
        report.ran_mail = true;
        match start_email_stack() {
            Ok(msg) => {
                report.notes.push(msg);
                // start_email_stack already validated and reloaded on success.
                let postfix_v = validate_postfix_config();
                let dovecot_v = validate_dovecot_config();
                report.validate_ok = postfix_v.ok && dovecot_v.ok;
                report.notes.push(format!(
                    "config validate: {}; {}",
                    postfix_v.detail, dovecot_v.detail
                ));
            }
            Err(e) => {
                // Fail-soft on upgrade/install heal: keep going with validate + maps.
                report.ok = false;
                report.notes.push(format!("email stack heal warning: {e}"));
                // Attempt a validated reload only (may abort reload on hard failure).
                let reload = reload_mail_services_validated();
                report.validate_ok = reload.validate_ok();
                if !reload.validate_ok() {
                    report.ok = false;
                }
                report.notes.push(reload.summary());
            }
        }

        crate::mail_hosted_domains::sync_hosted_mail_delivery_logged(&format!(
            "light email heal ({context})"
        ));
        report
            .notes
            .push(format!("hosted mail maps sync ({context})"));

        match crate::mail_home_layout::migrate_legacy_top_level_homes(false) {
            Ok(mig) => {
                report.notes.push(format!(
                    "mailbox home layout: scanned={} migrated={} skipped={}",
                    mig.scanned, mig.migrated, mig.skipped
                ));
                if mig.migrated > 0 {
                    crate::mail_hosted_domains::sync_hosted_mail_delivery_logged(
                        "mailbox home migrate (light heal)",
                    );
                }
            }
            Err(e) => report
                .notes
                .push(format!("mailbox home layout warning: {e}")),
        }

        let dns = soft_publish_cloudflare_mail_dns();
        report.notes.push(format!("mail DNS: {dns}"));
    }

    if gate.should_run_webmail_heal() {
        report.ran_webmail = true;
        report.notes.push(heal_webmail_fail_soft());
    }

    report
}

/// Convenience for background panel-start / upgrade threads (logs via eprintln).
pub fn run_light_email_heal_logged(context: &str) {
    let report = run_light_email_heal(context);
    for line in report.summary_lines() {
        eprintln!("cpn-installer: {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_skips_when_no_mail_signals() {
        let gate = LightHealGate {
            packages_ok: false,
            postfix_main_cf: false,
            dovecot_conf: false,
            postfix_or_dovecot_active: false,
            selected_mail_implies_local_stack: false,
            webmail_installed: false,
        };
        assert!(!gate.should_run_mail_heal());
        assert!(!gate.should_run_webmail_heal());
    }

    #[test]
    fn gate_runs_mail_when_packages_present() {
        let gate = LightHealGate {
            packages_ok: true,
            postfix_main_cf: false,
            dovecot_conf: false,
            postfix_or_dovecot_active: false,
            selected_mail_implies_local_stack: false,
            webmail_installed: false,
        };
        assert!(gate.should_run_mail_heal());
        assert!(!gate.should_run_webmail_heal());
    }

    #[test]
    fn gate_runs_mail_when_postfix_main_cf_present() {
        let gate = LightHealGate {
            packages_ok: false,
            postfix_main_cf: true,
            dovecot_conf: false,
            postfix_or_dovecot_active: false,
            selected_mail_implies_local_stack: false,
            webmail_installed: false,
        };
        assert!(gate.should_run_mail_heal());
    }

    #[test]
    fn gate_runs_mail_when_dovecot_conf_or_units_active() {
        let via_conf = LightHealGate {
            packages_ok: false,
            postfix_main_cf: false,
            dovecot_conf: true,
            postfix_or_dovecot_active: false,
            selected_mail_implies_local_stack: false,
            webmail_installed: false,
        };
        assert!(via_conf.should_run_mail_heal());

        let via_unit = LightHealGate {
            packages_ok: false,
            postfix_main_cf: false,
            dovecot_conf: false,
            postfix_or_dovecot_active: true,
            selected_mail_implies_local_stack: false,
            webmail_installed: false,
        };
        assert!(via_unit.should_run_mail_heal());
    }

    #[test]
    fn gate_runs_mail_for_local_webmail_selection() {
        assert!(selected_mail_implies_local_stack(Some(MailSystem::Tachyon)));
        assert!(selected_mail_implies_local_stack(Some(
            MailSystem::Snappymail
        )));
        assert!(selected_mail_implies_local_stack(Some(
            MailSystem::Roundcube
        )));
        assert!(selected_mail_implies_local_stack(Some(MailSystem::Sogo)));
        assert!(!selected_mail_implies_local_stack(Some(
            MailSystem::Thunderbird
        )));
        assert!(!selected_mail_implies_local_stack(None));

        let gate = LightHealGate {
            packages_ok: false,
            postfix_main_cf: false,
            dovecot_conf: false,
            postfix_or_dovecot_active: false,
            selected_mail_implies_local_stack: true,
            webmail_installed: true,
        };
        assert!(gate.should_run_mail_heal());
        assert!(gate.should_run_webmail_heal());
    }

    #[test]
    fn gate_webmail_only_still_heals_webmail() {
        let gate = LightHealGate {
            packages_ok: false,
            postfix_main_cf: false,
            dovecot_conf: false,
            postfix_or_dovecot_active: false,
            selected_mail_implies_local_stack: false,
            webmail_installed: true,
        };
        assert!(!gate.should_run_mail_heal());
        assert!(gate.should_run_webmail_heal());
    }

    #[test]
    fn reload_outcome_summary_mentions_abort_on_validate_fail() {
        let outcome = MailReloadOutcome {
            postfix_reloaded: false,
            dovecot_reloaded: false,
            postfix_validate: ConfigValidateResult {
                ok: false,
                detail: "postfix check failed".into(),
            },
            dovecot_validate: ConfigValidateResult {
                ok: true,
                detail: "doveconf -n ok".into(),
            },
        };
        assert!(!outcome.validate_ok());
        let s = outcome.summary();
        assert!(s.contains("postfix reload aborted"));
        assert!(s.contains("postfix check failed"));
    }

    #[test]
    fn skipped_report_summary_is_explicit() {
        let report = LightHealReport {
            skipped: true,
            skip_reason: Some("mail stack and webmail not present".into()),
            ok: true,
            validate_ok: true,
            ..LightHealReport::default()
        };
        let lines = report.summary_lines();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("skipped"));
    }
}
