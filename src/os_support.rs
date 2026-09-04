//! Guest OS detection and allowlists for the CPN installer.
//!
//! Linux guests use dnf/apt recipes. Windows Server 2016+ is Phase A
//! (installer UI + account bootstrap). Windows Server 2012/2012 R2 is not
//! supported for modern Rust runtimes. Hypervisors remain host-only.

use std::collections::HashMap;

/// Package family used by install recipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageFamily {
    Dnf,
    Apt,
    /// Windows Server Phase A: no dnf/apt; panel UI and data dir only.
    Windows,
}

/// How far CPN claims support for a detected guest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportStatus {
    /// Primary install path is implemented (dnf or apt recipes).
    Supported,
    /// Detected and allowed; recipes share the family path but need more lab proof,
    /// or Windows Phase A (UI + bootstrap without Linux package parity).
    Partial,
    /// Known target outside the installable allowlist (installer refuses helpfully).
    NotYet,
    /// Outside the CPN matrix.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestOs {
    pub id: String,
    pub pretty_name: String,
    pub major: u32,
    pub version_id: String,
    pub family: PackageFamily,
    pub support: SupportStatus,
    pub label: String,
}

impl GuestOs {
    pub fn is_installable(&self) -> bool {
        matches!(
            self.support,
            SupportStatus::Supported | SupportStatus::Partial
        )
    }

    pub fn uses_dnf(&self) -> bool {
        self.family == PackageFamily::Dnf
    }

    pub fn uses_apt(&self) -> bool {
        self.family == PackageFamily::Apt
    }

    pub fn is_windows(&self) -> bool {
        self.family == PackageFamily::Windows
    }

    /// Debian/Ubuntu suite name for LiteSpeed apt sources (when applicable).
    pub fn apt_codename(&self) -> Option<&'static str> {
        match (self.id.as_str(), self.major) {
            ("ubuntu", 24) => Some("noble"),
            ("ubuntu", 22) => Some("jammy"),
            ("ubuntu", 20) => Some("focal"),
            ("debian", 13) => Some("trixie"),
            ("debian", 12) => Some("bookworm"),
            ("debian", 11) => Some("bullseye"),
            _ => None,
        }
    }

    pub fn php_module_stream(&self) -> Option<&'static str> {
        match (self.uses_dnf(), self.major) {
            // Never enable AppStream PHP 8.0/8.1 (EOL). EL8 uses Remi 8.2 (issue #4).
            (true, 8) => Some("remi-8.2"),
            (true, 9) => Some("php:8.2"),
            _ => None,
        }
    }

    /// COPR / EPEL major used by Caddy on RHEL-family guests.
    pub fn epel_major_for_caddy(&self) -> Result<u32, String> {
        if !self.uses_dnf() {
            return Err("Caddy COPR epel path only applies to RHEL-family guests".into());
        }
        if !(8..=10).contains(&self.major) {
            return Err(format!("No Caddy COPR mapping for EL major {}", self.major));
        }
        Ok(self.major)
    }
}

fn parse_os_release(contents: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        map.insert(key.to_string(), value.to_string());
    }
    map
}

fn major_from_version_id(version_id: &str) -> Result<u32, String> {
    version_id
        .split('.')
        .next()
        .and_then(|part| part.parse().ok())
        .ok_or_else(|| format!("VERSION_ID no válida: {version_id}"))
}

