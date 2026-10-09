//! SOGo package sources per guest OS.
//!
//! * RHEL family: Inverse nightly repository (`packages.sogo.nu`). EL8 and EL9 use their own
//!   channel. Inverse publishes no EL10 channel yet; EL10 (AlmaLinux 10, Rocky 10, RHEL 10)
//!   installs the EL9 build, which resolves cleanly on EL10 (gnustep-base and the SOPE stack
//!   come from the same repo, memcached from AppStream) and is what CPN verifies on its
//!   AlmaLinux 10 lab. Older EL releases are refused with a clear message.
//! * Debian / Ubuntu: distro `sogo` packages first (signed, stable), Inverse nightly as a
//!   fallback for suites Inverse publishes (jammy, noble, bookworm).
//!
//! Optional operator override (for example a paid Inverse subscription repository), honored on
//! every OS before any built-in source:
//! `/var/lib/cpn/sogo/repo-override.json` with `{"baseurl": "...", "gpgkey": "..."}` on dnf
//! hosts or `{"apt_line": "deb [...] https://... suite comp", "keyring_url": "..."}` on apt hosts.

use crate::os_support::{GuestOs, PackageFamily};
use serde::Deserialize;
use std::path::Path;
use std::process::{Command, Stdio};

pub const SOGO_STATE_DIR: &str = "/var/lib/cpn/sogo";
const REPO_OVERRIDE: &str = "/var/lib/cpn/sogo/repo-override.json";
const DNF_REPO_FILE: &str = "/etc/yum.repos.d/cpn-sogo.repo";
const APT_LIST_FILE: &str = "/etc/apt/sources.list.d/cpn-sogo.list";
const APT_KEYRING: &str = "/usr/share/keyrings/cpn-sogo.gpg";
const INVERSE_KEY_URL: &str =
    "https://keys.openpgp.org/vks/v1/by-fingerprint/74FFC6D72B925A34B5D356BDF8A27B36A6E2EAE9";

/// RPM package set (Inverse naming).
pub const DNF_PKGS: &[&str] = &["sogo", "sogo-tool", "sope49-gdl1-mysql", "memcached"];
/// Debian / Ubuntu package set (distro and Inverse share these names).
pub const APT_PKGS: &[&str] = &["sogo", "sope4.9-gdl1-mysql", "memcached"];

#[derive(Debug, Default, Deserialize)]
struct RepoOverride {
    #[serde(default)]
    baseurl: String,
    #[serde(default)]
    gpgkey: String,
    #[serde(default)]
    apt_line: String,
    #[serde(default)]
    keyring_url: String,
}

fn load_override() -> Option<RepoOverride> {
    let raw = std::fs::read_to_string(REPO_OVERRIDE).ok()?;
    serde_json::from_str::<RepoOverride>(&raw).ok()
}

