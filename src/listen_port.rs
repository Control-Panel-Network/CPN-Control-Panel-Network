//! Listen-port resolution for the CPN web installer (default 2087).

use std::env;
use std::fs;
use std::path::PathBuf;

/// Default installer/panel listen port (Cloudflare-friendly; WHM HTTPS family).
pub const DEFAULT_PORT: u16 = 2087;

/// Preference file under the CPN data directory (override with `CPN_DATA_DIR`).
fn preference_path() -> PathBuf {
    let base = env::var("CPN_DATA_DIR").unwrap_or_else(|_| "/var/lib/cpn".into());
    PathBuf::from(base).join("listen_port")
}

/// Validate a TCP listen port (`1`..=`65535`). Prefer `>1024` when not root.
pub fn validate_listen_port(port: u16) -> Result<u16, String> {
    if port == 0 {
        return Err("Listen port must be between 1 and 65535".into());
    }
    Ok(port)
}

pub fn load_preferred_listen_port() -> Option<u16> {
    let path = preference_path();
    let raw = fs::read_to_string(&path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let port: u16 = trimmed.parse().ok()?;
    validate_listen_port(port).ok()
}

pub fn save_preferred_listen_port(port: u16) -> Result<(), String> {
    let port = validate_listen_port(port)?;
    let path = preference_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create data directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(&path, format!("{port}\n"))
        .map_err(|error| format!("Could not save listen port preference: {error}"))?;
    Ok(())
}

fn parse_port_value(raw: &str, source: &str) -> Result<u16, String> {
    let trimmed = raw.trim();
    let port: u16 = trimmed
        .parse()
        .map_err(|_| format!("Invalid {source} value '{trimmed}' (expected 1-65535)"))?;
    validate_listen_port(port).map_err(|error| format!("{source}: {error}"))
}

/// Resolve bind port: `--port` > `CPN_LISTEN_PORT` > saved preference > default `2087`.
pub fn resolve_listen_port(args: &[String]) -> Result<u16, String> {
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--port" {
            let value = iter
                .next()
                .ok_or_else(|| "Missing value for --port (example: --port 2087)".to_string())?;
            return parse_port_value(value, "--port");
        }
        if let Some(value) = arg.strip_prefix("--port=") {
            return parse_port_value(value, "--port");
        }
    }

    if let Ok(raw) = env::var("CPN_LISTEN_PORT") {
        if !raw.trim().is_empty() {
            return parse_port_value(&raw, "CPN_LISTEN_PORT");
        }
    }

    if let Some(preferred) = load_preferred_listen_port() {
        return Ok(preferred);
    }

    Ok(DEFAULT_PORT)
}

pub fn print_installer_help(version: &str) {
    println!("cpn-installer {version}");
    println!();
    println!("Usage: cpn-installer [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --port <PORT>       Listen port (default: {DEFAULT_PORT})");
    println!("                      Also: CPN_LISTEN_PORT, or /var/lib/cpn/listen_port");
    println!("  --allow-remote      Bind 0.0.0.0 (HTTP without TLS; lab/operator opt-in)");
    println!("  --listen-all        Alias of --allow-remote");
    println!("  -h, --help          Show this help");
    println!("  -V, --version       Show version");
    println!();
    println!("Ports 1-65535 are accepted. Prefer >1024 unless running as root.");
    println!(
        "Default {DEFAULT_PORT} matches the cPanel WHM HTTPS port family (Cloudflare-friendly)."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_port_is_2087() {
        assert_eq!(DEFAULT_PORT, 2087);
    }

    #[test]
    fn rejects_port_zero() {
        assert!(validate_listen_port(0).is_err());
    }

    #[test]
    fn accepts_whm_family_and_lab_ports() {
        assert_eq!(validate_listen_port(2087).unwrap(), 2087);
        assert_eq!(validate_listen_port(8787).unwrap(), 8787);
        assert_eq!(validate_listen_port(9443).unwrap(), 9443);
    }

    #[test]
    fn cli_port_overrides_env_and_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: serialized by ENV_LOCK for this test module.
        unsafe {
            env::remove_var("CPN_LISTEN_PORT");
            env::remove_var("CPN_DATA_DIR");
        }
        let args = vec!["cpn-installer".into(), "--port".into(), "9443".into()];
        assert_eq!(resolve_listen_port(&args).unwrap(), 9443);
    }

    #[test]
    fn env_port_used_without_cli() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("cpn-listen-port-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // SAFETY: serialized by ENV_LOCK for this test module.
        unsafe {
            env::set_var("CPN_DATA_DIR", &dir);
            env::set_var("CPN_LISTEN_PORT", "3333");
        }
        let args = vec!["cpn-installer".into()];
        let result = resolve_listen_port(&args);
        // SAFETY: serialized by ENV_LOCK for this test module.
        unsafe {
            env::remove_var("CPN_LISTEN_PORT");
            env::remove_var("CPN_DATA_DIR");
        }
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(result.unwrap(), 3333);
    }
}
