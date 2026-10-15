//! Mautic marketing automation installer for site document roots.

use crate::sites::SiteRecord;
use crate::wordpress_wpcli::is_wordpress_docroot;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MAUTIC_TARGET: &str = "5.2.1";
const MAUTIC_ZIP_URL: &str = "https://github.com/mautic/mautic/releases/download/5.2.1/5.2.1.zip";

#[derive(Debug, Clone)]
pub struct MauticStatus {
    pub installed: bool,
    pub version: String,
    pub detail: String,
}

fn read_limited(path: &Path, max: usize) -> String {
    fs::read(path)
        .ok()
        .map(|b| {
            let n = b.len().min(max);
            String::from_utf8_lossy(&b[..n]).into_owned()
        })
        .unwrap_or_default()
}

fn version_from_composer(doc: &Path) -> Option<String> {
    let composer = doc.join("composer.json");
    if !composer.is_file() {
        return None;
    }
    let text = read_limited(&composer, 16_384);
    if let Some(idx) = text.find("\"version\"") {
        let slice = &text[idx..];
        let quote = slice.find(':')?;
        let rest = slice[quote + 1..].trim();
        let q = rest.find('"')?;
        let rest = &rest[q + 1..];
        let end = rest.find('"')?;
        let ver = rest[..end].trim();
        if !ver.is_empty() && ver.len() < 32 {
            return Some(ver.to_string());
        }
    }
    None
}

pub fn detect_mautic(site: &SiteRecord) -> MauticStatus {
    let doc = Path::new(&site.docroot);
    let app_kernel = doc.join("app/AppKernel.php").is_file()
        || doc.join("bin/console").is_file()
        || doc.join("app/config/local.php").is_file();
    let version = version_from_composer(doc).unwrap_or_default();
    let installed = app_kernel || !version.is_empty() || doc.join("app").is_dir();
    let detail = if installed {
        if version.is_empty() {
            "Mautic files detected in this site document root.".into()
        } else {
            format!("Mautic {version} detected in this site document root.")
        }
    } else {
        format!("Mautic is not installed. Target {MAUTIC_TARGET}.")
    };
    MauticStatus {
        installed,
        version,
        detail,
    }
}

fn download_zip(dest: &Path) -> Result<(), String> {
    if dest.is_file() && dest.metadata().map(|m| m.len() > 100_000).unwrap_or(false) {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create cache dir: {e}"))?;
    }
    let status = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "20",
            "--max-time",
            "300",
            "-o",
            &dest.display().to_string(),
            MAUTIC_ZIP_URL,
        ])
        .status()
        .map_err(|e| format!("curl failed: {e}"))?;
    if !status.success() {
        return Err("Could not download Mautic release zip from GitHub.".into());
    }
    Ok(())
}

fn extract_zip(zip: &Path, doc: &Path) -> Result<(), String> {
    fs::create_dir_all(doc).map_err(|e| format!("Could not create document root: {e}"))?;
    let status = Command::new("unzip")
        .args([
            "-qo",
            &zip.display().to_string(),
            "-d",
            &doc.display().to_string(),
        ])
        .status()
        .or_else(|_| {
            Command::new("bsdtar")
                .args([
                    "-xf",
                    &zip.display().to_string(),
                    "-C",
                    &doc.display().to_string(),
                ])
                .status()
        })
        .map_err(|e| format!("unzip/bsdtar failed: {e}"))?;
    if !status.success() {
        return Err("Could not extract Mautic archive into the document root.".into());
    }
    Ok(())
}

pub fn install_mautic_version(site: &SiteRecord, version: &str) -> Result<String, String> {
    if version.trim() != MAUTIC_TARGET {
        return Err(format!(
            "Mautic {version} is not in the verified installer catalog (use {MAUTIC_TARGET})."
        ));
    }
    let doc = Path::new(&site.docroot);
    if is_wordpress_docroot(doc) {
        return Err(
            "This document root looks like WordPress. Install Mautic on a dedicated site or subdomain."
                .into(),
        );
    }
    let current = detect_mautic(site);
    if current.installed {
        return Ok(format!(
            "Mautic already present{}. Use update/restore from Apps lifecycle if needed.",
            if current.version.is_empty() {
                String::new()
            } else {
                format!(" ({})", current.version)
            }
        ));
    }
    let cache = PathBuf::from("/var/lib/cpn/cache/mautic");
    let zip = cache.join(format!("mautic-{MAUTIC_TARGET}.zip"));
    download_zip(&zip)?;
    if doc.is_dir() {
        let _ = crate::site_app_backups::create_app_backup(site, "mautic", MAUTIC_TARGET, doc);
    }
    extract_zip(&zip, doc)?;
    Ok(format!(
        "Mautic {MAUTIC_TARGET} extracted into {}. Finish web installer in the browser, then remove public install leftovers.",
        doc.display()
    ))
}

pub fn install_mautic(site: &SiteRecord) -> Result<String, String> {
    install_mautic_version(site, MAUTIC_TARGET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_version_constant() {
        assert_eq!(MAUTIC_TARGET, "5.2.1");
        assert!(MAUTIC_ZIP_URL.contains(MAUTIC_TARGET));
    }
}
