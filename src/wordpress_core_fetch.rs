//! Fast WordPress core fetch via curl + tar (WP-CLI download fallback lives in install).

use crate::paths;
use std::fs;
use std::path::Path;
use std::process::Command;

const WP_CORE_TARBALL_URL: &str = "https://wordpress.org/latest.tar.gz";

/// Download (cached) and extract `wordpress.org/latest.tar.gz` into `docroot`.
pub fn extract_wordpress_core_via_curl(docroot: &Path) -> Result<(), String> {
    let doc_s = docroot
        .to_str()
        .ok_or_else(|| "Document root path is not valid UTF-8".to_string())?;
    fs::create_dir_all(docroot).map_err(|e| format!("Could not create docroot: {e}"))?;
    let cache_dir = paths::join_data("cache");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("Could not create cache dir: {e}"))?;
    let tarball = cache_dir.join("wordpress-latest.tar.gz");
    let tarball_s = tarball
        .to_str()
        .ok_or_else(|| "Cache path is not valid UTF-8".to_string())?;
    let need_download = !tarball.is_file()
        || fs::metadata(&tarball)
            .map(|m| m.len() < 1_000_000)
            .unwrap_or(true);
    if need_download {
        let tmp = cache_dir.join("wordpress-latest.tar.gz.tmp");
        let tmp_s = tmp
            .to_str()
            .ok_or_else(|| "Temp cache path is not valid UTF-8".to_string())?;
        let status = Command::new("curl")
            .args([
                "-fL",
                "--connect-timeout",
                "30",
                "--max-time",
                "600",
                "-o",
                tmp_s,
                WP_CORE_TARBALL_URL,
            ])
            .status()
            .map_err(|e| format!("curl not available: {e}"))?;
        if !status.success() {
            let _ = fs::remove_file(&tmp);
            return Err("curl could not download wordpress.org/latest.tar.gz".into());
        }
        fs::rename(&tmp, &tarball).map_err(|e| format!("Could not store core tarball: {e}"))?;
    }
    let status = Command::new("tar")
        .args(["-xzf", tarball_s, "--strip-components=1", "-C", doc_s])
        .status()
        .map_err(|e| format!("tar not available: {e}"))?;
    if !status.success() {
        return Err("tar could not extract WordPress core tarball".into());
    }
    if !docroot.join("wp-load.php").is_file() {
        return Err("WordPress core extract did not produce wp-load.php".into());
    }
    Ok(())
}
