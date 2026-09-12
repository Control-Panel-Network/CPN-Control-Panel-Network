//! Interactive SSH/CLI installer front-end (alternative to the web UI).

use crate::account::{default_password_policy, setup_account};
use crate::cli_common::{is_root, print_generated, require_root_for_mutation};
use crate::installer::{AppState, InstallLogDetail};
use crate::listen_port::{self, DEFAULT_PORT};
use crate::model::{
    DatabaseEngine, InstallerEvent, InstallerStatus, MailSystem, PasswordPolicy, ServerEngine,
};
use crate::panel_network::save_panel_hostname;
use crate::panel_public_url::save_panel_public_url;
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::broadcast;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How the installer presents itself for a fresh/guided install session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFrontend {
    Web,
    Cli,
}

/// Resolve web vs SSH/CLI from flags, or prompt when stdin is a TTY.
pub fn resolve_install_frontend(args: &[String]) -> Result<InstallFrontend, String> {
    let wants_cli = args
        .iter()
        .any(|arg| arg == "--cli" || arg == "--ssh" || arg == "--ssh-cli");
    let wants_web = args.iter().any(|arg| arg == "--web" || arg == "--ui");
    if wants_cli && wants_web {
        return Err("Use either --cli (SSH/CLI) or --web (UI), not both.".into());
    }
    if wants_cli {
        return Ok(InstallFrontend::Cli);
    }
    if wants_web {
        return Ok(InstallFrontend::Web);
    }
    // systemd / pipes: keep web. Interactive TTY: ask.
    if stdin_is_tty() && stderr_is_tty() {
        prompt_frontend_choice()
    } else {
        Ok(InstallFrontend::Web)
    }
}

fn stdin_is_tty() -> bool {
    io::stdin().is_terminal()
}
fn stderr_is_tty() -> bool {
    io::stderr().is_terminal()
}

fn prompt_frontend_choice() -> Result<InstallFrontend, String> {
    println!("CPN Installer {VERSION}\n");
    println!("How do you want to install?");
    println!("  1) Web UI   (open a browser; default for remote SSH tunnels)");
    println!("  2) SSH/CLI  (answer questions in this terminal)\n");
    loop {
        eprint!("Enter choice [1/2] (default: 1): ");
        let _ = io::stderr().flush();
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|e| format!("Failed to read install mode: {e}"))?;
        match line.trim() {
            "" | "1" | "web" | "ui" | "w" => return Ok(InstallFrontend::Web),
            "2" | "cli" | "ssh" | "c" | "s" => return Ok(InstallFrontend::Cli),
            other => {
                eprintln!(
                    "error: Unknown choice `{other}`. Enter 1 (Web UI) or 2 (SSH/CLI), or pass --web / --cli."
                );
            }
        }
    }
}

fn read_line(prompt: &str) -> Result<String, String> {
    eprint!("{prompt}");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("Failed to read input: {e}"))?;
    Ok(line.trim().to_string())
}

fn prompt_choice(prompt: &str, default: &str) -> Result<String, String> {
    let raw = read_line(&format!("{prompt} [{default}]: "))?;
    Ok(if raw.is_empty() {
        default.to_string()
    } else {
        raw
    })
}

/// Ask until `parse` accepts the choice (empty input already resolved to `default`).
/// Prints a clear English error and re-prompts; only I/O failures abort.
fn prompt_menu<T, F>(prompt: &str, default: &str, mut parse: F) -> Result<T, String>
where
    F: FnMut(&str) -> Result<T, String>,
{
    loop {
        let raw = prompt_choice(prompt, default)?;
        match parse(raw.as_str()) {
            Ok(value) => return Ok(value),
            Err(msg) => eprintln!("error: {msg}"),
        }
    }
}

fn prompt_yes_no(prompt: &str, default_yes: bool) -> Result<bool, String> {
    let hint = if default_yes { "Y/n" } else { "y/N" };
    loop {
        let raw = read_line(&format!("{prompt} [{hint}]: "))?;
        if raw.is_empty() {
            return Ok(default_yes);
        }
        match raw.to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            other => {
                eprintln!("error: Expected yes/no (y or n), got `{other}`.");
            }
        }
    }
}

fn prompt_server() -> Result<ServerEngine, String> {
    println!("\nWeb server engine:");
    println!("  1) OpenLiteSpeed  (recommended for WordPress / LSCache)");
    println!("  2) Nginx");
    println!("  3) Caddy");
    prompt_menu("Enter 1-3 for web server", "1", |raw| match raw {
        "1" | "ols" | "openlitespeed" => Ok(ServerEngine::Openlitespeed),
        "2" | "nginx" => Ok(ServerEngine::Nginx),
        "3" | "caddy" => Ok(ServerEngine::Caddy),
        other => Err(format!(
            "Unknown server `{other}`. Enter 1 (OpenLiteSpeed), 2 (Nginx), or 3 (Caddy)."
        )),
    })
}

