//! Tachyon release resolution and in-place upgrade.
//!
//! Install and heal both follow the newest stable upstream release (GitHub Releases API
//! for the Tachyon project) instead of a version baked into the binary. The pinned
//! release below is only the offline fallback used when the API is unreachable.
//!
//! Upgrades add a new `tachyon/v/<version>` directory next to the old one, refresh
//! `index.php`, and leave `include.php` (data folder outside the docroot) and all data
//! untouched. After a new version directory appears, the repository stubs, admin SSO hooks,
//! SELinux labels, and PHP opcode cache are re-applied; once the new version verifies, the
//! superseded version directories are removed (see `install_tachyon_prune`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

const DOCROOT: &str = "/opt/cpn-webmail/tachyon";
const LATEST_API: &str = "https://api.github.com/repos/kimusan/Tachyon/releases/latest";
const DOWNLOAD_PREFIX: &str = "https://github.com/kimusan/Tachyon/releases/download/";
const STAMP_PATH: &str = "/var/lib/cpn/tachyon-release-check.stamp";
const OPT_OUT_MARKER: &str = "/var/lib/cpn/tachyon-auto-upgrade.disabled";
/// Re-check upstream at most this often after a successful check.
const CHECK_INTERVAL_SECS: u64 = 6 * 3600;
/// Retry sooner after a failed check (network, digest, extraction).
const RETRY_INTERVAL_SECS: u64 = 3600;

/// Offline fallback: newest release known when this CPN build was cut.
pub const PINNED_VERSION: &str = "4.8.0";
pub const PINNED_SHA256: &str = "9535c65ff0b0d722bc8008cb925cb1047b1fb54874b3e874906b9eb029b8a655";

static UPGRADE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TachyonRelease {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

pub fn pinned_release() -> TachyonRelease {
    TachyonRelease {
        version: PINNED_VERSION.to_string(),
        url: format!("{DOWNLOAD_PREFIX}v{PINNED_VERSION}/tachyon-{PINNED_VERSION}.tar.gz"),
        sha256: PINNED_SHA256.to_string(),
    }
}

fn valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.split('.').count() >= 2
        && v.split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse the GitHub "latest release" JSON into a verified download descriptor.
///
/// Requires a stable (non draft, non prerelease) tag, the `tachyon-<ver>.tar.gz` asset, a
/// `sha256:` digest published by GitHub, and a download URL under the upstream project.
pub fn parse_latest_release(body: &str) -> Option<TachyonRelease> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if value
        .get("draft")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || value
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        return None;
    }
    let tag = value.get("tag_name")?.as_str()?.trim();
    let version = tag.trim_start_matches('v');
    if !valid_version(version) {
        return None;
    }
    let want = format!("tachyon-{version}.tar.gz");
    let asset = value
        .get("assets")?
        .as_array()?
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(want.as_str()))?;
    let url = asset.get("browser_download_url")?.as_str()?.trim();
    if !url.starts_with(DOWNLOAD_PREFIX) || url.contains("..") {
        return None;
    }
    let digest = asset.get("digest")?.as_str()?.trim();
    let sha = digest.strip_prefix("sha256:")?.to_ascii_lowercase();
    if !valid_sha256(&sha) {
        return None;
    }
    Some(TachyonRelease {
        version: version.to_string(),
        url: url.to_string(),
        sha256: sha,
    })
}

/// Query upstream for the newest stable release (None when unreachable or malformed).
pub fn fetch_latest_release() -> Option<TachyonRelease> {
    let out = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--max-time",
            "15",
            "--connect-timeout",
            "10",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: cpn-installer",
            LATEST_API,
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_latest_release(&String::from_utf8(out.stdout).ok()?)
}

