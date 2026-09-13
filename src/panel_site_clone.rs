//! Clone / staging: copy site files to a new registered site under the parent home.

use crate::sites::{
    SiteRecord, create_site, hosting_home_root, load_site, normalize_domain, resolve_parent_domain,
    site_home_from_record,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn path_under_home(path: &Path) -> Result<(), String> {
    let home = hosting_home_root();
    let canon_home = fs::canonicalize(&home).unwrap_or(home.clone());
    let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !canon.starts_with(&canon_home) && !path.starts_with(&home) {
        return Err("Path escapes hosting home".into());
    }
    Ok(())
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<u64, String> {
    path_under_home(src)?;
    path_under_home(dst)?;
    if !src.is_dir() {
        return Err(format!("Source missing: {}", src.display()));
    }
    fs::create_dir_all(dst).map_err(|e| format!("Could not create {}: {e}", dst.display()))?;

    // Prefer cp -a on Unix for speed and mode preservation.
    #[cfg(unix)]
    {
        let status = Command::new("cp")
            .arg("-a")
            .arg(format!("{}/.", src.display()))
            .arg(dst)
            .status()
            .map_err(|e| format!("cp failed: {e}"))?;
        if !status.success() {
            return Err("cp -a failed while cloning site files".into());
        }
        return Ok(approx_file_count(dst));
    }

    #[cfg(not(unix))]
    {
        copy_recursive(src, dst)?;
        Ok(approx_file_count(dst))
    }
}

#[cfg(not(unix))]
fn copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| format!("read_dir: {e}"))? {
        let entry = entry.map_err(|e| format!("read_dir entry: {e}"))?;
        let ty = entry.file_type().map_err(|e| format!("file_type: {e}"))?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            fs::create_dir_all(&to).map_err(|e| format!("mkdir: {e}"))?;
            copy_recursive(&entry.path(), &to)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &to).map_err(|e| format!("copy: {e}"))?;
        }
    }
    Ok(())
}

fn approx_file_count(root: &Path) -> u64 {
    let mut n = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_dir() {
                stack.push(entry.path());
            } else {
                n += 1;
            }
        }
        if n > 50_000 {
            break;
        }
    }
    n
}

/// Build staging FQDN: `staging.<domain>` when valid, else `staging-<label>.<parent>`.
pub fn suggest_staging_domain(source: &SiteRecord) -> Result<String, String> {
    let candidate = format!("staging.{}", source.domain);
    match normalize_domain(&candidate) {
        Ok(d) => Ok(d),
        Err(_) => {
            let parent =
                resolve_parent_domain(&source.domain)?.unwrap_or_else(|| source.domain.clone());
            let label = source
                .domain
                .split('.')
                .next()
                .unwrap_or("site")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect::<String>();
            normalize_domain(&format!("staging-{label}.{parent}"))
        }
    }
}

fn resolve_target_domain(
    source: &SiteRecord,
    target_raw: &str,
    use_staging: bool,
) -> Result<String, String> {
    let trimmed = target_raw.trim();
    if use_staging && trimmed.is_empty() {
        return suggest_staging_domain(source);
    }
    if trimmed.is_empty() {
        return Err("Target domain or subdomain label is required".into());
    }
    // Short label: treat as subdomain of parent (or of source when source is primary).
    if !trimmed.contains('.') {
        let label = trimmed.to_ascii_lowercase();
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            || label.starts_with('-')
            || label.ends_with('-')
        {
            return Err("Subdomain label must be alphanumeric with optional hyphens".into());
        }
        let parent =
            resolve_parent_domain(&source.domain)?.unwrap_or_else(|| source.domain.clone());
        return normalize_domain(&format!("{label}.{parent}"));
    }
    normalize_domain(trimmed)
}

pub struct CloneResult {
    pub domain: String,
    pub docroot: String,
    pub files_copied: u64,
    pub note: String,
}

/// Create a new site registry entry and copy public_html files from the source.
pub fn clone_site_files(
    source: &SiteRecord,
    owner: &str,
    target_raw: &str,
    use_staging: bool,
) -> Result<CloneResult, String> {
    let target = resolve_target_domain(source, target_raw, use_staging)?;
    if target.eq_ignore_ascii_case(&source.domain) {
        return Err("Target domain must differ from the source".into());
    }
    if load_site(&target).is_ok() {
        return Err(format!("Site `{target}` already exists"));
    }

    let created = create_site(
        &target,
        owner,
        None,
        source.engine.as_deref(),
        Some(&format!(
            "Cloned files from {} (databases not copied)",
            source.domain
        )),
    )?;

    let src_doc = PathBuf::from(&source.docroot);
    let dst_doc = PathBuf::from(&created.docroot);
    // Remove placeholder index so clone content wins when present.
    let placeholder = dst_doc.join("index.html");
    if placeholder.is_file() {
        let _ = fs::remove_file(&placeholder);
    }

    let files_copied = copy_dir_contents(&src_doc, &dst_doc)?;

    // Best-effort: also copy common config sitting beside public_html (not nested sites).
    let src_home = site_home_from_record(source);
    let dst_home = site_home_from_record(&created);
    for name in [
        "wp-config.php",
        ".htaccess",
        "composer.json",
        "package.json",
    ] {
        let from = src_home.join(name);
        if from.is_file() {
            let to = dst_home.join(name);
            let _ = fs::copy(&from, &to);
        }
    }

    Ok(CloneResult {
        domain: created.domain,
        docroot: created.docroot,
        files_copied,
        note: "File clone finished. Databases are not copied; export/import separately if needed."
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::{now_unix, with_test_data_dir};
    use crate::sites::create_site;

    #[test]
    fn staging_suggestion_and_clone() {
        with_test_data_dir(|| {
            let home = std::env::temp_dir().join(format!(
                "cpn-clone-{}-{}",
                std::process::id(),
                now_unix()
            ));
            let _ = fs::remove_dir_all(&home);
            fs::create_dir_all(&home).unwrap();
            unsafe {
                std::env::set_var("CPN_SITES_HOME", &home);
            }
            let src = create_site("example.com", "Admin", None, None, None).unwrap();
            fs::write(Path::new(&src.docroot).join("hello.txt"), b"hi").unwrap();
            let sug = suggest_staging_domain(&src).unwrap();
            assert_eq!(sug, "staging.example.com");
            let out = clone_site_files(&src, "Admin", "", true).unwrap();
            assert_eq!(out.domain, "staging.example.com");
            assert!(Path::new(&out.docroot).join("hello.txt").is_file());
            unsafe {
                std::env::remove_var("CPN_SITES_HOME");
            }
            let _ = fs::remove_dir_all(&home);
        });
    }
}
