//! Email listener and firewall checks for System Repair.

use super::{CheckStatus, HealResult, push};
use crate::apps_email::{detect_mail_stack_public, email_packages_installed, start_email_stack};
use crate::panel_ops_firewall_heal;
use crate::service_detect::{port_open, systemd_unit_active};

pub const EMAIL_HEAL_IDS: &[&str] = &["email.stack", "email.firewall"];

const MAIL_PORTS: &[(u16, &str, &str)] = &[
    (25, "SMTP", "email.port.25"),
    (587, "SMTP submission", "email.port.587"),
    (465, "SMTPS", "email.port.465"),
    (143, "IMAP", "email.port.143"),
    (993, "IMAPS", "email.port.993"),
    (110, "POP3", "email.port.110"),
    (995, "POP3S", "email.port.995"),
    (4190, "ManageSieve", "email.port.4190"),
];

fn selinux_note() -> Option<String> {
    let out = crate::panel_ops_docker_probe::output_with_timeout(
        "getenforce",
        &[],
        std::time::Duration::from_secs(2),
    )
    .ok()?;
    let mode = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if mode.eq_ignore_ascii_case("Enforcing") {
        Some(
            "SELinux is Enforcing; CPN applies httpd mail-port allow rules on Email heal. If IMAP/SMTP still fail from PHP-FPM, re-run email heal or check ausearch for denied."
                .into(),
        )
    } else if mode.is_empty() {
        None
    } else {
        Some(format!("SELinux mode: {mode}"))
    }
}

pub fn collect(checks: &mut Vec<super::RepairCheck>) {
    let packages_ok = email_packages_installed();
    let stack = detect_mail_stack_public();
    if !packages_ok {
        push(
            checks,
            "email.packages",
            "email",
            "Email packages",
            CheckStatus::Pass,
            "Email host package not installed (optional). Install from Plugins > Host packages when needed.",
            false,
            None,
        );
        return;
    }

    push(
        checks,
        "email.packages",
        "email",
        "Email packages",
        CheckStatus::Pass,
        "Postfix/Dovecot packages present",
        false,
        None,
    );

    let stack_ok = stack.postfix_running && stack.dovecot_running && stack.imap_listening;
    push(
        checks,
        "email.stack",
        "email",
        "Mail stack",
        if stack_ok {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        stack.detail.clone(),
        true,
        Some("email.stack"),
    );

    let mut closed = Vec::new();
    for (port, label, id) in MAIL_PORTS {
        let open = port_open(&format!("127.0.0.1:{port}"), 250);
        // POP3 and Sieve are optional depending on Dovecot modules; warn instead of fail.
        let optional = matches!(*port, 110 | 995 | 4190 | 465);
        let status = if open {
            CheckStatus::Pass
        } else if optional {
            CheckStatus::Warn
        } else {
            CheckStatus::Fail
        };
        if !open {
            closed.push(format!("{label} :{port}"));
        }
        push(
            checks,
            id,
            "email",
            &format!("{label} :{port}"),
            status,
            if open {
                format!("listening on 127.0.0.1:{port}")
            } else {
                format!("not listening on 127.0.0.1:{port}")
            },
            !optional && !open,
            if open { None } else { Some("email.stack") },
        );
    }

    if !closed.is_empty() {
        push(
            checks,
            "email.ports.summary",
            "email",
            "Email ports summary",
            CheckStatus::Warn,
            format!(
                "closed or missing: {}. Heal starts Postfix/Dovecot and opens firewall mail services.",
                closed.join(", ")
            ),
            false,
            Some("email.firewall"),
        );
    } else {
        push(
            checks,
            "email.ports.summary",
            "email",
            "Email ports summary",
            CheckStatus::Pass,
            "SMTP/IMAP/POP3/Sieve listeners present on loopback",
            false,
            None,
        );
    }

    if let Some(note) = selinux_note() {
        push(
            checks,
            "email.selinux",
            "email",
            "SELinux (mail)",
            CheckStatus::Pass,
            note,
            false,
            None,
        );
    }

    let fw_active = systemd_unit_active("firewalld");
    push(
        checks,
        "email.firewall",
        "email",
        "Firewall mail services",
        if fw_active || !crate::panel_ops_security::which_exists("firewall-cmd") {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        if fw_active {
            "firewalld active (mail services applied on Email heal)"
        } else if crate::panel_ops_security::which_exists("firewall-cmd") {
            "firewalld installed but inactive; Email heal can start a safe baseline"
        } else {
            "firewalld not installed"
        },
        false,
        if fw_active {
            None
        } else {
            Some("email.firewall")
        },
    );
}

pub fn heal_email_stack() -> HealResult {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return HealResult {
                heal_id: "email.stack".into(),
                ok: false,
                message: "requires root (sudo cpn doctor heal --id email.stack)".into(),
            };
        }
    }
    if !email_packages_installed() {
        return HealResult {
            heal_id: "email.stack".into(),
            ok: true,
            message: "Email packages not installed; skipped".into(),
        };
    }
    match start_email_stack() {
        Ok(msg) => HealResult {
            heal_id: "email.stack".into(),
            ok: true,
            message: msg,
        },
        Err(err) => HealResult {
            heal_id: "email.stack".into(),
            ok: false,
            message: err,
        },
    }
}

pub fn heal_email_firewall() -> HealResult {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return HealResult {
                heal_id: "email.firewall".into(),
                ok: false,
                message: "requires root".into(),
            };
        }
    }
    if panel_ops_firewall_heal::operator_stopped() {
        return HealResult {
            heal_id: "email.firewall".into(),
            ok: true,
            message: "skipped: operator stopped firewalld from the panel".into(),
        };
    }
    match panel_ops_firewall_heal::heal_firewalld("system repair email") {
        Some(msg) => HealResult {
            heal_id: "email.firewall".into(),
            ok: !msg.contains("failed"),
            message: msg,
        },
        None => HealResult {
            heal_id: "email.firewall".into(),
            ok: true,
            message: "firewalld heal not needed or not installed".into(),
        },
    }
}
