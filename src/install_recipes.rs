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
        PackageFamily::Windows => command(
            "cmd",
            vec!["/C", "echo Windows Phase A has no dnf/apt recipes"],
            "Windows package install is not available",
            "failed",
            0,
        ),
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
        // Repo file is written by prepare_openlitespeed_repository (no curl|bash, issue #2).
        ServerEngine::Openlitespeed => vec![ols],
    }
}

/// Write LiteSpeed yum/apt repo directly instead of `curl | bash` (issue #2).
pub(crate) fn prepare_openlitespeed_repository(guest: &GuestOs) -> Result<(), String> {
    match guest.family {
        PackageFamily::Dnf => {
            let major = guest.major;
            let repository = format!(
                "[litespeed]\n\
                 name=LiteSpeed Tech Repository for EL{major}\n\
                 baseurl=https://rpms.litespeedtech.com/centos/{major}/$basearch/\n\
                 enabled=1\n\
                 gpgcheck=1\n\
                 gpgkey=https://rpms.litespeedtech.com/centos/RPM-GPG-KEY-litespeed\n"
            );
            std::fs::write("/etc/yum.repos.d/litespeed.repo", repository).map_err(|error| {
                format!("No se pudo configurar el repositorio de OpenLiteSpeed: {error}")
            })
        }
        PackageFamily::Apt => {
            let codename = guest.apt_codename().ok_or_else(|| {
                format!(
                    "No LiteSpeed apt suite mapping for {} (need Ubuntu 20/22/24 or Debian 11/12/13)",
                    guest.label
                )
            })?;
            let repository = format!(
                "deb http://rpms.litespeedtech.com/debian/ {codename} main\n\
                 #deb http://rpms.litespeedtech.com/edge/debian/ {codename} main\n"
            );
            std::fs::create_dir_all("/etc/apt/sources.list.d")
                .map_err(|error| format!("No se pudo crear /etc/apt/sources.list.d: {error}"))?;
            std::fs::write("/etc/apt/sources.list.d/lst_debian_repo.list", repository).map_err(
                |error| {
                    format!("No se pudo configurar el repositorio apt de OpenLiteSpeed: {error}")
                },
            )
        }
        PackageFamily::Windows => Err(crate::os_support::windows_linux_recipe_blocked_message(
            "OpenLiteSpeed repository setup",
        )),
    }
}

/// Register LiteSpeed apt keys and refresh indexes (matches vendor key paths, no curl|bash).
pub(crate) fn prepare_openlitespeed_apt_command() -> CommandSpec {
    command(
        "bash",
        vec![
            "-c",
            "apt-get update -y && apt-get install -y wget ca-certificates \
&& wget -O /etc/apt/trusted.gpg.d/lst_debian_repo.gpg http://rpms.litespeedtech.com/debian/lst_debian_repo.gpg \
&& wget -O /etc/apt/trusted.gpg.d/lst_repo.gpg http://rpms.litespeedtech.com/debian/lst_repo.gpg \
&& apt-get update -y",
        ],
        "Preparando el repositorio apt de OpenLiteSpeed",
        "downloading",
        3,
    )
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
        PackageFamily::Windows => Err(crate::os_support::windows_linux_recipe_blocked_message(
            "Caddy repository setup",
        )),
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
    "php-fpm",
    "php-mbstring",
    "php-intl",
    "php-xml",
    "php-pdo",
    "php-process",
    "php-gd",
    "php-opcache",
    "php-pecl-zip",
    "php-sqlite3",
    "unzip",
    "tar",
];

pub(crate) const PHP_PACKAGES_APT: &[&str] = &[
    "php-cli",
    "php-fpm",
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
    // If PHP is already present (any enabled stream), do not fail trying to switch streams.
    // Fresh hosts still enable a supported stream when php is missing (never EOL 8.0/8.1).
    match guest.php_module_stream()? {
        "remi-8.2" => Some(command(
            "bash",
            vec![
                "-c",
                "php -v >/dev/null 2>&1 && php -r 'exit(version_compare(PHP_VERSION,\"8.2.0\",\"<\")?1:0);' \
|| (dnf -y install https://rpms.remirepo.net/enterprise/remi-release-8.rpm \
&& dnf -y module reset php \
&& dnf -y module enable php:remi-8.2)",
            ],
            "Preparando PHP 8.2 (Remi en EL8)",
            "downloading",
            38,
        )),
        _ => Some(command(
            "bash",
            vec![
                "-c",
                "php -v >/dev/null 2>&1 && php -r 'exit(version_compare(PHP_VERSION,\"8.2.0\",\"<\")?1:0);' \
|| dnf module enable -y php:8.2",
            ],
            "Preparando PHP 8.2",
            "downloading",
            38,
        )),
    }
}

pub(crate) fn php_install_command(guest: &GuestOs, label: &'static str) -> CommandSpec {
    match guest.family {
        PackageFamily::Apt => apt_install(
            PHP_PACKAGES_APT.to_vec(),
            "Instalando PHP y sus extensiones",
            40,
        ),
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
        PackageFamily::Windows => command(
            "cmd",
            vec!["/C", "echo Windows Phase A has no PHP package recipes"],
            "Windows PHP install is not available",
            "failed",
            0,
        ),
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