/// Latest upstream release when reachable, otherwise the pinned fallback. The newer of the
/// two always wins so an old binary never downgrades the install target.
pub fn resolve_release_for_install() -> TachyonRelease {
    let pinned = pinned_release();
    match fetch_latest_release() {
        Some(latest)
            if crate::install_snappymail_repo::version_gt(&latest.version, &pinned.version) =>
        {
            latest
        }
        _ => pinned,
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn stamp_due() -> bool {
    let Ok(raw) = std::fs::read_to_string(STAMP_PATH) else {
        return true;
    };
    let mut parts = raw.split_whitespace();
    let at: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let interval: u64 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(CHECK_INTERVAL_SECS);
    now_secs().saturating_sub(at) >= interval
}

fn write_stamp(ok: bool) {
    let interval = if ok {
        CHECK_INTERVAL_SECS
    } else {
        RETRY_INTERVAL_SECS
    };
    if let Some(parent) = Path::new(STAMP_PATH).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(STAMP_PATH, format!("{} {}\n", now_secs(), interval));
}

/// Reject archive members that could escape the staging directory.
pub fn tar_listing_is_safe(listing: &str) -> bool {
    let mut any = false;
    for line in listing.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        any = true;
        if name.starts_with('/') || name.split('/').any(|seg| seg == "..") {
            return false;
        }
    }
    any
}

fn run(cmd: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{cmd} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn download_blocking(url: &str, dest: &str) -> Result<(), String> {
    run(
        "curl",
        &[
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "600",
            "--connect-timeout",
            "30",
            "--output",
            dest,
            url,
        ],
    )
}

struct StageDir(PathBuf);

impl Drop for StageDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn install_release_files(
    release: &TachyonRelease,
    archive: &str,
    stage: &Path,
) -> Result<(), String> {
    let ver = &release.version;
    let list = Command::new("tar")
        .args(["tzf", archive])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("tar list: {e}"))?;
    if !list.status.success() || !tar_listing_is_safe(&String::from_utf8_lossy(&list.stdout)) {
        return Err("Tachyon archive listing rejected".into());
    }
    let stage_s = stage.to_string_lossy().to_string();
    run("tar", &["xzf", archive, "--no-same-owner", "-C", &stage_s])?;

    let src_ver = stage.join("tachyon/v").join(ver);
    if !src_ver.join("include.php").is_file() || !stage.join("index.php").is_file() {
        return Err(format!("Tachyon {ver} archive is missing expected files"));
    }
    let v_root = Path::new(DOCROOT).join("tachyon/v");
    let final_dir = v_root.join(ver);
    let tmp_dir = v_root.join(format!(".{ver}.cpn-new"));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    run(
        "cp",
        &["-a", &src_ver.to_string_lossy(), &tmp_dir.to_string_lossy()],
    )?;
    let tmp_s = tmp_dir.to_string_lossy().to_string();
    let perms = run("chown", &["-R", "root:root", &tmp_s])
        .and_then(|_| run("chmod", &["-R", "u=rwX,g=rX,o=rX", &tmp_s]));
    if let Err(e) = perms {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }
    if final_dir.exists() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(format!("{} already exists", final_dir.display()));
    }
    std::fs::rename(&tmp_dir, &final_dir).map_err(|e| {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        format!("activate {}: {e}", final_dir.display())
    })?;
    // Entry points last, after the version directory they select is in place.
    // include.php (APP_DATA_FOLDER_PATH) and the data/ tree are intentionally not touched.
    for name in ["index.php", "_include.php", ".htaccess"] {
        let from = stage.join(name);
        if from.is_file() {
            let to = Path::new(DOCROOT).join(name);
            run(
                "install",
                &[
                    "-m",
                    "644",
                    "-o",
                    "root",
                    "-g",
                    "root",
                    &from.to_string_lossy(),
                    &to.to_string_lossy(),
                ],
            )?;
        }
    }
    Ok(())
}

fn reapply_after_new_version() {
    let _ = crate::install_snappymail_repo::ensure_snappymail_repo_fallback(DOCROOT);
    let _ = crate::install_snappymail_i18n::ensure_notification_count_placeholders();
    crate::install_selinux_mail::ensure_webmail_selinux();
    // Admin SSO hooks live in the new version directory's ActionsAdmin.php.
    let _ = crate::tachyon_admin_bridge_install::ensure_admin_bridge();
    let _ = Command::new("systemctl")
        .args(["reload", "php-fpm"])
        .status();
}

/// Upgrade an installed Tachyon to the newest stable upstream release.
///
/// Throttled (6 hours after success, 1 hour after failure) unless `force`. Skips hosts
/// without Tachyon or with the operator opt-out marker. Returns a short status line.
pub fn upgrade_tachyon_if_needed(force: bool) -> Result<String, String> {
    if !cfg!(unix) || !Path::new(DOCROOT).join("tachyon/v").is_dir() {
        return Ok("skipped: Tachyon not installed".into());
    }
    if Path::new(OPT_OUT_MARKER).is_file() {
        return Ok("skipped: auto-upgrade disabled by operator marker".into());
    }
    let Ok(_guard) = UPGRADE_LOCK.try_lock() else {
        return Ok("skipped: upgrade already running".into());
    };
    if !force && !stamp_due() {
        return Ok("skipped: checked recently".into());
    }
    let result = upgrade_locked();
    write_stamp(result.is_ok());
    result
}