fn run_quiet(program: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(program)
        .args(args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Could not start {program}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let tail: String = err
        .lines()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ");
    Err(format!(
        "{program} {} failed: {}",
        args.join(" "),
        tail.chars().take(400).collect::<String>()
    ))
}

/// Inverse RPM channel for an EL major: `(channel major, compat)`.
///
/// `compat` is true when the guest is newer than the newest Inverse channel (EL10+), in which
/// case the EL9 build is used. Returns `None` for EL releases older than 8.
pub fn inverse_dnf_channel(major: u32) -> Option<(u32, bool)> {
    match major {
        8 => Some((8, false)),
        9 => Some((9, false)),
        m if m >= 10 => Some((9, true)),
        _ => None,
    }
}

/// True when the operator override file provides a source for this package family.
pub fn override_present(guest: &GuestOs) -> bool {
    match (load_override(), guest.family) {
        (Some(o), PackageFamily::Dnf) => !o.baseurl.trim().is_empty(),
        (Some(o), PackageFamily::Apt) => !o.apt_line.trim().is_empty(),
        _ => false,
    }
}

/// Human-readable description of where SOGo packages come from on this guest.
pub fn source_label(guest: &GuestOs) -> String {
    if override_present(guest) {
        return "operator repository override".into();
    }
    match guest.family {
        PackageFamily::Dnf => match inverse_dnf_channel(guest.major) {
            Some((channel, false)) => format!("Inverse nightly repository (EL{channel})"),
            Some((channel, true)) => format!(
                "Inverse nightly repository (EL{channel} build, compatible with {})",
                guest.label
            ),
            None => "not available".into(),
        },
        PackageFamily::Apt => format!("{} packages (Inverse nightly fallback)", guest.label),
        PackageFamily::Windows => "not available".into(),
    }
}

/// Short operator note for EL10+ guests (shown on the card; not an error).
pub fn compat_note(guest: &GuestOs) -> Option<String> {
    if override_present(guest) {
        return None;
    }
    match (guest.family, inverse_dnf_channel(guest.major)) {
        (PackageFamily::Dnf, Some((channel, true))) => Some(format!(
            "Inverse publishes no EL{} channel yet; CPN installs the Inverse EL{channel} build on {} (same SOPE/GNUstep stack from the Inverse repo, memcached from AppStream). Override the source any time via {REPO_OVERRIDE}.",
            guest.major, guest.label
        )),
        _ => None,
    }
}

/// Clear, honest message for guests without any SOGo package source.
pub fn unsupported_message(guest: &GuestOs) -> String {
    format!(
        "SOGo packages are not available for {}. Inverse ships SOGo builds for EL8 and EL9 (used on EL10 too) and for Debian/Ubuntu. On {} you can point CPN at your own SOGo repository via {REPO_OVERRIDE} and retry Install. Tachyon (default), SnappyMail, and Roundcube remain LIVE webmail options on this host.",
        guest.label, guest.label
    )
}

fn write_dnf_repo(baseurl: &str, gpgkey: &str, major: u32) -> Result<(), String> {
    let (gpgcheck, key_line) = if gpgkey.trim().is_empty() {
        ("0", String::new())
    } else {
        ("1", format!("gpgkey={}\n", gpgkey.trim()))
    };
    let body = format!(
        "# Managed by CPN (SOGo host package). Edit {REPO_OVERRIDE} to change the source.\n[cpn-sogo]\nname=SOGo for Enterprise Linux {major} (CPN managed)\nbaseurl={baseurl}\nenabled=1\ngpgcheck={gpgcheck}\nrepo_gpgcheck=0\n{key_line}"
    );
    std::fs::write(DNF_REPO_FILE, body).map_err(|e| format!("Could not write {DNF_REPO_FILE}: {e}"))
}

fn ensure_dnf_packages(guest: &GuestOs) -> Result<String, String> {
    let overr = load_override();
    let (baseurl, gpgkey, label) = match overr {
        Some(o) if !o.baseurl.trim().is_empty() => (
            o.baseurl.trim().to_string(),
            o.gpgkey,
            "operator override repository".to_string(),
        ),
        _ => match inverse_dnf_channel(guest.major) {
            Some((channel, _compat)) => (
                format!("https://packages.sogo.nu/nightly/5/rhel/{channel}/$basearch/"),
                String::new(),
                source_label(guest),
            ),
            None => return Err(unsupported_message(guest)),
        },
    };
    if !baseurl.starts_with("https://") {
        return Err("SOGo repository baseurl must use https://".into());
    }
    write_dnf_repo(&baseurl, &gpgkey, guest.major)?;
    // EPEL provides libmemcached / liboath style dependencies on EL hosts; best effort.
    let _ = run_quiet("dnf", &["install", "-y", "-q", "epel-release"]);
    let mut args = vec!["install", "-y", "-q"];
    args.extend_from_slice(DNF_PKGS);
    run_quiet("dnf", &args).map_err(|e| {
        format!(
            "{e}. Source: {label}. Check that the host can reach packages.sogo.nu over HTTPS, then retry Install."
        )
    })?;
    Ok(format!("Installed SOGo packages from {label}."))
}

fn apt_install(pkgs: &[&str]) -> Result<(), String> {
    let mut args = vec![
        "install",
        "-y",
        "-q",
        "-o",
        "Dpkg::Options::=--force-confold",
    ];
    args.extend_from_slice(pkgs);
    run_quiet("apt-get", &args)
}

fn inverse_apt_suite(guest: &GuestOs) -> Option<(&'static str, &'static str)> {
    match (guest.id.as_str(), guest.major) {
        ("ubuntu", 24) => Some(("ubuntu", "noble")),
        ("ubuntu", 22) => Some(("ubuntu", "jammy")),
        ("debian", 12) => Some(("debian", "bookworm")),
        _ => None,
    }
}

fn fetch_keyring(url: &str) -> Result<(), String> {
    let script = format!(
        "set -o pipefail; curl --fail --silent --location --proto '=https' --max-time 60 {} | gpg --dearmor --yes -o {APT_KEYRING}",
        shell_quote(url)
    );
    run_quiet("bash", &["-c", &script])
}

fn shell_quote(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', "'\\''"))
}

fn ensure_apt_packages(guest: &GuestOs) -> Result<String, String> {
    let _ = run_quiet("apt-get", &["update", "-q"]);
    if apt_install(APT_PKGS).is_ok() {
        return Ok(format!(
            "Installed SOGo packages from the {} repositories.",
            guest.label
        ));
    }
    // Fallback: Inverse nightly (or operator override) for suites Inverse publishes.
    let overr = load_override();
    let (apt_line, keyring_url, label) = match overr {
        Some(o) if !o.apt_line.trim().is_empty() => (
            o.apt_line.trim().to_string(),
            o.keyring_url,
            "operator override repository".to_string(),
        ),
        _ => match inverse_apt_suite(guest) {
            Some((distro, suite)) => (
                format!(
                    "deb [signed-by={APT_KEYRING}] https://packages.sogo.nu/nightly/5/{distro} {suite} {suite}"
                ),
                INVERSE_KEY_URL.to_string(),
                format!("Inverse nightly repository ({suite})"),
            ),
            None => {
                return Err(format!(
                    "SOGo is not available from the {} package repositories on this host and Inverse does not publish a nightly suite for it. Enable the universe/main component, or point CPN at your own SOGo repository via {REPO_OVERRIDE} (apt_line + keyring_url), then retry Install.",
                    guest.label
                ));
            }
        },
    };
    if !keyring_url.trim().is_empty() {
        fetch_keyring(keyring_url.trim())?;
    }
    std::fs::write(APT_LIST_FILE, format!("{apt_line}\n"))
        .map_err(|e| format!("Could not write {APT_LIST_FILE}: {e}"))?;
    run_quiet("apt-get", &["update", "-q"])?;
    apt_install(APT_PKGS).map_err(|e| format!("{e}. Source: {label}."))?;
    Ok(format!("Installed SOGo packages from {label}."))
}

/// Install SOGo, the SOPE MariaDB adaptor, and memcached for the detected guest.
pub fn ensure_packages(guest: &GuestOs) -> Result<String, String> {
    match guest.family {
        PackageFamily::Windows => Err(crate::os_support::windows_linux_recipe_blocked_message(
            "SOGo host package",
        )),
        PackageFamily::Dnf => ensure_dnf_packages(guest),
        PackageFamily::Apt => ensure_apt_packages(guest),
    }
}

/// Remove SOGo packages (keeps memcached: other host packages may use it).
pub fn remove_packages(guest: &GuestOs) -> Result<(), String> {
    match guest.family {
        PackageFamily::Dnf => {
            run_quiet(
                "dnf",
                &[
                    "remove",
                    "-y",
                    "-q",
                    "sogo",
                    "sogo-tool",
                    "sope49-gdl1-mysql",
                ],
            )?;
            let _ = std::fs::remove_file(DNF_REPO_FILE);
            Ok(())
        }
        PackageFamily::Apt => {
            run_quiet(
                "apt-get",
                &[
                    "remove",
                    "-y",
                    "-q",
                    "sogo",
                    "sogo-common",
                    "sope4.9-gdl1-mysql",
                ],
            )?;
            if Path::new(APT_LIST_FILE).is_file() {
                let _ = std::fs::remove_file(APT_LIST_FILE);
                let _ = run_quiet("apt-get", &["update", "-q"]);
            }
            Ok(())
        }
        PackageFamily::Windows => Ok(()),
    }
}

/// True when the SOGo daemon binary is on disk (package installed by any source).
pub fn sogod_binary_present() -> bool {
    ["/usr/sbin/sogod", "/usr/bin/sogod", "/usr/local/sbin/sogod"]
        .iter()
        .any(|p| Path::new(p).is_file())
}

/// Static WebServerResources directory shipped by the SOGo package.
pub fn web_resources_dir() -> Option<&'static str> {
    [
        "/usr/lib64/GNUstep/SOGo/WebServerResources",
        "/usr/lib/GNUstep/SOGo/WebServerResources",
        "/usr/lib/x86_64-linux-gnu/GNUstep/SOGo/WebServerResources",
        "/usr/local/lib/GNUstep/SOGo/WebServerResources",
    ]
    .into_iter()
    .find(|p| Path::new(p).is_dir())
}