fn classify(id: &str, major: u32, version_id: &str) -> (PackageFamily, SupportStatus, String) {
    let label = match id {
        "almalinux" => format!("AlmaLinux {major}"),
        "rocky" => format!("Rocky Linux {major}"),
        "rhel" => format!("RHEL {major}"),
        "cloudlinux" => format!("CloudLinux {major}"),
        "centos" => format!("CentOS Stream {major}"),
        "ubuntu" => format!("Ubuntu {version_id}"),
        "debian" => format!("Debian {major}"),
        "openeuler" => format!("openEuler {major}"),
        other => format!("{other} {version_id}"),
    };

    match id {
        // Status tiers track verification evidence (see to-do/OS-SUPPORT-MATRIX.md).
        // Supported rows must have recipe path + smoke evidence (lab VM and/or docker matrix).
        "almalinux" if matches!(major, 9 | 10) => {
            (PackageFamily::Dnf, SupportStatus::Supported, label)
        }
        "almalinux" if major == 8 => (PackageFamily::Dnf, SupportStatus::Partial, label),
        "rocky" if major == 9 => (PackageFamily::Dnf, SupportStatus::Supported, label),
        "rocky" if major == 8 => (PackageFamily::Dnf, SupportStatus::Partial, label),
        "rhel" if matches!(major, 8 | 9) => (PackageFamily::Dnf, SupportStatus::Partial, label),
        "cloudlinux" if major == 8 => (PackageFamily::Dnf, SupportStatus::Partial, label),
        "centos" if major == 9 => (PackageFamily::Dnf, SupportStatus::Partial, label),
        "ubuntu" if matches!(major, 20 | 22 | 24) => {
            // 20.04 remains Partial: older PHP/repos and thinner lab evidence.
            let status = if matches!(major, 22 | 24) {
                SupportStatus::Supported
            } else {
                SupportStatus::Partial
            };
            (PackageFamily::Apt, status, label)
        }
        "debian" if (11..=13).contains(&major) => {
            (PackageFamily::Apt, SupportStatus::Partial, label)
        }
        // openEuler uses dnf; package names may diverge, so Partial until lab proof.
        "openeuler" if (20..=24).contains(&major) => {
            (PackageFamily::Dnf, SupportStatus::Partial, label)
        }
        "debian" => (PackageFamily::Apt, SupportStatus::NotYet, label),
        "openeuler" => (PackageFamily::Dnf, SupportStatus::NotYet, label),
        id if id.contains("rhel")
            || id.contains("centos")
            || id.contains("rocky")
            || id.contains("alma") =>
        {
            (PackageFamily::Dnf, SupportStatus::NotYet, label)
        }
        _ => {
            let family = if matches!(id, "linuxmint" | "pop" | "elementary") {
                PackageFamily::Apt
            } else {
                PackageFamily::Dnf
            };
            (family, SupportStatus::Unsupported, label)
        }
    }
}

/// Parse `/etc/os-release` text into a guest profile (unit-test friendly).
pub fn detect_from_os_release(contents: &str) -> Result<GuestOs, String> {
    let map = parse_os_release(contents);
    let id = map
        .get("ID")
        .cloned()
        .ok_or_else(|| "No se pudo leer ID en /etc/os-release".to_string())?
        .to_lowercase();
    let version_id = map
        .get("VERSION_ID")
        .cloned()
        .ok_or_else(|| "No se pudo leer VERSION_ID en /etc/os-release".to_string())?;
    let pretty_name = map
        .get("PRETTY_NAME")
        .cloned()
        .unwrap_or_else(|| format!("{id} {version_id}"));
    let major = major_from_version_id(&version_id)?;
    let (family, support, label) = classify(&id, major, &version_id);
    Ok(GuestOs {
        id,
        pretty_name,
        major,
        version_id,
        family,
        support,
        label,
    })
}

/// Classify a Windows host from ProductName + CurrentBuild (unit-test friendly).
///
/// Build thresholds (approx): 2012=9200, 2012 R2=9600, 2016=14393, 2019=17763, 2022=20348.
pub fn detect_from_windows_info(product_name: &str, current_build: u32) -> GuestOs {
    let lower = product_name.to_lowercase();
    let is_server = lower.contains("server");
    let (label, major, version_id) = if !is_server {
        (
            format!("Windows (build {current_build})"),
            current_build / 1000,
            current_build.to_string(),
        )
    } else if current_build < 9200 {
        (
            format!("Windows Server (build {current_build})"),
            0,
            current_build.to_string(),
        )
    } else if current_build < 9600 {
        ("Windows Server 2012".into(), 2012, "2012".into())
    } else if current_build < 14393 {
        ("Windows Server 2012 R2".into(), 2012, "2012 R2".into())
    } else if current_build < 17763 {
        ("Windows Server 2016".into(), 2016, "2016".into())
    } else if current_build < 20348 {
        ("Windows Server 2019".into(), 2019, "2019".into())
    } else if current_build < 26000 {
        ("Windows Server 2022".into(), 2022, "2022".into())
    } else {
        (
            format!("Windows Server (build {current_build})"),
            2025,
            current_build.to_string(),
        )
    };

    let support = if !is_server {
        SupportStatus::Unsupported
    } else if current_build < 14393 {
        SupportStatus::NotYet
    } else {
        SupportStatus::Partial
    };

    GuestOs {
        id: "windows".into(),
        pretty_name: if product_name.trim().is_empty() {
            label.clone()
        } else {
            product_name.trim().to_string()
        },
        major,
        version_id,
        family: PackageFamily::Windows,
        support,
        label,
    }
}