fn upgrade_locked() -> Result<String, String> {
    let installed = crate::install_snappymail_repo::detect_app_version(DOCROOT, "tachyon")
        .unwrap_or_else(|| "0.0.0".into());
    let Some(release) = fetch_latest_release() else {
        return Err(
            "Tachyon release check failed (GitHub unreachable or no verified asset)".into(),
        );
    };
    if !crate::install_snappymail_repo::version_gt(&release.version, &installed) {
        return Ok(format!("Tachyon {installed} is current"));
    }

    let area = crate::install_webmail::EphemeralDownload::create("tachyon-upgrade.tar.gz")?;
    let archive = area.as_str().to_string();
    download_blocking(&release.url, &archive)?;
    crate::install_webmail::verify_sha256(&archive, &release.sha256)?;

    let stage = PathBuf::from(format!(
        "/var/tmp/cpn-tachyon-stage-{}-{}",
        std::process::id(),
        now_secs()
    ));
    std::fs::create_dir_all(&stage).map_err(|e| format!("stage dir: {e}"))?;
    let _stage_guard = StageDir(stage.clone());
    install_release_files(&release, &archive, &stage)?;
    reapply_after_new_version();
    // Cutover is done (hooks, labels, PHP-FPM reload): drop the superseded directories once the
    // new version verifies. A failed cleanup never fails the upgrade; the next heal retries.
    let cleanup = match crate::install_tachyon_prune::prune_old_versions() {
        Ok(msg) => msg,
        Err(e) => format!("cleanup deferred: {e}"),
    };
    Ok(format!(
        "Tachyon upgraded {installed} -> {} ({cleanup})",
        release.version
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(prerelease: bool, digest: &str, url: &str) -> String {
        format!(
            r#"{{"tag_name":"v4.9.1","draft":false,"prerelease":{prerelease},"assets":[
              {{"name":"tachyon-4.9.1.zip","browser_download_url":"{url}z","digest":"{digest}"}},
              {{"name":"tachyon-4.9.1.tar.gz","browser_download_url":"{url}","digest":"{digest}"}}]}}"#
        )
    }

    const GOOD_URL: &str =
        "https://github.com/kimusan/Tachyon/releases/download/v4.9.1/tachyon-4.9.1.tar.gz";

    #[test]
    fn parses_stable_release_with_digest() {
        let digest = format!("sha256:{}", "ab".repeat(32));
        let rel = parse_latest_release(&sample(false, &digest, GOOD_URL)).expect("release");
        assert_eq!(rel.version, "4.9.1");
        assert_eq!(rel.url, GOOD_URL);
        assert_eq!(rel.sha256, "ab".repeat(32));
    }

    #[test]
    fn rejects_prerelease_missing_digest_and_foreign_url() {
        let digest = format!("sha256:{}", "ab".repeat(32));
        assert!(parse_latest_release(&sample(true, &digest, GOOD_URL)).is_none());
        assert!(parse_latest_release(&sample(false, "sha256:zz", GOOD_URL)).is_none());
        assert!(parse_latest_release(&sample(false, "md5:abc", GOOD_URL)).is_none());
        assert!(
            parse_latest_release(&sample(false, &digest, "https://example.invalid/t.tar.gz"))
                .is_none()
        );
        assert!(parse_latest_release("not json").is_none());
    }

    #[test]
    fn rejects_bad_tags() {
        let body = r#"{"tag_name":"v4.9.1; rm","assets":[]}"#;
        assert!(parse_latest_release(body).is_none());
        assert!(valid_version("4.8.0"));
        assert!(!valid_version("4"));
        assert!(!valid_version("4..1"));
    }

    #[test]
    fn pinned_release_is_well_formed() {
        let p = pinned_release();
        assert!(valid_version(&p.version));
        assert!(valid_sha256(&p.sha256));
        assert!(p.url.starts_with(DOWNLOAD_PREFIX));
        assert!(p.url.ends_with(&format!("tachyon-{}.tar.gz", p.version)));
    }

    #[test]
    fn tar_listing_guard() {
        assert!(tar_listing_is_safe(
            "tachyon/v/4.8.0/include.php\nindex.php\n"
        ));
        assert!(!tar_listing_is_safe("../etc/passwd\n"));
        assert!(!tar_listing_is_safe("tachyon/../../x\n"));
        assert!(!tar_listing_is_safe("/etc/passwd\n"));
        assert!(!tar_listing_is_safe(""));
    }
}
