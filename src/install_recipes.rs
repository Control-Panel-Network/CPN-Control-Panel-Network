//! Package-manager aware install recipes (dnf for RHEL-family, apt for Ubuntu).

use crate::model::ServerEngine;
use crate::os_support::{GuestOs, PackageFamily};

#[derive(Clone, Copy)]
pub(crate) struct DnfProgress {
    pub download_start: u8,
    pub download_end: u8,
    pub install_start: u8,
    pub install_end: u8,
    pub label: &'static str,
}

pub(crate) struct CommandSpec {
    pub program: &'static str,
    pub args: Vec<&'static str>,
    pub description: &'static str,
    pub phase: &'static str,
    pub progress: u8,
    pub dnf: Option<DnfProgress>,
}

pub(crate) fn command(
    program: &'static str,
    args: Vec<&'static str>,
    description: &'static str,
    phase: &'static str,
    progress: u8,
) -> CommandSpec {
    CommandSpec {
        program,
        args,
        description,
        phase,
        progress,
        dnf: None,
    }
}

pub(crate) fn dnf(
    args: Vec<&'static str>,
    description: &'static str,
    tracking: DnfProgress,
) -> CommandSpec {
    CommandSpec {
        program: "dnf",
        args,
        description,
        phase: "downloading",
        progress: tracking.download_start,
        dnf: Some(tracking),
    }
}

pub(crate) fn apt_install(
    packages: Vec<&'static str>,
    description: &'static str,
    progress: u8,
) -> CommandSpec {
    let mut args = vec!["install", "-y"];
    args.extend(packages);
    command("apt-get", args, description, "downloading", progress)
}

pub(crate) fn pkg_install(
    guest: &GuestOs,
    packages_dnf: Vec<&'static str>,
    packages_apt: Vec<&'static str>,
    description: &'static str,
    tracking: DnfProgress,
) -> CommandSpec {
    match guest.family {
        PackageFamily::Dnf => {
            let mut args = vec!["install", "-y"];
            args.extend(packages_dnf);
            dnf(args, description, tracking)
        }
        PackageFamily::Apt => apt_install(packages_apt, description, tracking.download_start),
    }
}

pub(crate) fn server_recipes(guest: &GuestOs, server: ServerEngine) -> Vec<CommandSpec> {
    let nginx = pkg_install(
        guest,
        vec!["nginx"],
        vec!["nginx"],
        "Instalando Nginx",
        DnfProgress {
            download_start: 2,
            download_end: 48,
            install_start: 50,
            install_end: 82,
            label: "Nginx",
        },
    );
    let caddy = pkg_install(
        guest,
        vec!["caddy"],
        vec!["caddy"],
        "Instalando Caddy",
        DnfProgress {
            download_start: 5,
            download_end: 48,
            install_start: 50,
            install_end: 82,
            label: "Caddy",
        },
    );
    let ols = pkg_install(
        guest,
        vec!["openlitespeed"],
        vec!["openlitespeed"],
        "Instalando OpenLiteSpeed",
        DnfProgress {
            download_start: 5,
            download_end: 48,
            install_start: 50,
            install_end: 78,
            label: "OpenLiteSpeed",
        },
    );
    match server {
        ServerEngine::Nginx => vec![
            nginx,
            command(
                "systemctl",
                vec!["enable", "--now", "nginx"],
                "Activando Nginx",
                "installing",
                84,
            ),
        ],
        ServerEngine::Caddy => vec![
            caddy,
            command(
                "systemctl",
                vec!["enable", "--now", "caddy"],
                "Activando Caddy",
                "installing",
                84,
            ),
        ],
        ServerEngine::Openlitespeed => vec![
            command(
                "bash",
                vec!["-c", "curl -fsSL https://repo.litespeed.sh | bash"],
                "Preparando el repositorio de OpenLiteSpeed",
                "downloading",
                2,
            ),
            ols,
        ],
    }
}

pub(crate) fn prepare_caddy_repository(guest: &GuestOs) -> Result<(), String> {
    match guest.family {
        PackageFamily::Dnf => {
            let major = guest.epel_major_for_caddy()?;
            let repository = format!(
                "[copr:copr.fedorainfracloud.org:group_caddy:caddy]\n\
                 name=Caddy official COPR\n\
                 baseurl=https://download.copr.fedorainfracloud.org/results/@caddy/caddy/epel-{major}-$basearch/\n\
                 type=rpm-md\n\
                 skip_if_unavailable=False\n\
                 gpgcheck=1\n\
                 gpgkey=https://download.copr.fedorainfracloud.org/results/@caddy/caddy/pubkey.gpg\n\
                 repo_gpgcheck=0\n\
                 enabled=1\n"
            );
            std::fs::write("/etc/yum.repos.d/caddy.repo", repository)
                .map_err(|error| format!("No se pudo configurar el repositorio de Caddy: {error}"))
        }
        PackageFamily::Apt => Ok(()),
    }
}

/// One-shot apt repo bootstrap for Caddy (Cloudsmith stable).
pub(crate) fn prepare_caddy_apt_command() -> CommandSpec {
    command(
        "bash",
        vec![
            "-c",
            "apt-get update -y && apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl gnupg && curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg && curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null && apt-get update -y",
        ],
        "Preparando el repositorio apt de Caddy",
        "downloading",
        3,
    )
}

pub(crate) const PHP_PACKAGES_DNF: &[&str] = &[
    "php-cli",
    "php-mbstring",
    "php-intl",
    "php-xml",
    "php-pdo",
    "php-process",
    "php-gd",
    "php-opcache",
    "php-pecl-zip",
    "unzip",
    "tar",
];

pub(crate) const PHP_PACKAGES_APT: &[&str] = &[
    "php-cli",
    "php-mbstring",
    "php-intl",
    "php-xml",
    "php-sqlite3",
    "php-gd",
    "php-opcache",
    "php-zip",
    "unzip",
    "tar",
];

pub(crate) fn php_module_enable_command(guest: &GuestOs) -> Option<CommandSpec> {
    match guest.php_module_stream()? {
        "php:8.0" => Some(command(
            "dnf",
            vec!["module", "enable", "-y", "php:8.0"],
            "Preparando PHP 8.0",
            "downloading",
            38,
        )),
        _ => Some(command(
            "dnf",
            vec!["module", "enable", "-y", "php:8.1"],
            "Preparando PHP 8.1",
            "downloading",
            38,
        )),
    }
}

pub(crate) fn php_install_command(guest: &GuestOs, label: &'static str) -> CommandSpec {
    match guest.family {
        PackageFamily::Apt => {
            apt_install(PHP_PACKAGES_APT.to_vec(), "Instalando PHP y sus extensiones", 40)
        }
        PackageFamily::Dnf => {
            let mut args = vec!["install", "-y"];
            args.extend(PHP_PACKAGES_DNF.iter().copied());
            dnf(
                args,
                "Instalando PHP y sus extensiones",
                DnfProgress {
                    download_start: 40,
                    download_end: 58,
                    install_start: 60,
                    install_end: 76,
                    label,
                },
            )
        }
    }
}

pub(crate) fn apt_update_command() -> CommandSpec {
    command(
        "apt-get",
        vec!["update", "-y"],
        "Actualizando índices apt",
        "downloading",
        39,
    )
}