#[cfg(windows)]
fn read_windows_registry_value(value_name: &str) -> Option<String> {
    use std::process::Command;
    let output = Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "/v",
            value_name,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        if !line.contains(value_name) {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            return Some(parts[parts.len() - 1].to_string());
        }
    }
    None
}

#[cfg(windows)]
fn detect_windows_guest() -> Result<GuestOs, String> {
    let product =
        read_windows_registry_value("ProductName").unwrap_or_else(|| "Windows Server".to_string());
    let build_raw = read_windows_registry_value("CurrentBuild")
        .or_else(|| read_windows_registry_value("CurrentBuildNumber"))
        .ok_or_else(|| "Could not read Windows CurrentBuild from registry".to_string())?;
    let build: u32 = build_raw
        .parse()
        .map_err(|_| format!("Invalid Windows CurrentBuild value: {build_raw}"))?;
    Ok(detect_from_windows_info(&product, build))
}

pub fn detect_guest_os() -> Result<GuestOs, String> {
    #[cfg(windows)]
    {
        detect_windows_guest()
    }
    #[cfg(not(windows))]
    {
        let release = std::fs::read_to_string("/etc/os-release").map_err(|_| {
            "No se pudo identificar el sistema operativo (/etc/os-release)".to_string()
        })?;
        detect_from_os_release(&release)
    }
}

fn supported_list_message() -> &'static str {
    "Supported Linux guests: Ubuntu 24.04/22.04/20.04; AlmaLinux 10/9/8; Rocky Linux 9/8; \
     RHEL 9/8; CloudLinux 8; CentOS Stream 9; Debian 11/12/13 (partial); \
     openEuler 20-24 (partial). \
     Windows Server 2016 and later: Phase A (installer UI + account bootstrap; partial). \
     Windows Server 2012 / 2012 R2: not supported (modern Rust / MSVC toolchain). \
     Hosts/hypervisors (not install targets): VirtualBox, Hyper-V. \
     See to-do/OS-SUPPORT-MATRIX.md and to-do/WINDOWS-SERVER-INSTALL.md"
}

/// Refuse install with a clear message unless the guest is Supported or Partial.
pub fn require_installable_guest() -> Result<GuestOs, String> {
    let guest = detect_guest_os()?;
    if guest.is_installable() {
        return Ok(guest);
    }
    match guest.support {
        SupportStatus::NotYet => Err(format!(
            "{} is recognized but not yet an installable CPN target. {}",
            guest.label,
            supported_list_message()
        )),
        _ => Err(format!(
            "{} is not in the CPN guest matrix. {}",
            guest.label,
            supported_list_message()
        )),
    }
}

/// Clear message when Linux dnf/apt recipes are requested on Windows.
pub fn windows_linux_recipe_blocked_message(feature: &str) -> String {
    format!(
        "{feature} uses Linux package recipes (dnf/apt) and is not available on Windows Server. \
         Phase A supports the installer UI, Windows service, and account bootstrap under \
         %ProgramData%\\CPN. IIS / reverse-proxy helpers are Phase B. \
         See to-do/WINDOWS-SERVER-INSTALL.md"
    )
}

#[cfg(test)]
mod tests {
    use super::{PackageFamily, SupportStatus, detect_from_os_release, detect_from_windows_info};

    #[test]
    fn detects_almalinux_9_and_10() {
        let nine = detect_from_os_release(
            "ID=almalinux\nVERSION_ID=\"9.8\"\nPRETTY_NAME=\"AlmaLinux 9.8\"\n",
        )
        .unwrap();
        assert_eq!(nine.major, 9);
        assert_eq!(nine.support, SupportStatus::Supported);
        assert_eq!(nine.family, PackageFamily::Dnf);
        assert_eq!(nine.php_module_stream(), Some("php:8.2"));

        let ten = detect_from_os_release(
            "ID=\"almalinux\"\nVERSION_ID=\"10.0\"\nPRETTY_NAME=\"AlmaLinux 10.0\"\n",
        )
        .unwrap();
        assert_eq!(ten.major, 10);
        assert_eq!(ten.php_module_stream(), None);
        assert_eq!(ten.epel_major_for_caddy().unwrap(), 10);
    }

