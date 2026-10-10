//! Debian / Ubuntu package selection for the SOGo host package.
//!
//! Distro packaging of the SOPE MySQL/MariaDB adaptor differs per release:
//!
//! * Debian 12 (bookworm) and the Inverse nightly suites ship it as a separate
//!   `sope4.9-gdl1-mysql` package.
//! * Ubuntu 26.04 (resolute) folds the adaptor into `libsope1`
//!   (`.../GNUstep/GDLAdaptors-4.9/MySQL.gdladaptor`), so `sope4.9-gdl1-mysql` has no
//!   candidate and a hard `apt-get install` of it fails even though `sogo` installs fine.
//!
//! The helpers here keep `sogo` and `memcached` mandatory and treat the adaptor package as
//! optional: it is installed when `apt-cache policy` reports a candidate and skipped otherwise.
//! Everything that touches `apt-cache` / `dpkg-query` is isolated so the selection logic is
//! unit-testable without a Debian host.

use std::path::Path;
use std::process::{Command, Stdio};

/// Packages every apt host needs for SOGo (fail Install when these cannot be installed).
pub const APT_REQUIRED_PKGS: &[&str] = &["sogo", "memcached"];
/// Packages installed only when the apt cache offers a candidate (release-dependent split).
pub const APT_OPTIONAL_PKGS: &[&str] = &["sope4.9-gdl1-mysql"];
/// Packages removed on Uninstall when installed (memcached stays: other host packages use it).
pub const APT_REMOVE_PKGS: &[&str] = &["sogo", "sogo-common", "sope4.9-gdl1-mysql"];

/// Known locations of the SOPE MySQL adaptor bundle (distro multiarch, EL, legacy paths).
const MYSQL_ADAPTOR_DIRS: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu/GNUstep/GDLAdaptors-4.9/MySQL.gdladaptor",
    "/usr/lib/aarch64-linux-gnu/GNUstep/GDLAdaptors-4.9/MySQL.gdladaptor",
    "/usr/lib/GNUstep/GDLAdaptors-4.9/MySQL.gdladaptor",
    "/usr/lib64/GNUstep/GDLAdaptors-4.9/MySQL.gdladaptor",
    "/usr/lib/sope4.9/GDLAdaptors-4.9/MySQL.gdladaptor",
    "/usr/lib64/sope4.9/GDLAdaptors-4.9/MySQL.gdladaptor",
];

/// Result of resolving the apt package set for one host.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AptSelection {
    /// Packages passed to `apt-get install`.
    pub install: Vec<&'static str>,
    /// Optional packages left out because the apt cache offers no candidate.
    pub skipped: Vec<&'static str>,
}

impl AptSelection {
    /// Operator-facing note for skipped optional packages (empty when nothing was skipped).
    pub fn skipped_note(&self) -> String {
        if self.skipped.is_empty() {
            return String::new();
        }
        format!(
            "Skipped {} (no candidate in the apt cache on this release; the MySQL/MariaDB adaptor ships inside libsope1).",
            self.skipped.join(", ")
        )
    }
}

/// Parse `apt-cache policy <pkg>` output: true when a real candidate version is listed.
///
/// Unknown packages print nothing on stdout (apt writes `N: Unable to locate package` to
/// stderr); known-but-uninstallable packages print `Candidate: (none)`.
pub fn apt_policy_has_candidate(output: &str) -> bool {
    output.lines().any(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("Candidate:")
            .map(|v| {
                let v = v.trim();
                !v.is_empty() && v != "(none)"
            })
            .unwrap_or(false)
    })
}

/// Parse `dpkg-query -W -f='${Status}' <pkg>` output: true when the package is installed.
pub fn dpkg_status_installed(output: &str) -> bool {
    output
        .lines()
        .any(|line| line.split_whitespace().last() == Some("installed"))
}

/// Pick the apt install set: required packages always, optional ones only with a candidate.
pub fn select_apt_packages(has_candidate: impl Fn(&str) -> bool) -> AptSelection {
    let mut sel = AptSelection {
        install: APT_REQUIRED_PKGS.to_vec(),
        skipped: Vec::new(),
    };
    for pkg in APT_OPTIONAL_PKGS {
        if has_candidate(pkg) {
            sel.install.push(pkg);
        } else {
            sel.skipped.push(pkg);
        }
    }
    sel
}

/// Pick the apt remove set: only packages dpkg reports as installed (apt-get fails on
/// unknown names such as `sope4.9-gdl1-mysql` on Ubuntu 26.04).
pub fn select_apt_remove_packages(is_installed: impl Fn(&str) -> bool) -> Vec<&'static str> {
    APT_REMOVE_PKGS
        .iter()
        .copied()
        .filter(|pkg| is_installed(pkg))
        .collect()
}