fn prompt_database() -> Result<DatabaseEngine, String> {
    println!("\nDatabase defaults (installed with the web server):");
    println!("  1) MariaDB   (default)");
    println!("  2) MySQL");
    println!("  3) None      (skip local database packages)");
    prompt_menu("Enter 1-3 for database", "1", |raw| match raw {
        "1" | "mariadb" | "maria" => Ok(DatabaseEngine::Mariadb),
        "2" | "mysql" => Ok(DatabaseEngine::Mysql),
        "3" | "none" | "skip" => Ok(DatabaseEngine::None),
        other => Err(format!(
            "Unknown database `{other}`. Enter 1 (MariaDB), 2 (MySQL), or 3 (none)."
        )),
    })
}

fn parse_mail_option(raw: &str) -> Result<Option<MailSystem>, String> {
    match raw {
        "1" | "skip" | "none" | "n" => Ok(None),
        "2" | "snappymail" | "snappy" => Ok(Some(MailSystem::Snappymail)),
        "3" | "roundcube" => Ok(Some(MailSystem::Roundcube)),
        "4" | "thunderbird" => Ok(Some(MailSystem::Thunderbird)),
        other => Err(format!(
            "Unknown mail option `{other}`. Enter 1 (skip), 2, 3, or 4 (not an email address)."
        )),
    }
}

fn prompt_mail() -> Result<Option<MailSystem>, String> {
    println!("\nMail / webmail (optional; can skip):");
    println!("  1) Skip");
    println!("  2) SnappyMail");
    println!("  3) Roundcube");
    println!("  4) Thunderbird (desktop client package only)");
    println!("Enter a menu number 1-4 (not an email address). Empty uses 1 (Skip).");
    prompt_menu("Enter 1-4 for mail option", "1", parse_mail_option)
}

fn prompt_port(default: u16) -> Result<u16, String> {
    println!("\nPanel listen port (saved preference; default {DEFAULT_PORT}).");
    prompt_menu("Panel port", &default.to_string(), |raw| {
        let port: u16 = raw
            .parse()
            .map_err(|_| format!("Invalid port `{raw}` (use 1-65535)."))?;
        if port == 0 {
            return Err("Port must be between 1 and 65535.".into());
        }
        Ok(port)
    })
}

fn prompt_account(
    _policy: &PasswordPolicy,
) -> Result<(String, Option<String>, bool, String), String> {
    println!(
        "\nFirst panel account (the detected language is saved; change it later in the panel)."
    );
    let username = prompt_choice("Username (empty = admin)", "")?;
    let generate = prompt_yes_no("Generate a strong password", true)?;
    let password = if generate {
        None
    } else {
        loop {
            eprint!("Password: ");
            let _ = io::stderr().flush();
            let value =
                rpassword::read_password().map_err(|e| format!("Failed to read password: {e}"))?;
            if value.is_empty() {
                eprintln!("error: Password was empty. Enter a non-empty password.");
                continue;
            }
            eprint!("Confirm password: ");
            let _ = io::stderr().flush();
            let confirm = rpassword::read_password()
                .map_err(|e| format!("Failed to read password confirmation: {e}"))?;
            if confirm != value {
                eprintln!("error: Passwords do not match. Try again.");
                continue;
            }
            break Some(value);
        }
    };
    let email = loop {
        let email = read_line("Recovery email: ")?;
        if email.trim().is_empty() {
            eprintln!("error: Recovery email is required.");
            continue;
        }
        break email.trim().to_string();
    };
    Ok((username, password, generate, email))
}

fn make_cli_state(bind_port: u16) -> Arc<AppState> {
    let (events, _) = broadcast::channel(256);
    Arc::new(AppState {
        status: std::sync::RwLock::new(InstallerStatus {
            phase: "ready",
            progress: 0,
            message: "Ready for SSH/CLI install".into(),
            language: crate::http_helpers::detect_system_language(),
            listen_port: bind_port,
            ..InstallerStatus::default()
        }),
        events,
        token: "cli".into(),
        session_id: "clisessionid0000000000000001".into(),
        bind_port,
        allow_remote: false,
        allowed_hosts: crate::http_helpers::build_allowed_hosts(bind_port, &[]),
        cancel_requested: AtomicBool::new(false),
        active_child_pids: std::sync::Mutex::new(Vec::new()),
        install_log_detail: std::sync::Mutex::new(InstallLogDetail::Full),
    })
}

