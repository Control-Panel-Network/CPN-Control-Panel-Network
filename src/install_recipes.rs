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
            "Ensuring Nginx",
            2,
        ),
        ServerEngine::Caddy => (
            "if command -v caddy >/dev/null 2>&1; then echo 'Caddy already installed; reusing existing binary'; else dnf install -y caddy; fi",
            "if command -v caddy >/dev/null 2>&1; then echo 'Caddy already installed; reusing existing binary'; else apt-get install -y caddy; fi",
            "Ensuring Caddy",
            5,
        ),
        ServerEngine::Openlitespeed => (
            "if test -x /usr/local/lsws/bin/openlitespeed || command -v openlitespeed >/dev/null 2>&1 || command -v lshttpd >/dev/null 2>&1; then echo 'OpenLiteSpeed already installed; reusing existing installation'; else dnf install -y openlitespeed; fi",
            "if test -x /usr/local/lsws/bin/openlitespeed || command -v openlitespeed >/dev/null 2>&1 || command -v lshttpd >/dev/null 2>&1; then echo 'OpenLiteSpeed already installed; reusing existing installation'; else apt-get install -y openlitespeed; fi",
            "Ensuring OpenLiteSpeed",
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
            vec![
                "/C",
                "echo Windows Phase A has no web-server package recipes",
            ],
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
                "Enabling Nginx",
                "installing",
                84,
            ),
        ],
        ServerEngine::Caddy => vec![
            package,
            command(
                "systemctl",
                vec!["enable", "--now", "caddy"],
                "Enabling Caddy",
                "installing",
                84,
            ),
        ],
        ServerEngine::Openlitespeed => vec![package],
    }
}

fn openlitespeed_apt_repository_text(codename: &str) -> String {
    format!(
        "deb [signed-by=/usr/share/keyrings/litespeed-archive-keyring.gpg] https://rpms.litespeedtech.com/debian/ {codename} main\n\
         #deb [signed-by=/usr/share/keyrings/litespeed-archive-keyring.gpg] https://rpms.litespeedtech.com/edge/debian/ {codename} main\n"
    )
}

/// Write the LiteSpeed repository without executing a remote shell script.
pub(crate) fn prepare_openlitespeed_repository(guest: &GuestOs) -> Result<(), String> {
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
            .map_err(|error| format!("Failed to configure the OpenLiteSpeed repository: {error}"))
        }
        PackageFamily::Apt => {
            let codename = guest.apt_codename().ok_or_else(|| {
                format!(
                    "No LiteSpeed apt suite mapping for {} (need Ubuntu 22/24/26 or Debian 12/13)",
                    guest.label
                )
            })?;
            let repository = openlitespeed_apt_repository_text(codename);
            crate::install_journal::write_file_tracked(
                "server",
                Path::new("/etc/apt/sources.list.d/lst_debian_repo.list"),
                &repository,
            )
            .map_err(|error| {
                format!("Failed to configure the OpenLiteSpeed apt repository: {error}")
            })
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
            "set -eu; \
repo=/etc/apt/sources.list.d/lst_debian_repo.list; \
disabled=${repo}.cpn-disabled; \
keyring=/usr/share/keyrings/litespeed-archive-keyring.gpg; \
tmpdir=$(mktemp -d /tmp/cpn-litespeed-key.XXXXXX); \
export GNUPGHOME=\"$tmpdir/gnupg\"; \
install -d -m 0700 \"$GNUPGHOME\"; \
restore_repo() { if test -f \"$disabled\" && ! test -f \"$repo\"; then mv \"$disabled\" \"$repo\"; fi; rm -rf \"$tmpdir\"; }; \
trap restore_repo EXIT HUP INT TERM; \
if test -f \"$repo\"; then mv -f \"$repo\" \"$disabled\"; fi; \
apt-get update -y; \
DEBIAN_FRONTEND=noninteractive apt-get install -y ca-certificates curl gnupg; \
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error --output \"$tmpdir/lst_debian_repo.gpg\" https://rpms.litespeedtech.com/debian/lst_debian_repo.gpg; \
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error --output \"$tmpdir/lst_repo.gpg\" https://rpms.litespeedtech.com/debian/lst_repo.gpg; \
legacy_fingerprints=$(gpg --batch --show-keys --with-colons \"$tmpdir/lst_debian_repo.gpg\" | awk -F: '$1 == \"fpr\" { print $10 }'); \
current_fingerprints=$(gpg --batch --show-keys --with-colons \"$tmpdir/lst_repo.gpg\" | awk -F: '$1 == \"fpr\" { print $10 }'); \
case \"$legacy_fingerprints\" in *42259994257E19EB6A91CA853F6F627083084D0E*) ;; *) echo 'LiteSpeed legacy apt key fingerprint verification failed.' >&2; exit 1;; esac; \
case \"$current_fingerprints\" in *3E892522DB44E1B063D366C5011AA62DEDA1F085*) ;; *) echo 'LiteSpeed apt key fingerprint verification failed.' >&2; exit 1;; esac; \
install -d -m 0755 /usr/share/keyrings; \
cat \"$tmpdir/lst_debian_repo.gpg\" \"$tmpdir/lst_repo.gpg\" > \"$tmpdir/litespeed-archive-keyring.gpg\"; \
install -m 0644 \"$tmpdir/litespeed-archive-keyring.gpg\" \"$keyring\"; \
if test -f \"$disabled\"; then mv -f \"$disabled\" \"$repo\"; fi; \
apt-get update -y; \
trap - EXIT HUP INT TERM; \
rm -rf \"$tmpdir\"",
        ],
        "Preparing the OpenLiteSpeed apt repository",
        "downloading",
        3,
    )
}