    #[test]
    fn detects_almalinux_8_partial() {
        let eight = detect_from_os_release("ID=almalinux\nVERSION_ID=\"8.10\"\n").unwrap();
        assert_eq!(eight.support, SupportStatus::Partial);
        assert_eq!(eight.php_module_stream(), Some("remi-8.2"));
    }

    #[test]
    fn detects_rocky_9_and_ubuntu() {
        let rocky = detect_from_os_release("ID=rocky\nVERSION_ID=\"9.5\"\n").unwrap();
        assert_eq!(rocky.support, SupportStatus::Supported);
        assert!(rocky.uses_dnf());

        let rocky8 = detect_from_os_release("ID=rocky\nVERSION_ID=\"8.10\"\n").unwrap();
        assert_eq!(rocky8.support, SupportStatus::Partial);

        let jammy = detect_from_os_release("ID=ubuntu\nVERSION_ID=\"22.04\"\n").unwrap();
        assert_eq!(jammy.major, 22);
        assert_eq!(jammy.support, SupportStatus::Supported);
        assert!(jammy.uses_apt());
        assert_eq!(jammy.apt_codename(), Some("jammy"));

        let noble = detect_from_os_release("ID=ubuntu\nVERSION_ID=\"24.04\"\n").unwrap();
        assert_eq!(noble.support, SupportStatus::Supported);
        assert_eq!(noble.apt_codename(), Some("noble"));
    }

    #[test]
    fn detects_rhel_cloudlinux_centos_debian_openeuler() {
        let rhel = detect_from_os_release("ID=rhel\nVERSION_ID=\"9.4\"\n").unwrap();
        assert_eq!(rhel.support, SupportStatus::Partial);

        let cl = detect_from_os_release("ID=cloudlinux\nVERSION_ID=\"8.9\"\n").unwrap();
        assert_eq!(cl.support, SupportStatus::Partial);

        let centos = detect_from_os_release("ID=centos\nVERSION_ID=\"9\"\n").unwrap();
        assert_eq!(centos.support, SupportStatus::Partial);

        let debian = detect_from_os_release("ID=debian\nVERSION_ID=\"12\"\n").unwrap();
        assert_eq!(debian.support, SupportStatus::Partial);
        assert!(debian.is_installable());
        assert_eq!(debian.apt_codename(), Some("bookworm"));

        let euler = detect_from_os_release("ID=openeuler\nVERSION_ID=\"22.03\"\n").unwrap();
        assert_eq!(euler.support, SupportStatus::Partial);
        assert!(euler.uses_dnf());
        assert!(euler.is_installable());
    }

    #[test]
    fn rejects_unknown_distro() {
        let weird = detect_from_os_release("ID=somethingos\nVERSION_ID=\"1.0\"\n").unwrap();
        assert_eq!(weird.support, SupportStatus::Unsupported);
    }

    #[test]
    fn windows_2016_plus_is_partial_phase_a() {
        let ws2016 = detect_from_windows_info("Windows Server 2016 Standard", 14393);
        assert!(ws2016.is_windows());
        assert!(ws2016.is_installable());
        assert_eq!(ws2016.support, SupportStatus::Partial);
        assert_eq!(ws2016.label, "Windows Server 2016");
        assert!(!ws2016.uses_dnf());
        assert!(!ws2016.uses_apt());

        let ws2022 = detect_from_windows_info("Windows Server 2022 Datacenter", 20348);
        assert_eq!(ws2022.support, SupportStatus::Partial);
        assert_eq!(ws2022.label, "Windows Server 2022");
    }

    #[test]
    fn windows_2012_is_not_yet() {
        let ws2012 = detect_from_windows_info("Windows Server 2012 Standard", 9200);
        assert_eq!(ws2012.support, SupportStatus::NotYet);
        assert!(!ws2012.is_installable());
        assert_eq!(ws2012.label, "Windows Server 2012");

        let r2 = detect_from_windows_info("Windows Server 2012 R2 Standard", 9600);
        assert_eq!(r2.support, SupportStatus::NotYet);
        assert_eq!(r2.label, "Windows Server 2012 R2");
    }

    #[test]
    fn windows_client_unsupported() {
        let client = detect_from_windows_info("Windows 10 Pro", 19045);
        assert_eq!(client.support, SupportStatus::Unsupported);
        assert!(!client.is_installable());
    }
}
