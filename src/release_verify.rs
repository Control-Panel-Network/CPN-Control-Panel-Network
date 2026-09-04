//! Verify downloaded GitHub Release artifacts before privileged install (issue #16).

use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// When true (default), refuse remote release installs that fail checksum verification.
pub fn verify_release_enabled() -> bool {
    match std::env::var("CPN_VERIFY_RELEASE") {
        Ok(value) => {
            let lower = value.trim().to_ascii_lowercase();
            !(lower == "0" || lower == "false" || lower == "no" || lower == "off")
        }
        Err(_) => true,
    }
}

/// When true, also require a GPG-verified `SHA256SUMS.asc` (needs `gpg` on PATH).
pub fn verify_gpg_enabled() -> bool {
    match std::env::var("CPN_VERIFY_GPG") {
        Ok(value) => {
            let lower = value.trim().to_ascii_lowercase();
            lower == "1" || lower == "true" || lower == "yes" || lower == "on"
        }
        Err(_) => false,
    }
}

fn sha256_hex(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Parse GNU `sha256sum` lines (`<hash>  <filename>` or `<hash> *<filename>`).
pub fn expected_hash_for(sums_body: &str, file_name: &str) -> Option<String> {
    for line in sums_body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        if name == file_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

pub fn verify_sha256_file(artifact: &Path, sums_body: &str) -> Result<(), String> {
    let name = artifact
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Artifact path has no file name".to_string())?;
    let expected = expected_hash_for(sums_body, name).ok_or_else(|| {
        format!(
            "SHA256SUMS has no entry for {name}; refuse install while CPN_VERIFY_RELEASE is enabled"
        )
    })?;
    let actual = sha256_hex(artifact)?;
    if actual != expected {
        return Err(format!(
            "SHA-256 mismatch for {name}: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

pub async fn verify_gpg_sums(
    sums_path: &Path,
    asc_path: &Path,
    keyring: Option<&Path>,
) -> Result<(), String> {
    if !asc_path.is_file() {
        return Err(format!(
            "Missing {} while CPN_VERIFY_GPG=1",
            asc_path.display()
        ));
    }
    let gnupg_home = format!(
        "/var/tmp/cpn-gpg-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    std::fs::create_dir_all(&gnupg_home)
        .map_err(|error| format!("Could not create GNUPGHOME: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&gnupg_home, std::fs::Permissions::from_mode(0o700));
    }

    if let Some(key) = keyring.filter(|path| path.is_file()) {
        let status = Command::new("gpg")
            .env("GNUPGHOME", &gnupg_home)
            .args(["--batch", "--import"])
            .arg(key)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .await
            .map_err(|error| format!("gpg import failed: {error}"))?;
        if !status.success() {
            let _ = std::fs::remove_dir_all(&gnupg_home);
            return Err("gpg could not import CPN release public key".into());
        }
    }

    let output = Command::new("gpg")
        .env("GNUPGHOME", &gnupg_home)
        .args(["--batch", "--verify"])
        .arg(asc_path)
        .arg(sums_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("gpg verify failed to start: {error}"))?;
    let _ = std::fs::remove_dir_all(&gnupg_home);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "GPG verification of SHA256SUMS failed: {}",
            stderr.trim().chars().take(200).collect::<String>()
        ));
    }
    Ok(())
}

/// Optional `rpm --checksig` when the artifact is an RPM and `rpm` exists.
pub async fn maybe_check_rpm_sig(artifact: &Path) -> Result<(), String> {
    if artifact
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("rpm"))
        != Some(true)
    {
        return Ok(());
    }
    let rpm_available = Command::new("rpm")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false);
    if !rpm_available {
        return Ok(());
    }
    let output = Command::new("rpm")
        .args(["--checksig"])
        .arg(artifact)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| format!("rpm --checksig failed: {error}"))?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        return Err(format!(
            "rpm --checksig failed for {}: {}",
            artifact.display(),
            combined.trim().chars().take(200).collect::<String>()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{expected_hash_for, verify_release_enabled};

    #[test]
    fn parses_sha256sums_lines() {
        let body = "abcdef0123456789  cpn-installer.rpm\n111  other.bin\n";
        assert_eq!(
            expected_hash_for(body, "cpn-installer.rpm").as_deref(),
            Some("abcdef0123456789")
        );
        assert!(expected_hash_for(body, "missing").is_none());
    }

    #[test]
    fn verify_flag_defaults_on() {
        // Unset in unit tests may still be present from the environment; just ensure parsing works.
        unsafe {
            std::env::set_var("CPN_VERIFY_RELEASE", "0");
        }
        assert!(!verify_release_enabled());
        unsafe {
            std::env::set_var("CPN_VERIFY_RELEASE", "1");
        }
        assert!(verify_release_enabled());
        unsafe {
            std::env::remove_var("CPN_VERIFY_RELEASE");
        }
    }
}