pub(crate) fn openlitespeed_apt_repository_healthy() -> bool {
    let source = Path::new("/etc/apt/sources.list.d/lst_debian_repo.list");
    let keyring = Path::new("/usr/share/keyrings/litespeed-archive-keyring.gpg");
    std::fs::read_to_string(source)
        .map(|contents| {
            contents.contains("signed-by=/usr/share/keyrings/litespeed-archive-keyring.gpg")
                && contents.contains("https://rpms.litespeedtech.com/debian/")
        })
        .unwrap_or(false)
        && keyring
            .metadata()
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false)
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
            .map_err(|error| format!("Failed to configure the Caddy repository: {error}"))
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
        "Preparing the Caddy apt repository",
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
    // If PHP is already present (any enabled non-EOL stream), do not fail trying to switch.
    // Fresh hosts enable the preferred Remi stream (8.5 on EL9+, Remi 8.2 on EL8).
    // Full selection + fallback is handled by `php_defaults::prepare_and_persist_php`.
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
            "Preparing PHP 8.2 (Remi on EL8)",
            "downloading",
            38,
        )),
        "php:remi-8.5" => Some(command(
            "bash",
            vec![
                "-c",
                "php -v >/dev/null 2>&1 && php -r 'exit(version_compare(PHP_VERSION,\"8.2.0\",\"<\")?1:0);' \
|| (rpm -q remi-release >/dev/null 2>&1 || dnf -y install https://rpms.remirepo.net/enterprise/remi-release-9.rpm; \
dnf -y module reset php; \
(dnf -y module enable php:remi-8.5 \
|| dnf -y module enable php:remi-8.4 \
|| dnf -y module enable php:remi-8.3 \
|| dnf -y module enable php:remi-8.2 \
|| dnf -y module enable php:8.2))",
            ],
            "Preparing PHP 8.5 (Remi; fallback 8.4/8.3/8.2)",
            "downloading",
            38,
        )),
        _ => Some(command(
            "bash",
            vec![
                "-c",
                "php -v >/dev/null 2>&1 && php -r 'exit(version_compare(PHP_VERSION,\"8.2.0\",\"<\")?1:0);' \
|| (rpm -q remi-release >/dev/null 2>&1 || true; \
dnf -y module reset php; \
(dnf -y module enable php:remi-8.5 \
|| dnf -y module enable php:remi-8.4 \
|| dnf -y module enable php:remi-8.3 \
|| dnf -y module enable php:remi-8.2 \
|| dnf -y module enable php:8.2))",
            ],
            "Preparing PHP (Remi preferred)",
            "downloading",
            38,
        )),
    }
}

pub(crate) fn php_install_command(guest: &GuestOs, label: &'static str) -> CommandSpec {
    match guest.family {
        PackageFamily::Apt => apt_install(
            PHP_PACKAGES_APT.to_vec(),
            "Installing PHP and extensions",
            40,
        ),
        PackageFamily::Dnf => {
            let mut args = vec!["install", "-y"];
            args.extend(PHP_PACKAGES_DNF.iter().copied());
            dnf(
                args,
                "Installing PHP and extensions",
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
        "Updating apt indexes",
        "downloading",
        39,
    )
}

#[cfg(test)]
#[path = "install_recipes_tests.rs"]
mod tests;
