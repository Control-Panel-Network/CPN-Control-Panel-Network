//! DKIM key store under `/var/lib/cpn/dkim/<domain>/`.

use crate::paths::join_data;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SELECTOR: &str = "default";

pub fn dkim_root() -> PathBuf {
    join_data("dkim")
}

pub fn ensure_dkim_root() -> Result<PathBuf, String> {
    let dir = dkim_root();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create DKIM dir {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

fn sanitize_domain(raw: &str) -> Result<String, String> {
    let d = raw.trim().trim_end_matches('.').to_ascii_lowercase();
    if d.is_empty() || !d.contains('.') || d.len() > 253 {
        return Err("DKIM domain is invalid".into());
    }
    if d.contains("..") || d.starts_with('.') || d.ends_with('-') {
        return Err("DKIM domain format is invalid".into());
    }
    Ok(d)
}

pub fn domain_dkim_dir(domain: &str) -> Result<PathBuf, String> {
    let domain = sanitize_domain(domain)?;
    Ok(ensure_dkim_root()?.join(domain))
}

pub fn private_key_path(domain: &str) -> Result<PathBuf, String> {
    Ok(domain_dkim_dir(domain)?.join(format!("{SELECTOR}.private")))
}

pub fn dns_txt_path(domain: &str) -> Result<PathBuf, String> {
    Ok(domain_dkim_dir(domain)?.join(format!("{SELECTOR}.txt")))
}

pub fn dkim_status_detail() -> (bool, String) {
    let dir = dkim_root();
    if !dir.is_dir() {
        return (
            false,
            format!(
                "No DKIM keys yet. Keys will be stored under {} when generated.",
                dir.display()
            ),
        );
    }
    let domains = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count()
        })
        .unwrap_or(0);
    (
        true,
        format!(
            "DKIM store at {} ({} domain folder(s)). Selector `{SELECTOR}`.",
            dir.display(),
            domains
        ),
    )
}

/// Ensure store exists and generate keys for `domain` when missing.
pub fn ensure_dkim_for_domain(domain: &str) -> Result<String, String> {
    let domain = sanitize_domain(domain)?;
    let dir = domain_dkim_dir(&domain)?;
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    let priv_path = private_key_path(&domain)?;
    if priv_path.is_file() {
        let txt = read_dkim_txt_value(&domain).unwrap_or_default();
        return Ok(format!(
            "DKIM keys already present for `{domain}` (selector `{SELECTOR}`). TXT length {}.",
            txt.len()
        ));
    }
    generate_keypair(&domain, &dir)?;
    Ok(format!(
        "Generated DKIM keys for `{domain}` under {} (selector `{SELECTOR}`).",
        dir.display()
    ))
}

fn openssl_bin() -> &'static str {
    if Path::new("/usr/bin/openssl").exists() {
        "/usr/bin/openssl"
    } else {
        "openssl"
    }
}

fn generate_keypair(domain: &str, dir: &Path) -> Result<(), String> {
    let priv_path = dir.join(format!("{SELECTOR}.private"));
    let pub_pem = dir.join(format!("{SELECTOR}.public.pem"));
    let status = Command::new(openssl_bin())
        .args(["genrsa", "-out"])
        .arg(&priv_path)
        .arg("2048")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("openssl genrsa failed to start: {e}"))?;
    if !status.success() {
        return Err("openssl genrsa failed".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&priv_path, fs::Permissions::from_mode(0o600));
    }
    let status = Command::new(openssl_bin())
        .args(["rsa", "-in"])
        .arg(&priv_path)
        .args(["-pubout", "-out"])
        .arg(&pub_pem)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("openssl rsa pubout failed: {e}"))?;
    if !status.success() {
        return Err("openssl rsa -pubout failed".into());
    }
    let b64 = public_key_base64(&pub_pem)?;
    let txt_value = format!("v=DKIM1; k=rsa; p={b64}");
    let dns_name = format!("{SELECTOR}._domainkey.{domain}");
    let zone_line = format!("{dns_name}. IN TXT \"{txt_value}\"\n");
    fs::write(dir.join(format!("{SELECTOR}.txt")), zone_line)
        .map_err(|e| format!("Cannot write DKIM TXT: {e}"))?;
    fs::write(dir.join(format!("{SELECTOR}.dnsvalue")), &txt_value)
        .map_err(|e| format!("Cannot write DKIM DNS value: {e}"))?;
    Ok(())
}

fn public_key_base64(pub_pem: &Path) -> Result<String, String> {
    let der = Command::new(openssl_bin())
        .args(["rsa", "-pubin", "-in"])
        .arg(pub_pem)
        .args(["-outform", "DER"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("openssl DER export failed: {e}"))?;
    if !der.status.success() || der.stdout.is_empty() {
        return Err("Could not export DKIM public key as DER".into());
    }
    Ok(data_encoding::BASE64.encode(&der.stdout))
}

/// DKIM TXT content (`v=DKIM1; k=rsa; p=...`) for DNS.
pub fn read_dkim_txt_value(domain: &str) -> Result<String, String> {
    let domain = sanitize_domain(domain)?;
    let value_path = domain_dkim_dir(&domain)?.join(format!("{SELECTOR}.dnsvalue"));
    if value_path.is_file() {
        return fs::read_to_string(value_path)
            .map(|s| s.trim().to_string())
            .map_err(|e| e.to_string());
    }
    let txt_path = dns_txt_path(&domain)?;
    let raw = fs::read_to_string(txt_path).map_err(|e| format!("DKIM TXT missing: {e}"))?;
    if let Some(start) = raw.find('"')
        && let Some(end) = raw[start + 1..].find('"')
    {
        return Ok(raw[start + 1..start + 1 + end].to_string());
    }
    Err("Could not parse DKIM TXT value".into())
}

pub fn dkim_dns_name(domain: &str) -> Result<String, String> {
    let domain = sanitize_domain(domain)?;
    Ok(format!("{SELECTOR}._domainkey.{domain}"))
}

pub fn selector() -> &'static str {
    SELECTOR
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn ensure_root_creates_dir() {
        with_test_data_dir(|| {
            let dir = ensure_dkim_root().unwrap();
            assert!(dir.is_dir());
            let (ready, detail) = dkim_status_detail();
            assert!(ready);
            assert!(detail.contains("DKIM store"));
        });
    }
}
