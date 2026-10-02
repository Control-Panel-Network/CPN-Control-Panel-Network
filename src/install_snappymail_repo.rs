//! Local SnappyMail / Tachyon package/core repository fallback when upstream is unreachable.
//!
//! SnappyMail: upstream `https://snappymail.eu/repository/v2/` often times out.
//! Tachyon: Admin About `DoAdminInfo()` calls GitHub Releases first; a hang or
//! failed outbound HTTPS leaves load averages at 0, an empty PHP table, and
//! "Cannot access the repository", and can stall panel proxy workers until the
//! browser sees ERR_CONNECTION_RESET.
//!
//! CPN ships local JSON stubs and short-circuits Repository HTTP so admin Json
//! returns quickly without outbound HTTPS.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LOCAL_REPO_DIR: &str = "/var/lib/cpn-webmail/snappy-repo/v2";
const MARKER: &str = "CPN_LOCAL_REPO_FALLBACK";

/// Write stub packages/core JSON and patch lineage Repository HTTP to prefer them.
pub fn ensure_snappymail_repo_fallback(docroot: &str) -> Result<(), String> {
    let is_snappy = docroot.contains("snappymail");
    let is_tachyon = docroot.contains("tachyon");
    if !is_snappy && !is_tachyon {
        return Ok(());
    }
    fs::create_dir_all(LOCAL_REPO_DIR).map_err(|e| format!("snappy-repo dir: {e}"))?;

    let packages = Path::new(LOCAL_REPO_DIR).join("packages.json");
    if !packages.is_file() {
        fs::write(&packages, "[]\n").map_err(|e| format!("packages.json: {e}"))?;
    }

    if is_snappy {
        let version = detect_app_version(docroot, "snappymail").unwrap_or_else(|| "2.38.2".into());
        let core = Path::new(LOCAL_REPO_DIR).join("core.json");
        let core_body =
            format!("{{\n  \"version\": \"{version}\",\n  \"file\": \"\",\n  \"warnings\": []\n}}\n");
        fs::write(&core, core_body).map_err(|e| format!("core.json: {e}"))?;
    }

    if is_tachyon {
        let version = detect_app_version(docroot, "tachyon").unwrap_or_else(|| "4.2.3".into());
        // Minimal GitHub Releases API shape for Tachyon::Repository::getLatestCoreInfo().
        let github = Path::new(LOCAL_REPO_DIR).join("core-github.json");
        let github_body = format!(
            "{{\n  \"tag_name\": \"v{version}\",\n  \"assets\": [\n    {{\n      \"name\": \"tachyon-{version}.tar.gz\",\n      \"browser_download_url\": \"\"\n    }}\n  ]\n}}\n"
        );
        fs::write(&github, github_body).map_err(|e| format!("core-github.json: {e}"))?;
    }

    let _ = Command::new("bash")
        .args([
            "-c",
            "chown -R cpn-webmail:cpn-webmail /var/lib/cpn-webmail/snappy-repo 2>/dev/null || true; \
             chmod -R u=rwX,g=rX,o= /var/lib/cpn-webmail/snappy-repo 2>/dev/null || true",
        ])
        .status();

    for repo_php in find_repository_php(docroot) {
        if is_tachyon_repository(&repo_php) {
            patch_tachyon_http_get(&repo_php)?;
        } else {
            patch_repository_get(&repo_php)?;
        }
        shorten_repository_http_timeout(&repo_php)?;
    }
    Ok(())
}

fn detect_app_version(docroot: &str, app_dir: &str) -> Option<String> {
    let versions = Path::new(docroot).join(app_dir).join("v");
    let mut best: Option<String> = None;
    let Ok(entries) = fs::read_dir(&versions) else {
        return None;
    };
    for ent in entries.flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if name
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            match &best {
                None => best = Some(name),
                Some(cur) if version_gt(&name, cur) => best = Some(name),
                _ => {}
            }
        }
    }
    best
}

fn version_gt(a: &str, b: &str) -> bool {
    let pa = parse_ver(a);
    let pb = parse_ver(b);
    pa > pb
}

fn parse_ver(s: &str) -> Vec<u32> {
    s.split('.')
        .filter_map(|p| {
            let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().ok()
        })
        .collect()
}

fn is_tachyon_repository(path: &Path) -> bool {
    path.to_string_lossy().contains("tachyon_util")
}

fn find_repository_php(docroot: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // SnappyMail: snappymail/v/{ver}/app/libraries/snappymail/repository.php
    push_repo_candidates(docroot, "snappymail", &["snappymail"], &mut out);
    // Tachyon: tachyon/v/{ver}/app/libraries/tachyon_util/repository.php
    push_repo_candidates(docroot, "tachyon", &["tachyon_util", "snappymail"], &mut out);
    out
}