fn capture_stdout(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

/// Live `apt-cache policy` probe (run after `apt-get update`).
pub fn apt_candidate_available(pkg: &str) -> bool {
    apt_policy_has_candidate(&capture_stdout("apt-cache", &["policy", pkg]))
}

/// Live `dpkg-query` probe.
pub fn apt_package_installed(pkg: &str) -> bool {
    dpkg_status_installed(&capture_stdout(
        "dpkg-query",
        &["-W", "-f=${Status}\n", pkg],
    ))
}

/// Resolve the install set against the live apt cache.
pub fn live_apt_selection() -> AptSelection {
    select_apt_packages(apt_candidate_available)
}

/// Resolve the remove set against the live dpkg database.
pub fn live_apt_remove_selection() -> Vec<&'static str> {
    select_apt_remove_packages(apt_package_installed)
}

/// True when the SOPE MySQL/MariaDB adaptor bundle exists on disk (any package source).
pub fn mysql_adaptor_present() -> bool {
    MYSQL_ADAPTOR_DIRS.iter().any(|p| Path::new(p).is_dir())
}

/// Post-install note when the adaptor is missing even though packages installed.
pub fn missing_adaptor_warning() -> &'static str {
    "The SOPE MySQL/MariaDB adaptor bundle (MySQL.gdladaptor) was not found after install. SOGo cannot open its MariaDB profile database without it; install the sope4.9-gdl1-mysql (or distro libsope1) package that provides GDLAdaptors-4.9/MySQL.gdladaptor and retry Start."
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESOLUTE_SOGO_POLICY: &str = "sogo:\n  Installed: (none)\n  Candidate: 5.12.4-1.2\n  Version table:\n     5.12.4-1.2 500\n        500 http://archive.ubuntu.com/ubuntu resolute/universe amd64 Packages\n";
    const BOOKWORM_ADAPTOR_POLICY: &str = "sope4.9-gdl1-mysql:\n  Installed: (none)\n  Candidate: 5.8.4-1\n  Version table:\n     5.8.4-1 500\n";

    #[test]
    fn policy_candidate_parsing() {
        assert!(apt_policy_has_candidate(RESOLUTE_SOGO_POLICY));
        assert!(apt_policy_has_candidate(BOOKWORM_ADAPTOR_POLICY));
        // Unknown package: apt-cache prints nothing on stdout.
        assert!(!apt_policy_has_candidate(""));
        assert!(!apt_policy_has_candidate(
            "N: Unable to locate package sope4.9-gdl1-mysql\n"
        ));
        // Known but uninstallable.
        assert!(!apt_policy_has_candidate(
            "sope4.9-gdl1-mysql:\n  Installed: (none)\n  Candidate: (none)\n"
        ));
    }

    #[test]
    fn dpkg_status_parsing() {
        assert!(dpkg_status_installed("install ok installed\n"));
        assert!(!dpkg_status_installed("deinstall ok config-files\n"));
        assert!(!dpkg_status_installed("unknown ok not-installed\n"));
        assert!(!dpkg_status_installed(""));
    }

    #[test]
    fn resolute_skips_adaptor_package_but_keeps_required() {
        // Ubuntu 26.04: sogo + memcached have candidates, sope4.9-gdl1-mysql does not.
        let sel = select_apt_packages(|pkg| pkg != "sope4.9-gdl1-mysql");
        assert_eq!(sel.install, vec!["sogo", "memcached"]);
        assert_eq!(sel.skipped, vec!["sope4.9-gdl1-mysql"]);
        let note = sel.skipped_note();
        assert!(note.contains("sope4.9-gdl1-mysql"), "{note}");
        assert!(note.contains("libsope1"), "{note}");
    }

    #[test]
    fn bookworm_and_inverse_install_adaptor_package() {
        // Debian 12 / Inverse nightly: every package has a candidate.
        let sel = select_apt_packages(|_| true);
        assert_eq!(sel.install, vec!["sogo", "memcached", "sope4.9-gdl1-mysql"]);
        assert!(sel.skipped.is_empty());
        assert_eq!(sel.skipped_note(), "");
    }

    #[test]
    fn required_packages_are_never_dropped_by_selection() {
        // Even when nothing has a candidate the required set is still attempted so apt-get
        // reports the real failure (missing universe component, offline mirror, ...).
        let sel = select_apt_packages(|_| false);
        assert_eq!(sel.install, APT_REQUIRED_PKGS.to_vec());
        assert_eq!(sel.skipped, APT_OPTIONAL_PKGS.to_vec());
    }

    #[test]
    fn remove_set_only_lists_installed_packages() {
        let resolute = select_apt_remove_packages(|pkg| pkg == "sogo" || pkg == "sogo-common");
        assert_eq!(resolute, vec!["sogo", "sogo-common"]);
        let bookworm = select_apt_remove_packages(|_| true);
        assert_eq!(bookworm, APT_REMOVE_PKGS.to_vec());
        let none: Vec<&str> = select_apt_remove_packages(|_| false);
        assert!(none.is_empty());
    }

    #[test]
    fn package_sets_are_consistent() {
        for pkg in APT_REQUIRED_PKGS {
            assert!(!APT_OPTIONAL_PKGS.contains(pkg), "{pkg} listed twice");
        }
        assert!(APT_REMOVE_PKGS.contains(&"sogo"));
        assert!(!APT_REMOVE_PKGS.contains(&"memcached"));
    }
}
