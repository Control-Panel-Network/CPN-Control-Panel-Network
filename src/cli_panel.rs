//! `cpn panel` subcommands: live login URL and panel status (no secrets).

use crate::listen_port::{DEFAULT_PORT, load_preferred_listen_port};
use crate::motd::ensure_motd_installed;
use crate::panel_network::{load_panel_hostname, preferred_listen_port_or_default, public_base_url};
use crate::panel_public_url::{load_panel_public_url, local_listen_base_url};
use crate::panel_service::UNIT_NAME;
use clap::Subcommand;
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Subcommand, Debug)]
pub enum PanelCommands {
    /// Print live panel login URL(s) from current CPN config (no secrets)
    Url {
        /// Compact key=value lines (for scripts)
        #[arg(long)]
        raw: bool,
        /// Indented human lines for embedding in SSH MOTD
        #[arg(long)]
        motd: bool,
    },
    /// Print panel version, service state, and live login URL(s)
    Status {
        #[arg(long)]
        raw: bool,
    },
    /// Reinstall `/etc/profile.d/cpn-motd.sh` from the embedded script (root)
    InstallMotd,
}

/// Live panel URL facts resolved from disk prefs (same files the UI writes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelUrlInfo {
    pub listen_port: u16,
    pub panel_public_url: Option<String>,
    pub panel_hostname: Option<String>,
    pub primary_base: String,
    pub primary_login: String,
    pub local_base: String,
    pub local_login: String,
    pub hostname_login: Option<String>,
}

fn service_active_label() -> String {
    let output = Command::new("systemctl")
        .args(["is-active", UNIT_NAME])
        .output();
    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if text.is_empty() {
                "unknown".into()
            } else {
                text
            }
        }
        Err(_) => "unknown".into(),
    }
}

/// Resolve login URLs from live CPN data dir (port / public URL / hostname).
pub fn resolve_panel_url_info() -> PanelUrlInfo {
    let listen_port = load_preferred_listen_port().unwrap_or(DEFAULT_PORT);
    let listen_port = if listen_port == 0 {
        preferred_listen_port_or_default()
    } else {
        listen_port
    };
    let panel_public_url = load_panel_public_url();
    let panel_hostname = load_panel_hostname();
    let primary_base = public_base_url(listen_port, None);
    let local_base = local_listen_base_url(listen_port);
    let hostname_login = panel_hostname
        .as_ref()
        .map(|host| format!("https://{host}/login"));
    PanelUrlInfo {
        listen_port,
        panel_public_url,
        panel_hostname,
        primary_login: format!("{primary_base}/login"),
        primary_base,
        local_login: format!("{local_base}/login"),
        local_base,
        hostname_login,
    }
}

fn print_url_human(info: &PanelUrlInfo, indent: &str) {
    println!("{indent}Login URL     : {}", info.primary_login);
    if info.primary_base != info.local_base {
        println!("{indent}Local login   : {}", info.local_login);
    }
    if let Some(ref host_login) = info.hostname_login {
        let host_base = host_login.trim_end_matches("/login");
        if host_base != info.primary_base && host_base != info.local_base {
            println!("{indent}Hostname login: {host_login}");
        }
    }
    println!("{indent}Listen port   : {}", info.listen_port);
    if let Some(ref public) = info.panel_public_url {
        println!("{indent}Public URL    : {public}");
        println!(
            "{indent}Lab tip       : Windows host may use the public URL when NAT forwards the guest port"
        );
    } else if info.panel_hostname.is_none() {
        println!(
            "{indent}Lab tip       : ssh -L {0}:127.0.0.1:{0} user@host",
            info.listen_port
        );
        println!(
            "{indent}VBox NAT tip  : host forward 2089->{0} => http://127.0.0.1:2089/login",
            info.listen_port
        );
    }
}

fn print_url_raw(info: &PanelUrlInfo) {
    println!("login_url={}", info.primary_login);
    println!("local_login_url={}", info.local_login);
    println!("listen_port={}", info.listen_port);
    println!(
        "panel_public_url={}",
        info.panel_public_url.as_deref().unwrap_or("")
    );
    println!(
        "panel_hostname={}",
        info.panel_hostname.as_deref().unwrap_or("")
    );
    println!("public_base_url={}", info.primary_base);
    if let Some(ref host_login) = info.hostname_login {
        println!("hostname_login_url={host_login}");
    }
}

pub fn run_panel(
    command: PanelCommands,
    require_root: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match command {
        PanelCommands::Url { raw, motd } => {
            let info = resolve_panel_url_info();
            if raw {
                print_url_raw(&info);
            } else if motd {
                print_url_human(&info, "  ");
            } else {
                print_url_human(&info, "");
            }
            Ok(())
        }
        PanelCommands::Status { raw } => {
            let info = resolve_panel_url_info();
            let active = service_active_label();
            if raw {
                println!("version={VERSION}");
                println!("service={UNIT_NAME}");
                println!("service_active={active}");
                print_url_raw(&info);
            } else {
                println!("CPN / Control Panel Network");
                println!("Version       : {VERSION}");
                println!("Service       : {UNIT_NAME} ({active})");
                print_url_human(&info, "");
                println!("Start panel   : sudo systemctl start {UNIT_NAME}");
                println!("Or foreground : sudo cpn-installer --web");
                println!("CLI install   : sudo cpn-installer --cli");
            }
            Ok(())
        }
        PanelCommands::InstallMotd => {
            require_root()?;
            ensure_motd_installed();
            println!("motd_path=/etc/profile.d/cpn-motd.sh");
            println!("motd_lib=/usr/lib/cpn/cpn-motd.sh");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::listen_port::save_preferred_listen_port;
    use crate::panel_network::{clear_panel_hostname, save_panel_hostname};
    use crate::panel_public_url::{clear_panel_public_url, save_panel_public_url};

    #[test]
    fn live_url_follows_listen_port_file() {
        with_test_data_dir(|| {
            clear_panel_public_url().unwrap();
            clear_panel_hostname().unwrap();
            save_preferred_listen_port(2099).unwrap();
            let info = resolve_panel_url_info();
            assert_eq!(info.listen_port, 2099);
            assert_eq!(info.primary_login, "http://127.0.0.1:2099/login");
            assert_eq!(info.local_login, "http://127.0.0.1:2099/login");
        });
    }

    #[test]
    fn live_url_prefers_public_url_over_port() {
        with_test_data_dir(|| {
            save_preferred_listen_port(2087).unwrap();
            save_panel_public_url("http://127.0.0.1:2089").unwrap();
            let info = resolve_panel_url_info();
            assert_eq!(info.listen_port, 2087);
            assert_eq!(info.primary_login, "http://127.0.0.1:2089/login");
            assert_eq!(info.local_login, "http://127.0.0.1:2087/login");
            clear_panel_public_url().unwrap();
        });
    }

    #[test]
    fn live_url_uses_hostname_when_no_public() {
        with_test_data_dir(|| {
            clear_panel_public_url().unwrap();
            save_preferred_listen_port(2087).unwrap();
            save_panel_hostname("panel.example.com").unwrap();
            let info = resolve_panel_url_info();
            assert_eq!(info.primary_login, "https://panel.example.com/login");
            assert_eq!(info.local_login, "http://127.0.0.1:2087/login");
            clear_panel_hostname().unwrap();
        });
    }
}
