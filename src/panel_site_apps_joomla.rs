//! Joomla CMS installer for site document roots.

use crate::sites::SiteRecord;
use crate::wordpress_wpcli::is_wordpress_docroot;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const JOOMLA_TARGET: &str = "5.2.3";
const JOOMLA_ZIP_URL: &str =
    "https://downloads.joomla.org/cms/joomla5/5-2-3/Joomla_5.2.3-Stable-Full_Package.zip?format=zip";

#[derive(Debug, Clone)]
pub struct JoomlaStatus {
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

fn parse_joomla_version(text: &str) -> Option<String> {
    for needle in ["RELEASE", "JVERSION"] {
        if let Some(idx) = text.find(needle) {
            let slice = &text[idx..];
            let quote = slice.find('\'').or_else(|| slice.find('"'))?;
            let rest = &slice[quote + 1..];
            let end = rest.find(['\'', '"'])?;
            let ver = rest[..end].trim();
            if !ver.is_empty() && ver.len() < 24 {
                return Some(ver.to_string());
            }
        }
    }
    None
}

pub fn detect_joomla(site: &SiteRecord) -> JoomlaStatus {
    let doc = Path::new(&site.docroot);
    let version_file = doc.join("libraries/src/Version.php");
    let version = if version_file.is_file() {
        parse_joomla_version(&read_limited(&version_file, 12_288)).unwrap_or_default()
    } else {
        String::new()
    };
    let installed = version_file.is_file()
        || doc.join("configuration.php").is_file()
        || doc.join("administrator/index.php").is_file();
    let detail = if installed {
        if version.is_empty() {
            "Joomla files detected in this site document root.".into()
        } else {
            format!("Joomla {version} detected in this site document root.")
        }
    } else {
        format!("Joomla is not installed. Target {JOOMLA_TARGET}.")
    };
    JoomlaStatus {
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
            "-L",
            "-o",
            &dest.display().to_string(),
            JOOMLA_ZIP_URL,
        ])
        .status()
        .map_err(|e| format!("curl failed: {e}"))?;
    if !status.success() {
        return Err("Could not download Joomla package.".into());
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
        return Err("Could not extract Joomla archive into the document root.".into());
    }
    Ok(())
}

pub fn install_joomla_version(site: &SiteRecord, version: &str) -> Result<String, String> {
    if version.trim() != JOOMLA_TARGET {
        return Err(format!(
            "Joomla {version} is not in the verified installer catalog (use {JOOMLA_TARGET})."
        ));
    }
    let doc = Path::new(&site.docroot);
    if is_wordpress_docroot(doc) {
        return Err(
            "This document root looks like WordPress. Install Joomla on a dedicated site or subdomain."
                .into(),
        );
    }
    let current = detect_joomla(site);
    if current.installed {
        return Ok(format!(
            "Joomla already present{}. Use Apps lifecycle update/restore if needed.",
            if current.version.is_empty() {
                String::new()
            } else {
                format!(" ({})", current.version)
            }
        ));
    }
    let cache = PathBuf::from("/var/lib/cpn/cache/joomla");
    let zip = cache.join(format!("joomla-{JOOMLA_TARGET}.zip"));
    download_zip(&zip)?;
    if doc.is_dir() {
        let _ = crate::site_app_backups::create_app_backup(site, "joomla", JOOMLA_TARGET, doc);
    }
    extract_zip(&zip, doc)?;
    Ok(format!(
        "Joomla {JOOMLA_TARGET} extracted into {}. Finish the web installer, then remove the installation folder.",
        doc.display()
    ))
}

pub fn install_joomla(site: &SiteRecord) -> Result<String, String> {
    install_joomla_version(site, JOOMLA_TARGET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_version_constant() {
        assert_eq!(JOOMLA_TARGET, "5.2.3");
    }
}
