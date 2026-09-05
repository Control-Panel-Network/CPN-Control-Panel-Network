//! Package-manager aware install recipes (dnf for RHEL-family, apt for Ubuntu/Debian).

use crate::model::ServerEngine;
use crate::os_support::{GuestOs, PackageFamily};
use std::path::Path;

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

/// Detect common package/manual-install locations before CPN mutates repositories.
pub(crate) fn web_server_present(server: ServerEngine) -> bool {
    match server {
        ServerEngine::Nginx => [
            "/usr/sbin/nginx",
            "/usr/bin/nginx",
            "/usr/local/sbin/nginx",
            "/usr/local/nginx/sbin/nginx",
        ]
        .iter()
        .any(|path| Path::new(path).is_file()),
        ServerEngine::Caddy => ["/usr/bin/caddy", "/usr/local/bin/caddy"]
            .iter()
            .any(|path| Path::new(path).is_file()),
        ServerEngine::Openlitespeed => [
            "/usr/local/lsws/bin/openlitespeed",
            "/usr/bin/openlitespeed",
            "/usr/sbin/openlitespeed",
            "/usr/local/lsws/bin/lshttpd",
        ]
        .iter()
        .any(|path| Path::new(path).is_file()),
    }
}

fn server_package_recipe(guest: &GuestOs, server: ServerEngine) -> CommandSpec {
    let (dnf_script, apt_script, description, progress) = match server {
        ServerEngine::Nginx => (
            "if command -v nginx >/dev/null 2>&1; then echo 'Nginx already installed; reusing existing binary'; else dnf install -y nginx; fi",
            "if command -v nginx >/dev/null 2>&1; then echo 'Nginx already installed; reusing existing binary'; else apt-get install -y nginx; fi",
            "Asegurando Nginx",
            2,
        ),
        ServerEngine::Caddy => (
            "if command -v caddy >/dev/null 2>&1; then echo 'Caddy already installed; reusing existing binary'; else dnf install -y caddy; fi",
            "if command -v caddy >/dev/null 2>&1; then echo 'Caddy already installed; reusing existing binary'; else apt-get install -y caddy; fi",
            "Asegurando Caddy",
            5,
        ),
        ServerEngine::Openlitespeed => (
            "if test -x /usr/local/lsws/bin/openlitespeed || command -v openlitespeed >/dev/null 2>&1 || command -v lshttpd >/dev/null 2>&1; then echo 'OpenLiteSpeed already installed; reusing existing installation'; else dnf install -y openlitespeed; fi",
            "if test -x /usr/local/lsws/bin/openlitespeed || command -v openlitespeed >/dev/null 2>&1 || command -v lshttpd >/dev/null 2>&1; then echo 'OpenLiteSpeed already installed; reusing existing installation'; else apt-get install -y openlitespeed; fi",
            "Asegurando OpenLiteSpeed",
            5,
        ),
    };

    match guest.family {
        PackageFamily::Dnf => command(
            "bash",
            vec!["-c", dnf_script],
            description,
            "downloading",
            progress,
        ),
        PackageFamily::Apt => command(
            "bash",
            vec!["-c", apt_script],
            description,
            "downloading",
            progress,
        ),
        PackageFamily::Windows => command(
            "cmd",
            vec!["/C", "echo Windows Phase A has no web-server package recipes"],
            "Windows web-server install is not available",
            "failed",
            0,
        ),
    }
}

pub(crate) fn server_recipes(guest: &GuestOs, server: ServerEngine) -> Vec<CommandSpec> {
    let package = server_package_recipe(guest, server);
    match server {
        ServerEngine::Nginx => vec![
            package,
            command(
                "systemctl",
                vec!["enable", "--now", "nginx"],
                "Activando Nginx",
                "installing",
                84,
            ),
        ],
        ServerEngine::Caddy => vec![
            package,
            command(
                "systemctl",
                vec!["enable", "--now", "caddy"],
                "Activando Caddy",
                "installing",
                84,
            ),
        ],
        ServerEngine::Openlitespeed => vec![package],
    }
}

