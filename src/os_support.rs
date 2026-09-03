//! Guest OS detection and CyberPanel-aligned allowlists for the Linux installer.
//!
//! CPN installs into a Linux guest. Windows Server, VirtualBox, and Hyper-V are
//! host/hypervisor targets for those guests, not native panel install targets.

use std::collections::HashMap;

/// Package family used by install recipes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageFamily {
    Dnf,
    Apt,
}

/// How far CPN claims support for a detected guest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportStatus {
    /// Primary install path is implemented (dnf or apt recipes).
    Supported,
    /// Detected and allowed; recipes share the family path but need more lab proof.
    Partial,
    /// Community / third-party: clear "not yet" error (do not hard-fail as unknown).
    NotYet,
    /// Outside the CyberPanel-aligned matrix.
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

    pub fn php_module_stream(&self) -> Option<&'static str> {
        match (self.uses_dnf(), self.major) {
            (true, 8) => Some("php:8.0"),
            (true, 9) => Some("php:8.1"),
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
        "almalinux" if (8..=10).contains(&major) => {
            let status = if major == 8 {
                SupportStatus::Partial
            } else {
                SupportStatus::Supported
            };
            (PackageFamily::Dnf, status, label)
        }
        "rocky" if matches!(major, 8 | 9) => {
            let status = if major == 9 {
                SupportStatus::Supported
            } else {
                SupportStatus::Partial
            };
            (PackageFamily::Dnf, status, label)
        }
        "rhel" if matches!(major, 8 | 9) => (PackageFamily::Dnf, SupportStatus::Partial, label),
        "cloudlinux" if major == 8 => (PackageFamily::Dnf, SupportStatus::Partial, label),
        "centos" if major == 9 => (PackageFamily::Dnf, SupportStatus::Partial, label),
        "ubuntu" if matches!(major, 20 | 22 | 24) => {
            let status = if matches!(major, 22 | 24) {
                SupportStatus::Supported
            } else {
                SupportStatus::Partial
            };
            (PackageFamily::Apt, status, label)
        }
        "debian" | "openeuler" => (PackageFamily::Apt, SupportStatus::NotYet, label),
        id if id.contains("rhel")
            || id.contains("centos")
            || id.contains("rocky")
            || id.contains("alma") =>
        {
            (PackageFamily::Dnf, SupportStatus::NotYet, label)
        }
        _ => {
            let family = if matches!(id, "debian" | "linuxmint" | "pop" | "elementary") {
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

pub fn detect_guest_os() -> Result<GuestOs, String> {
    let release = std::fs::read_to_string("/etc/os-release")
        .map_err(|_| "No se pudo identificar el sistema operativo (/etc/os-release)".to_string())?;
    detect_from_os_release(&release)
}

fn supported_list_message() -> &'static str {
    "Sistemas invitados admitidos (alineados con CyberPanel): \
     Ubuntu 24.04/22.04/20.04; AlmaLinux 10/9/8; Rocky Linux 9/8; \
     RHEL 9/8; CloudLinux 8; CentOS Stream 9. \
     Best-effort (aún no): Debian, openEuler, otros derivados RHEL. \
     Hosts/hipervisores (no instalador nativo Windows): VirtualBox, Hyper-V, Windows Server. \
     Ver to-do/OS-SUPPORT-MATRIX.md"
}

/// Refuse install with a clear message unless the guest is Supported or Partial.
pub fn require_installable_guest() -> Result<GuestOs, String> {
    let guest = detect_guest_os()?;
    if guest.is_installable() {
        return Ok(guest);
    }
    match guest.support {
        SupportStatus::NotYet => Err(format!(
            "{} está en la matriz CyberPanel como best-effort, pero CPN aún no tiene \
             recetas listas para este sistema. {}",
            guest.label,
            supported_list_message()
        )),
        _ => Err(format!(
            "{} no está en la matriz de invitados de CPN. {}",
            guest.label,
            supported_list_message()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{PackageFamily, SupportStatus, detect_from_os_release};

    #[test]
    fn detects_almalinux_9_and_10() {
        let nine = detect_from_os_release(
            "ID=almalinux\nVERSION_ID=\"9.8\"\nPRETTY_NAME=\"AlmaLinux 9.8\"\n",
        )
        .unwrap();
        assert_eq!(nine.major, 9);
        assert_eq!(nine.support, SupportStatus::Supported);
        assert_eq!(nine.family, PackageFamily::Dnf);
        assert_eq!(nine.php_module_stream(), Some("php:8.1"));

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
        assert_eq!(eight.php_module_stream(), Some("php:8.0"));
    }

    #[test]
    fn detects_rocky_9_and_ubuntu() {
        let rocky = detect_from_os_release("ID=rocky\nVERSION_ID=\"9.5\"\n").unwrap();
        assert_eq!(rocky.support, SupportStatus::Supported);
        assert!(rocky.uses_dnf());

        let jammy = detect_from_os_release("ID=ubuntu\nVERSION_ID=\"22.04\"\n").unwrap();
        assert_eq!(jammy.major, 22);
        assert_eq!(jammy.support, SupportStatus::Supported);
        assert!(jammy.uses_apt());

        let noble = detect_from_os_release("ID=ubuntu\nVERSION_ID=\"24.04\"\n").unwrap();
        assert_eq!(noble.support, SupportStatus::Supported);
    }

    #[test]
    fn detects_rhel_cloudlinux_centos_and_not_yet() {
        let rhel = detect_from_os_release("ID=rhel\nVERSION_ID=\"9.4\"\n").unwrap();
        assert_eq!(rhel.support, SupportStatus::Partial);

        let cl = detect_from_os_release("ID=cloudlinux\nVERSION_ID=\"8.9\"\n").unwrap();
        assert_eq!(cl.support, SupportStatus::Partial);

        let centos = detect_from_os_release("ID=centos\nVERSION_ID=\"9\"\n").unwrap();
        assert_eq!(centos.support, SupportStatus::Partial);

        let debian = detect_from_os_release("ID=debian\nVERSION_ID=\"12\"\n").unwrap();
        assert_eq!(debian.support, SupportStatus::NotYet);
        assert!(!debian.is_installable());
    }

    #[test]
    fn rejects_unknown_distro() {
        let weird = detect_from_os_release("ID=somethingos\nVERSION_ID=\"1.0\"\n").unwrap();
        assert_eq!(weird.support, SupportStatus::Unsupported);
    }
}
