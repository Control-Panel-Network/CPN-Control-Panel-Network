//! Low-level uninstall steps (package, binaries, docker, stack packages).

use crate::panel_service::{self, UNIT_NAME};
use crate::sites::{self, SiteRecord};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const PACKAGE_NAME: &str = "cpn-installer";
pub(crate) const WEBMAIL_DATA: &str = "/var/lib/cpn-webmail";
pub(crate) const WEBMAIL_OPT: &str = "/opt/cpn-webmail";
pub(crate) const ETC_CPN: &str = "/etc/cpn";
pub(crate) const PROFILE_MOTD: &str = "/etc/profile.d/cpn-motd.sh";
pub(crate) const LIB_CPN: &str = "/usr/lib/cpn";
const UNIT_VENDOR: &str = "/usr/lib/systemd/system/cpn-installer.service";
const UNIT_ETC: &str = "/etc/systemd/system/cpn-installer.service";
const UNIT_DROPIN_DIR: &str = "/etc/systemd/system/cpn-installer.service.d";

pub(crate) fn log_step(msg: &str) {
    let _ = writeln!(std::io::stderr(), "[cpn-uninstall] {msg}");
}

fn run_cmd(bin: &str, args: &[&str], dry_run: bool) -> Result<(), String> {
    let rendered = format!("{bin} {}", args.join(" "));
    if dry_run {
        log_step(&format!("dry-run: {rendered}"));
        return Ok(());
    }
    log_step(&rendered);
    let status = Command::new(bin)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run `{rendered}`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{rendered}` exited with {}",
            status.code().unwrap_or(-1)
        ))
    }
}

fn try_cmd(bin: &str, args: &[&str], dry_run: bool) {
    if let Err(error) = run_cmd(bin, args, dry_run) {
        log_step(&format!("warning: {error}"));
    }
}

fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

pub(crate) fn remove_path(path: &Path, dry_run: bool) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if dry_run {
        log_step(&format!("dry-run: remove {}", path.display()));
        return Ok(());
    }
    log_step(&format!("remove {}", path.display()));
    if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|e| format!("failed to remove {}: {e}", path.display()))?;
    } else {
        fs::remove_file(path).map_err(|e| format!("failed to remove {}: {e}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn site_home_guess(site: &SiteRecord) -> PathBuf {
    let doc = PathBuf::from(&site.docroot);
    if doc
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("public_html"))
        && let Some(parent) = doc.parent()
    {
        return parent.to_path_buf();
    }
    sites::hosting_home_root().join(&site.domain)
}

pub(crate) fn stop_and_disable_unit(dry_run: bool) {
    if cfg!(windows) {
        return;
    }
    if dry_run {
        log_step(&format!("dry-run: systemctl stop/disable {UNIT_NAME}"));
        return;
    }
    let _ = panel_service::stop_orphan_panel_listeners();
    try_cmd("systemctl", &["stop", UNIT_NAME], false);
    try_cmd("systemctl", &["disable", UNIT_NAME], false);
    try_cmd("systemctl", &["reset-failed", UNIT_NAME], false);
}

fn docker_bin() -> Option<&'static str> {
    ["docker", "podman"].into_iter().find(|&candidate| {
        Command::new(candidate)
            .arg("--version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

fn cpn_compose_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let root = data_dir.join("docker");
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && (p.join("compose.yml").is_file()
                    || p.join("docker-compose.yml").is_file()
                    || p.join("compose.yaml").is_file())
        })
        .collect()
}

pub(crate) fn tear_down_cpn_docker(data_dir: &Path, purge_volumes: bool, dry_run: bool) {
    let Some(bin) = docker_bin() else {
        log_step("docker/podman not found; skipping CPN Docker teardown");
        return;
    };
    for dir in cpn_compose_dirs(data_dir) {
        let compose_file = ["compose.yml", "docker-compose.yml", "compose.yaml"]
            .into_iter()
            .map(|n| dir.join(n))
            .find(|p| p.is_file());
        let Some(file) = compose_file else {
            continue;
        };
        let file_s = file.to_string_lossy().into_owned();
        let dir_s = dir.to_string_lossy().into_owned();
        let mut args: Vec<&str> = vec![
            "compose",
            "-f",
            &file_s,
            "--project-directory",
            &dir_s,
            "down",
            "--remove-orphans",
        ];
        if purge_volumes {
            args.push("-v");
        }
        try_cmd(bin, &args, dry_run);
    }
    let filter_out = Command::new(bin)
        .args(["ps", "-aq", "--filter", "label=com.cpn.managed=1"])
        .output();
    if let Ok(out) = filter_out {
        let ids: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        for id in ids {
            try_cmd(bin, &["rm", "-f", &id], dry_run);
        }
    }
}

fn package_installed_rpm() -> bool {
    Command::new("rpm")
        .args(["-q", PACKAGE_NAME])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn package_installed_deb() -> bool {
    let output = Command::new("dpkg-query")
        .args(["-W", "-f=${Status}", PACKAGE_NAME])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let status = String::from_utf8_lossy(&out.stdout);
            status.contains("install ok installed")
        }
        _ => false,
    }
}

pub(crate) fn remove_package(dry_run: bool) -> Result<(), String> {
    if package_installed_rpm() {
        if path_exists("/usr/bin/dnf") {
            return run_cmd("dnf", &["remove", "-y", PACKAGE_NAME], dry_run);
        }
        if path_exists("/usr/bin/yum") {
            return run_cmd("yum", &["remove", "-y", PACKAGE_NAME], dry_run);
        }
        return run_cmd("rpm", &["-e", PACKAGE_NAME], dry_run);
    }
    if package_installed_deb() {
        if path_exists("/usr/bin/apt-get") {
            return run_cmd(
                "apt-get",
                &["remove", "-y", "--purge", PACKAGE_NAME],
                dry_run,
            );
        }
        return run_cmd("dpkg", &["--purge", PACKAGE_NAME], dry_run);
    }
    log_step(&format!(
        "package `{PACKAGE_NAME}` not installed via rpm/dpkg; continuing with file cleanup"
    ));
    Ok(())
}

pub(crate) fn remove_binaries_and_helpers(dry_run: bool) -> Result<(), String> {
    let paths = [
        "/usr/bin/cpn",
        "/usr/bin/cpn-installer",
        "/usr/local/bin/cpn",
        "/usr/local/bin/cpn-installer",
        "/usr/bin/cpn-installer.bak",
        "/usr/bin/cpn-installer.old",
        PROFILE_MOTD,
        UNIT_VENDOR,
        UNIT_ETC,
    ];
    for path in paths {
        remove_path(Path::new(path), dry_run)?;
    }
    remove_path(Path::new(LIB_CPN), dry_run)?;
    remove_path(Path::new(UNIT_DROPIN_DIR), dry_run)?;
    if let Ok(entries) = fs::read_dir("/usr/local/bin") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == "cpn"
                || name == "cpn-installer"
                || name.starts_with("cpn.bak.")
                || name.starts_with("cpn-installer.bak.")
            {
                remove_path(&entry.path(), dry_run)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn purge_site_homes(sites: &[SiteRecord], dry_run: bool) -> Result<(), String> {
    let root = sites::hosting_home_root();
    for site in sites {
        let home = site_home_guess(site);
        if !home.starts_with(&root) || home == root {
            log_step(&format!(
                "skip site home outside hosting root: {}",
                home.display()
            ));
            continue;
        }
        remove_path(&home, dry_run)?;
    }
    Ok(())
}

pub(crate) fn purge_stack_packages(dry_run: bool) {
    let candidates = [
        "openlitespeed",
        "mariadb-server",
        "MariaDB-server",
        "mariadb",
        "phpMyAdmin",
        "phpmyadmin",
    ];
    if path_exists("/usr/bin/dnf") {
        for pkg in candidates {
            let installed = Command::new("rpm")
                .args(["-q", pkg])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if installed {
                try_cmd("dnf", &["remove", "-y", pkg], dry_run);
            }
        }
        return;
    }
    if path_exists("/usr/bin/apt-get") {
        for pkg in ["openlitespeed", "mariadb-server", "phpmyadmin"] {
            try_cmd("apt-get", &["remove", "-y", pkg], dry_run);
        }
    }
}

pub(crate) fn daemon_reload(dry_run: bool) {
    if cfg!(unix) {
        try_cmd("systemctl", &["daemon-reload"], dry_run);
    }
}