/// Write the LiteSpeed repository without executing a remote shell script.
pub(crate) fn prepare_openlitespeed_repository(guest: &GuestOs) -> Result<(), String> {
    if web_server_present(ServerEngine::Openlitespeed) {
        return Ok(());
    }

    match guest.family {
        PackageFamily::Dnf => {
            let major = guest.major;
            let key = if major > 9 {
                "RPM-GPG-KEY-litespeed2025"
            } else {
                "RPM-GPG-KEY-litespeed"
            };
            let repository = format!(
                "[litespeed]\n\
                 name=LiteSpeed Tech Repository for EL{major}\n\
                 baseurl=https://rpms.litespeedtech.com/centos/{major}/$basearch/\n\
                 enabled=1\n\
                 gpgcheck=1\n\
                 gpgkey=https://rpms.litespeedtech.com/centos/{key}\n\n\
                 [litespeed-update]\n\
                 name=LiteSpeed Tech Updates for EL{major}\n\
                 baseurl=https://rpms.litespeedtech.com/centos/{major}/update/$basearch/\n\
                 enabled=1\n\
                 gpgcheck=1\n\
                 gpgkey=https://rpms.litespeedtech.com/centos/{key}\n"
            );
            crate::install_journal::write_file_tracked(
                "server",
                Path::new("/etc/yum.repos.d/litespeed.repo"),
                &repository,
            )
            .map_err(|error| format!("No se pudo configurar el repositorio de OpenLiteSpeed: {error}"))
        }
        PackageFamily::Apt => {
            let codename = guest.apt_codename().ok_or_else(|| {
                format!(
                    "No LiteSpeed apt suite mapping for {} (need Ubuntu 22/24 or Debian 12/13)",
                    guest.label
                )
            })?;
            let repository = format!(
                "deb https://rpms.litespeedtech.com/debian/ {codename} main\n\
                 #deb https://rpms.litespeedtech.com/edge/debian/ {codename} main\n"
            );
            crate::install_journal::write_file_tracked(
                "server",
                Path::new("/etc/apt/sources.list.d/lst_debian_repo.list"),
                &repository,
            )
            .map_err(|error| format!("No se pudo configurar el repositorio apt de OpenLiteSpeed: {error}"))
        }
        PackageFamily::Windows => Err(crate::os_support::windows_linux_recipe_blocked_message(
            "OpenLiteSpeed repository setup",
        )),
    }
}

/// Register LiteSpeed apt keys without overwriting operator-provided key files.
pub(crate) fn prepare_openlitespeed_apt_command() -> CommandSpec {
    command(
        "bash",
        vec![
            "-c",
            "if test -x /usr/local/lsws/bin/openlitespeed || command -v openlitespeed >/dev/null 2>&1 || command -v lshttpd >/dev/null 2>&1; then echo 'OpenLiteSpeed already installed; skipping repository bootstrap'; exit 0; fi; \
apt-get update -y && apt-get install -y wget ca-certificates \
&& (test -s /etc/apt/trusted.gpg.d/lst_debian_repo.gpg || wget -qO /etc/apt/trusted.gpg.d/lst_debian_repo.gpg https://rpms.litespeedtech.com/debian/lst_debian_repo.gpg) \
&& (test -s /etc/apt/trusted.gpg.d/lst_repo.gpg || wget -qO /etc/apt/trusted.gpg.d/lst_repo.gpg https://rpms.litespeedtech.com/debian/lst_repo.gpg) \
&& apt-get update -y",
        ],
        "Preparando el repositorio apt de OpenLiteSpeed",
        "downloading",
        3,
    )
}

pub(crate) fn prepare_caddy_repository(guest: &GuestOs) -> Result<(), String> {
    if web_server_present(ServerEngine::Caddy) {
        return Ok(());
    }

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
            crate::install_journal::write_file_tracked(
                "server",
                Path::new("/etc/yum.repos.d/caddy.repo"),
                &repository,
            )
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
            "if command -v caddy >/dev/null 2>&1; then echo 'Caddy already installed; skipping repository bootstrap'; exit 0; fi; \
apt-get update -y && apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl gnupg \
&& (test -s /usr/share/keyrings/caddy-stable-archive-keyring.gpg || (curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg)) \
&& (test -s /etc/apt/sources.list.d/caddy-stable.list || (curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null)) \
&& apt-get update -y",
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
