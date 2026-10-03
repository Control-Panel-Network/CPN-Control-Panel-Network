//! CMS Made Simple detect and install into a site docroot (2.2.x installer).

use crate::sites::SiteRecord;
use crate::wordpress_wpcli::is_wordpress_docroot;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CMSMS_TARGET: &str = "2.2.23";
pub const CMSMS_INSTALLER: &str = "cmsms-2.2.23-install.php";
const CMSMS_INSTALLER_URL: &str =
    "https://s3.amazonaws.com/cmsms/downloads/15022/cmsms-2.2.23-install.php";

#[derive(Debug, Clone)]
pub struct CmsmsStatus {
    pub installed: bool,
    pub version: String,
    pub installer_present: bool,
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

fn parse_cms_version(text: &str) -> Option<String> {
    for needle in ["$CMS_VERSION", "CMS_VERSION"] {
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

fn version_file(docroot: &Path) -> Option<PathBuf> {
    for rel in ["lib/version.php", "version.php"] {
        let p = docroot.join(rel);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn detect_cmsms(site: &SiteRecord) -> CmsmsStatus {
    let doc = Path::new(&site.docroot);
    let installer = doc.join(CMSMS_INSTALLER);
    let installer_present = installer.is_file();
    let version = version_file(doc)
        .and_then(|p| parse_cms_version(&read_limited(&p, 8_192)))
        .unwrap_or_default();
    let config_php = doc.join("config.php").is_file();
    let include_php = doc.join("include.php").is_file() || doc.join("lib/include.php").is_file();
    let installed = !version.is_empty() || (config_php && include_php);
    let detail = if installed {
        if version.is_empty() {
            "CMS Made Simple files detected in this site document root.".into()
        } else {
            format!("CMS Made Simple {version} detected in this site document root.")
        }
    } else if installer_present {
        format!("Installer {CMSMS_INSTALLER} is in the document root. Finish setup in the browser, then remove the installer.")
    } else {
        format!("CMS Made Simple is not installed. Target {CMSMS_TARGET}.")
    };
    CmsmsStatus {
        installed,
        version,
        installer_present,
        detail,
    }
}

pub fn install_cmsms(site: &SiteRecord) -> Result<String, String> {
    let doc = Path::new(&site.docroot);
    if is_wordpress_docroot(doc) {
        return Err(
            "This document root looks like WordPress. Use WordPress for that site; CMS Made Simple stays on its own install."
                .into(),
        );
    }
    let current = detect_cmsms(site);
    if current.installed {
        return Ok(current.detail);
    }
    fs::create_dir_all(doc).map_err(|e| format!("Could not create document root: {e}"))?;
    let dest = doc.join(CMSMS_INSTALLER);
    if dest.is_file() {
        return Ok(format!(
            "Installer already present as {CMSMS_INSTALLER}. Open the site and finish CMS Made Simple setup."
        ));
    }
    let cache_dir = crate::paths::join_data("cache");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("Could not create cache dir: {e}"))?;
    let cached = cache_dir.join(CMSMS_INSTALLER);
    if !cached.is_file()
        || fs::metadata(&cached)
            .map(|m| m.len() < 80_000)
            .unwrap_or(true)
    {
        let tmp = cache_dir.join(format!("{CMSMS_INSTALLER}.tmp"));
        let tmp_s = tmp
            .to_str()
            .ok_or_else(|| "Cache path is not valid UTF-8".to_string())?;
        let status = Command::new("curl")
            .args([
                "-fL",
                "--connect-timeout",
                "20",
                "--max-time",
                "180",
                "-o",
                tmp_s,
                CMSMS_INSTALLER_URL,
            ])
            .status()
            .map_err(|e| format!("curl is not available: {e}"))?;
        if !status.success() {
            let _ = fs::remove_file(&tmp);
            return Err(
                "Could not download the CMS Made Simple 2.2.23 installer. Try again when the host can reach the download server."
                    .into(),
            );
        }
        fs::rename(&tmp, &cached).map_err(|e| format!("Could not store installer: {e}"))?;
    }
    fs::copy(&cached, &dest).map_err(|e| format!("Could not copy installer into the site: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o644));
    }
    Ok(format!(
        "Copied CMS Made Simple {CMSMS_TARGET} installer to the site document root as {CMSMS_INSTALLER}. Open the site to finish setup, then remove the installer."
    ))
}

pub fn remove_cmsms_installer(site: &SiteRecord) -> Result<String, String> {
    let dest = Path::new(&site.docroot).join(CMSMS_INSTALLER);
    if !dest.is_file() {
        return Ok("No CMS Made Simple installer file was present.".into());
    }
    fs::remove_file(&dest).map_err(|e| format!("Could not remove installer: {e}"))?;
    Ok("Removed the CMS Made Simple installer from the document root.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_from_php() {
        let sample = "$CMS_VERSION = '2.2.23';\n";
        assert_eq!(parse_cms_version(sample).as_deref(), Some("2.2.23"));
    }
}