async fn pump_events(mut rx: broadcast::Receiver<InstallerEvent>) {
    while let Ok(event) = rx.recv().await {
        match event {
            InstallerEvent::Log { line, level } => println!("[{level}] {line}"),
            InstallerEvent::Progress { status } => {
                println!(
                    "[{}%] {}: {}",
                    status.progress, status.phase, status.message
                );
            }
            InstallerEvent::Completed { status } => {
                println!("[done] {}", status.message);
                break;
            }
            InstallerEvent::Error { status } => {
                if let Some(error) = status.error {
                    eprintln!("[error] {error}");
                } else {
                    eprintln!("[error] {}", status.message);
                }
                break;
            }
            InstallerEvent::Snapshot { .. } => {}
        }
    }
}

fn status_failed(state: &AppState) -> Option<String> {
    let status = state.status.read().unwrap_or_else(|e| e.into_inner());
    if status.phase == "failed" || status.error.is_some() {
        Some(
            status
                .error
                .clone()
                .unwrap_or_else(|| status.message.clone()),
        )
    } else {
        None
    }
}

fn fail(error: String) -> i32 {
    eprintln!("error: {error}");
    1
}

/// Run the full interactive SSH/CLI install wizard.
pub async fn run_interactive_cli(_args: &[String]) -> i32 {
    if !stdin_is_tty() {
        eprintln!(
            "error: --cli requires an interactive terminal. Re-run over SSH with a TTY, or use --web."
        );
        return 2;
    }
    if let Err(error) = require_root_for_mutation() {
        return fail(error);
    }
    if !is_root() {
        eprintln!("warning: not running as root; package installs may fail.");
    }

    println!("\nCPN Server Panel · SSH/CLI Installer {VERSION}");
    let detected_language = crate::http_helpers::detect_system_language();
    println!("Detected language: {detected_language}. You can change it later in the panel.\n");

    let server = match prompt_server() {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    let database = match prompt_database() {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    let install_phpmyadmin = if matches!(database, DatabaseEngine::None) {
        false
    } else {
        match prompt_yes_no("Install phpMyAdmin", true) {
            Ok(v) => v,
            Err(e) => return fail(e),
        }
    };
    let enable_proxy_front = match prompt_yes_no(
        "Enable Nginx proxy-front (advanced; unique internal IPs per domain)",
        false,
    ) {
        Ok(v) => v,
        Err(e) => return fail(e),
    };

    let preferred = listen_port::load_preferred_listen_port().unwrap_or(DEFAULT_PORT);
    let port = match prompt_port(preferred) {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    if let Err(error) = listen_port::save_preferred_listen_port(port) {
        eprintln!("warning: could not save preferred port: {error}");
    } else {
        println!("Saved preferred panel port: {port}");
    }

    println!("\nOptional panel hostname (DNS name for HTTPS login without a port).");
    println!("Leave empty to skip. Example: panel.example.com");
    let hostname = match read_line("Panel hostname: ") {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    if !hostname.is_empty() {
        if let Err(error) = save_panel_hostname(&hostname) {
            eprintln!("warning: could not save panel hostname: {error}");
        } else {
            println!("Saved panel hostname: {hostname}");
        }
    }

    println!("\nOptional external panel URL for emails and browsers (NAT labs, reverse proxies).");
    println!(
        "Leave empty to skip. Example for VirtualBox host forward 2089: http://127.0.0.1:2089"
    );
    let public_url = match read_line("Panel public URL: ") {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    if !public_url.is_empty() {
        if let Err(error) = save_panel_public_url(&public_url) {
            eprintln!("warning: could not save panel public URL: {error}");
        } else {
            println!("Saved panel public URL: {public_url}");
        }
    }

    let mail = match prompt_mail() {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    let policy = default_password_policy();
    let (username, password, generate, email) = match prompt_account(&policy) {
        Ok(v) => v,
        Err(e) => return fail(e),
    };

    println!("\nSummary");
    println!("  Web server : {}", server.label());
    println!("  Database   : {}", database.label());
    println!(
        "  phpMyAdmin : {}",
        if install_phpmyadmin { "yes" } else { "no" }
    );
    println!(
        "  Proxy front: {}",
        if enable_proxy_front { "yes" } else { "no" }
    );
    println!("  Panel port : {port}");
    if !hostname.is_empty() {
        println!("  Hostname   : {hostname}");
    }
    if !public_url.is_empty() {
        println!("  Public URL : {public_url}");
    }
    println!(
        "  Mail       : {}",
        mail.map(|m| m.label()).unwrap_or("skipped")
    );
    println!(
        "  Account    : {}",
        if username.is_empty() {
            "admin"
        } else {
            username.as_str()
        }
    );
    println!();
    match prompt_yes_no("Start installation now", true) {
        Ok(true) => {}
        Ok(false) => {
            println!("Aborted.");
            return 0;
        }
        Err(e) => return fail(e),
    }

    let state = make_cli_state(port);
    state.set_install_log_detail(InstallLogDetail::Full);
    println!(
        "Full installation logging is enabled: {}",
        crate::installer::installation_log_path().display()
    );
    let rx = state.events.subscribe();
    let pump = tokio::spawn(pump_events(rx));
    {
        let mut current = state.status.write().unwrap_or_else(|e| e.into_inner());
        current.selected_server = Some(server);
        current.phase = "configuring";
        current.progress = 0;
        current.error = None;
    }

    println!("\nInstalling web stack ({})...", server.label());
    crate::installer::install_with_database(
        state.clone(),
        server,
        database,
        install_phpmyadmin,
        enable_proxy_front,
    )
    .await;
    let _ = pump.await;
    if let Some(error) = status_failed(&state) {
        return fail(format!("web server install failed: {error}"));
    }

    if let Some(mail) = mail {
        let rx = state.events.subscribe();
        let pump = tokio::spawn(pump_events(rx));
        {
            let mut current = state.status.write().unwrap_or_else(|e| e.into_inner());
            current.selected_mail = Some(mail);
            current.phase = "downloading";
            current.progress = 0;
            current.error = None;
        }
        println!("\nInstalling mail ({})...", mail.label());
        crate::installer::install_mail(state.clone(), mail).await;
        let _ = pump.await;
        if let Some(error) = status_failed(&state) {
            return fail(format!("mail install failed: {error}"));
        }
    }

    match setup_account(
        &username,
        password.as_deref(),
        generate,
        &email,
        policy,
        &detected_language,
    ) {
        Ok(result) => {
            // Avoid cleartext username/email/password on stdout (CodeQL cleartext-logging).
            let _ = result.public;
            println!("\nFirst account ready.");
            if let Err(error) = print_generated(result.generated_password) {
                return fail(error);
            }
            println!("If a password file was written, store it securely, then delete it.");
        }
        Err(error) => return fail(format!("account setup failed: {error}")),
    }

    println!("\nInstallation finished.");
    crate::motd::ensure_motd_installed();
    let allow_remote = crate::panel_service::allow_remote_requested();
    crate::panel_service::ensure_panel_service_best_effort(
        crate::panel_service::PanelServiceMode::EnableAndStart,
        allow_remote,
    );
    let host = if hostname.is_empty() {
        None
    } else {
        Some(hostname.as_str())
    };
    crate::motd::print_panel_ready_banner(VERSION, port, host);
    println!("SSH logins will show the CPN MOTD (English) via /etc/profile.d/cpn-motd.sh.");
    println!("The panel uses the detected language unless you change it in the language menu.");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_select_cli_or_web() {
        let cli = resolve_install_frontend(&[
            "cpn-installer".into(),
            "--cli".into(),
            "--port".into(),
            "2089".into(),
        ])
        .unwrap();
        assert_eq!(cli, InstallFrontend::Cli);

        let web = resolve_install_frontend(&["cpn-installer".into(), "--web".into()]).unwrap();
        assert_eq!(web, InstallFrontend::Web);

        let ssh = resolve_install_frontend(&["cpn-installer".into(), "--ssh".into()]).unwrap();
        assert_eq!(ssh, InstallFrontend::Cli);

        assert!(
            resolve_install_frontend(&["cpn-installer".into(), "--cli".into(), "--web".into()])
                .is_err()
        );
    }

    #[test]
    fn non_tty_without_flags_defaults_web() {
        if !stdin_is_tty() {
            let mode = resolve_install_frontend(&["cpn-installer".into()]).unwrap();
            assert_eq!(mode, InstallFrontend::Web);
        }
    }

    #[test]
    fn mail_option_rejects_email_and_accepts_menu() {
        assert!(parse_mail_option("info@newstargeted.com").is_err());
        assert!(parse_mail_option("1").unwrap().is_none());
        assert!(matches!(
            parse_mail_option("2").unwrap(),
            Some(MailSystem::Snappymail)
        ));
        assert!(matches!(
            parse_mail_option("3").unwrap(),
            Some(MailSystem::Roundcube)
        ));
        assert!(matches!(
            parse_mail_option("4").unwrap(),
            Some(MailSystem::Thunderbird)
        ));
        let err = parse_mail_option("info@newstargeted.com").unwrap_err();
        assert!(err.contains("not an email address"));
    }
}
