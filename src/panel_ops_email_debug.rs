//! Email debugger: DNS MX/SPF/DKIM/DMARC summary, SMTP probe, recent log lines.

use crate::panel_ops_security::{cmd_stdout, which_exists};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

fn safe_domain(domain: &str) -> Result<String, String> {
    let d = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if d.is_empty() || d.len() > 253 {
        return Err("Domain is required".into());
    }
    if !d
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err("Domain may only contain letters, digits, dots, and hyphens".into());
    }
    if d.contains("..") || d.starts_with('-') || d.starts_with('.') {
        return Err("Invalid domain".into());
    }
    Ok(d)
}

fn dig_or_host(name: &str, rtype: &str) -> String {
    if which_exists("dig") {
        if let Some(out) = cmd_stdout("dig", &["+short", name, rtype]) {
            if !out.is_empty() {
                return out;
            }
        }
    }
    if which_exists("host") {
        if let Some(out) = cmd_stdout("host", &["-t", rtype, name]) {
            return out;
        }
    }
    "(no dig/host output)".into()
}

fn smtp_probe(host: &str, port: u16) -> String {
    use std::net::ToSocketAddrs;
    let addr = format!("{host}:{port}");
    let Ok(mut addrs) = addr.to_socket_addrs() else {
        return format!("Could not resolve {addr}");
    };
    let Some(sock) = addrs.next() else {
        return format!("No address for {addr}");
    };
    match TcpStream::connect_timeout(&sock, Duration::from_secs(3)) {
        Ok(mut stream) => {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut buf = [0u8; 512];
            match stream.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let banner = String::from_utf8_lossy(&buf[..n]).trim().to_string();
                    format!("Connected to {addr}; banner: {banner}")
                }
                _ => format!("Connected to {addr}; no banner"),
            }
        }
        Err(e) => format!("SMTP probe to {addr} failed: {e}"),
    }
}

fn recent_mail_log_lines(needle: &str, max_lines: usize) -> String {
    let candidates = [
        "/var/log/maillog",
        "/var/log/mail.log",
        "/var/log/postfix.log",
    ];
    for path in candidates {
        if !Path::new(path).is_file() {
            continue;
        }
        if let Ok(mut f) = fs::File::open(path) {
            let _ = f.seek(SeekFrom::End(0));
            let len = f.stream_position().unwrap_or(0);
            let start = len.saturating_sub(64 * 1024);
            let _ = f.seek(SeekFrom::Start(start));
            let mut raw = String::new();
            let _ = f.read_to_string(&mut raw);
            let matched: Vec<&str> = raw
                .lines()
                .rev()
                .filter(|l| {
                    l.to_ascii_lowercase()
                        .contains(&needle.to_ascii_lowercase())
                })
                .take(max_lines)
                .collect();
            if matched.is_empty() {
                return format!("No recent lines matching `{needle}` in {path}.");
            }
            let mut out = matched;
            out.reverse();
            return out.join("\n");
        }
    }
    "No mail log found (/var/log/maillog or /var/log/mail.log).".into()
}

#[derive(Debug, Clone)]
pub struct DebugReport {
    pub domain: String,
    pub mx: String,
    pub spf: String,
    pub dkim: String,
    pub dmarc: String,
    pub smtp_local: String,
    pub logs: String,
}

pub fn run_debug(domain: &str) -> Result<DebugReport, String> {
    let domain = safe_domain(domain)?;
    let mx = dig_or_host(&domain, "MX");
    let spf = dig_or_host(&domain, "TXT")
        .lines()
        .filter(|l| l.contains("v=spf1") || l.contains("spf"))
        .collect::<Vec<_>>()
        .join("\n");
    let spf = if spf.is_empty() {
        dig_or_host(&domain, "TXT")
    } else {
        spf
    };
    let dkim = dig_or_host(&format!("default._domainkey.{domain}"), "TXT");
    let dmarc = dig_or_host(&format!("_dmarc.{domain}"), "TXT");
    let smtp_local = smtp_probe("127.0.0.1", 25);
    let logs = recent_mail_log_lines(&domain, 40);
    Ok(DebugReport {
        domain,
        mx,
        spf,
        dkim,
        dmarc,
        smtp_local,
        logs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_validation() {
        assert!(safe_domain("example.com").is_ok());
        assert!(safe_domain("../etc").is_err());
        assert!(safe_domain("").is_err());
    }
}