fn push_repo_candidates(docroot: &str, app_dir: &str, lib_names: &[&str], out: &mut Vec<PathBuf>) {
    let root = Path::new(docroot).join(app_dir).join("v");
    let Ok(versions) = fs::read_dir(&root) else {
        return;
    };
    for ent in versions.flatten() {
        for lib in lib_names {
            let candidate = ent
                .path()
                .join("app")
                .join("libraries")
                .join(lib)
                .join("repository.php");
            if candidate.is_file() && !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }
}

fn patch_repository_get(path: &Path) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if raw.contains(MARKER) {
        return Ok(());
    }
    let needle = "private static function get(string $path) : string\n\t{\n";
    let alt = "private static function get(string $path) : string\n\t{\r\n";
    let (found, use_needle) = if raw.contains(needle) {
        (true, needle)
    } else if raw.contains(alt) {
        (true, alt)
    } else if let Some(idx) = raw.find("private static function get(string $path)") {
        let after = &raw[idx..];
        if let Some(brace) = after.find('{') {
            let insert_at = idx + brace + 1;
            let patched = format!(
                "{}{}{}",
                &raw[..insert_at],
                local_repo_snippet(),
                &raw[insert_at..]
            );
            fs::write(path, patched).map_err(|e| format!("write {}: {e}", path.display()))?;
            return Ok(());
        }
        return Err(format!(
            "could not locate Repository::get opening brace in {}",
            path.display()
        ));
    } else {
        (false, needle)
    };
    if !found {
        return Err(format!("Repository::get not found in {}", path.display()));
    }
    let patched = raw.replacen(
        use_needle,
        &format!("{use_needle}{}", local_repo_snippet()),
        1,
    );
    fs::write(path, patched).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

/// Tachyon uses httpGet($url) against GitHub / raw.githubusercontent, not get($path).
fn patch_tachyon_http_get(path: &Path) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if raw.contains(MARKER) {
        return Ok(());
    }
    let sig = "private static function httpGet(string $url, array $headers = []) : ?string";
    let Some(idx) = raw.find(sig) else {
        return Err(format!(
            "Tachyon Repository::httpGet not found in {}",
            path.display()
        ));
    };
    let after = &raw[idx..];
    let Some(brace) = after.find('{') else {
        return Err(format!(
            "could not locate httpGet opening brace in {}",
            path.display()
        ));
    };
    let insert_at = idx + brace + 1;
    let patched = format!(
        "{}{}{}",
        &raw[..insert_at],
        tachyon_http_get_snippet(),
        &raw[insert_at..]
    );
    fs::write(path, patched).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

fn local_repo_snippet() -> String {
    format!(
        "\t\t// {MARKER}: prefer local stub when snappymail.eu is unreachable\n\
         \t\t$cpnLocal = '{LOCAL_REPO_DIR}/' . \\basename($path);\n\
         \t\tif (\\is_readable($cpnLocal)) {{\n\
         \t\t\t$cpnBody = \\file_get_contents($cpnLocal);\n\
         \t\t\tif (false !== $cpnBody) {{\n\
         \t\t\t\treturn $cpnBody;\n\
         \t\t\t}}\n\
         \t\t}}\n"
    )
}

fn tachyon_http_get_snippet() -> String {
    format!(
        "\n\t\t// {MARKER}: prefer local stub when GitHub/raw is slow or blocked\n\
         \t\tif (false !== \\strpos($url, 'packages.json')) {{\n\
         \t\t\t$cpnLocal = '{LOCAL_REPO_DIR}/packages.json';\n\
         \t\t\tif (\\is_readable($cpnLocal)) {{\n\
         \t\t\t\t$cpnBody = \\file_get_contents($cpnLocal);\n\
         \t\t\t\tif (false !== $cpnBody) {{\n\
         \t\t\t\t\treturn $cpnBody;\n\
         \t\t\t\t}}\n\
         \t\t\t}}\n\
         \t\t}}\n\
         \t\tif (false !== \\strpos($url, 'releases/latest')) {{\n\
         \t\t\t$cpnLocal = '{LOCAL_REPO_DIR}/core-github.json';\n\
         \t\t\tif (\\is_readable($cpnLocal)) {{\n\
         \t\t\t\t$cpnBody = \\file_get_contents($cpnLocal);\n\
         \t\t\t\tif (false !== $cpnBody) {{\n\
         \t\t\t\t\treturn $cpnBody;\n\
         \t\t\t\t}}\n\
         \t\t\t}}\n\
         \t\t}}\n"
    )
}

/// Keep outbound HTTP short so a missing local stub cannot hang the admin UI (~30s JS abort).
fn shorten_repository_http_timeout(path: &Path) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let updated = raw
        .replace("$oHTTP->timeout = 15;", "$oHTTP->timeout = 5;")
        .replace("$oHTTP->timeout = 15 ;", "$oHTTP->timeout = 5;");
    if updated != raw {
        fs::write(path, updated).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_mentions_marker_and_local_dir() {
        let s = local_repo_snippet();
        assert!(s.contains(MARKER));
        assert!(s.contains(LOCAL_REPO_DIR));
        assert!(s.contains("file_get_contents"));
    }

    #[test]
    fn tachyon_snippet_covers_github_and_packages() {
        let s = tachyon_http_get_snippet();
        assert!(s.contains(MARKER));
        assert!(s.contains("packages.json"));
        assert!(s.contains("core-github.json"));
        assert!(s.contains("releases/latest"));
    }

    #[test]
    fn version_compare_orders_semverish() {
        assert!(version_gt("2.38.2", "2.38.1"));
        assert!(version_gt("2.39.0", "2.38.9"));
        assert!(!version_gt("2.38.2", "2.38.2"));
        assert!(version_gt("4.2.3", "4.2.2"));
    }
}
