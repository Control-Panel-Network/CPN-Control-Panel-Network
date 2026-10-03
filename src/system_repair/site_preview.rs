//! System Repair check + heal for the Site preview headless browser.

use super::{CheckStatus, HealResult, push};
use crate::apps_pkg::{install_packages_dnf_or_apt, package_manager};
use crate::site_preview_capture::{capture_available, discovered_browser_path};

pub const SITE_PREVIEW_HEAL_IDS: &[&str] = &["site.preview.chromium"];

pub fn collect(checks: &mut Vec<super::RepairCheck>) {
    let available = capture_available();
    let path = discovered_browser_path();
    let (status, detail) = if available {
        (
            CheckStatus::Pass,
            match path {
                Some(p) => format!("Headless browser for Site preview: {}", p.display()),
                None => "Headless browser for Site preview is available".into(),
            },
        )
    } else {
        (
            CheckStatus::Warn,
            "No headless browser for Site preview (install chromium-headless, chromium, or google-chrome). Refresh preview will not use a remote screenshot quota.".into(),
        )
    };
    push(
        checks,
        "host.site_preview_browser",
        "web",
        "Site preview browser",
        status,
        detail,
        false,
        if available {
            None
        } else {
            Some("site.preview.chromium")
        },
    );
}

pub fn heal_chromium() -> HealResult {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            return HealResult {
                heal_id: "site.preview.chromium".into(),
                ok: false,
                message: "requires root".into(),
            };
        }
    }
    if capture_available() {
        let path = discovered_browser_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "already present".into());
        return HealResult {
            heal_id: "site.preview.chromium".into(),
            ok: true,
            message: format!("Site preview browser already installed ({path})"),
        };
    }
    let pm = package_manager().unwrap_or("dnf");
    let attempts: &[(&[&str], &[&str])] = if pm == "dnf" {
        &[
            (&["chromium-headless"], &[]),
            (&["chromium"], &[]),
            (&["chromium-browser"], &[]),
            (&["google-chrome-stable"], &[]),
        ]
    } else {
        &[
            (&[], &["chromium-headless-shell"]),
            (&[], &["chromium"]),
            (&[], &["chromium-browser"]),
            (&[], &["google-chrome-stable"]),
        ]
    };
    let mut last = String::from("No matching browser package");
    for (dnf, apt) in attempts {
        if dnf.is_empty() && apt.is_empty() {
            continue;
        }
        match install_packages_dnf_or_apt(dnf, apt) {
            Ok(()) => {
                if capture_available() {
                    let path = discovered_browser_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "installed".into());
                    return HealResult {
                        heal_id: "site.preview.chromium".into(),
                        ok: true,
                        message: format!("Installed Site preview browser ({path})"),
                    };
                }
                last = "Package installed but browser binary not found on known paths".into();
            }
            Err(err) => last = err,
        }
    }
    HealResult {
        heal_id: "site.preview.chromium".into(),
        ok: false,
        message: format!("Could not install a headless browser: {last}"),
    }
}