/// GNUstep SOGo bundle root (holds `<Product>.SOGo/Resources`).
pub fn gnustep_sogo_dir() -> Option<&'static str> {
    web_resources_dir().and_then(|dir| dir.strip_suffix("/WebServerResources"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os_support::detect_from_os_release;

    #[test]
    fn el10_uses_el9_compat_channel_with_note() {
        let ten = detect_from_os_release("ID=almalinux\nVERSION_ID=\"10.2\"\n").unwrap();
        assert_eq!(inverse_dnf_channel(ten.major), Some((9, true)));
        let label = source_label(&ten);
        assert!(label.contains("EL9 build"), "{label}");
        assert!(label.contains("AlmaLinux 10"), "{label}");
        let note = compat_note(&ten).expect("compat note on EL10");
        assert!(note.contains("repo-override.json"));
        assert!(
            !note
                .to_lowercase()
                .contains("not published for almalinux 10 yet")
        );
    }

    #[test]
    fn el8_and_el9_use_their_own_channels() {
        let nine = detect_from_os_release("ID=almalinux\nVERSION_ID=\"9.8\"\n").unwrap();
        assert_eq!(inverse_dnf_channel(nine.major), Some((9, false)));
        assert!(source_label(&nine).contains("EL9"));
        assert!(compat_note(&nine).is_none());
        let eight = detect_from_os_release("ID=rocky\nVERSION_ID=\"8.10\"\n").unwrap();
        assert_eq!(inverse_dnf_channel(eight.major), Some((8, false)));
        assert!(source_label(&eight).contains("EL8"));
    }

    #[test]
    fn el7_is_honestly_unsupported() {
        assert_eq!(inverse_dnf_channel(7), None);
        let seven = detect_from_os_release("ID=centos\nVERSION_ID=\"7\"\n").unwrap();
        let msg = unsupported_message(&seven);
        assert!(msg.contains("repo-override.json"));
        assert_eq!(source_label(&seven), "not available");
    }

    #[test]
    fn apt_suites_map_to_inverse_names() {
        let noble = detect_from_os_release("ID=ubuntu\nVERSION_ID=\"24.04\"\n").unwrap();
        assert_eq!(inverse_apt_suite(&noble), Some(("ubuntu", "noble")));
        let resolute = detect_from_os_release("ID=ubuntu\nVERSION_ID=\"26.04\"\n").unwrap();
        assert_eq!(inverse_apt_suite(&resolute), None);
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }
}
