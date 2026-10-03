//! WordPress install orchestration (site, database, WP-CLI, registry).

use crate::account::now_unix;
use crate::panel_ops_db::create_database_with_user;
use crate::panel_ops_php::detect_php;
use crate::resource_accounts;
use crate::sites::{create_site, load_site, normalize_domain};
use crate::wordpress::{WordpressSite, get_wordpress_site, new_site_id, upsert_wordpress_site};
use crate::wordpress_core_fetch::extract_wordpress_core_via_curl;
use crate::wordpress_manage::refresh_wordpress_site;
use crate::wordpress_wpcli::{
    chown_docroot_to_web_user, ensure_wp_cli, is_wordpress_docroot, wp_run,
};
use rand::{Rng, distr::Alphanumeric};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_PLUGIN_ZIP_BYTES: usize = 40 * 1024 * 1024;
const MAX_PLUGIN_ZIP_COUNT: usize = 8;

#[derive(Debug, Clone)]
pub struct WordpressInstallRequest {
    pub domain: String,
    pub owner: String,
    pub title: String,
    pub admin_user: String,
    pub admin_password: String,
    pub admin_email: String,
    pub site_url: String,
    pub plugin_sources: Vec<String>,
    /// Absolute paths to uploaded plugin ZIP files (installed after core).
    pub plugin_zip_paths: Vec<PathBuf>,
    pub create_site_if_missing: bool,
}

#[derive(Debug, Clone)]
pub struct WordpressInstallResult {
    pub site: WordpressSite,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UploadedPluginZip {
    pub filename: String,
    pub bytes: Vec<u8>,
}

/// Persist uploaded plugin ZIPs under the CPN data tmp tree; returns absolute paths.
pub fn store_uploaded_plugin_zips(
    owner: &str,
    uploads: &[UploadedPluginZip],
) -> Result<Vec<PathBuf>, String> {
    if uploads.is_empty() {
        return Ok(Vec::new());
    }
    if uploads.len() > MAX_PLUGIN_ZIP_COUNT {
        return Err(format!(
            "Too many plugin ZIP uploads (max {MAX_PLUGIN_ZIP_COUNT})"
        ));
    }
    let owner_safe: String = owner
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Stage outside private /var/lib/cpn so site-user WP-CLI can read the ZIP.
    let dir = PathBuf::from("/var/tmp/cpn-wp-plugin-uploads")
        .join(format!("{owner_safe}-{}", now_unix()));
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create upload directory: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o755));
    }
    let mut out = Vec::new();
    for upload in uploads {
        let name = sanitize_zip_filename(&upload.filename)?;
        if upload.bytes.is_empty() {
            return Err(format!("Plugin ZIP `{name}` is empty"));
        }
        if upload.bytes.len() > MAX_PLUGIN_ZIP_BYTES {
            return Err(format!(
                "Plugin ZIP `{name}` exceeds {} MB limit",
                MAX_PLUGIN_ZIP_BYTES / (1024 * 1024)
            ));
        }
        if !looks_like_zip(&upload.bytes) {
            return Err(format!("`{name}` is not a valid ZIP archive"));
        }
        let path = dir.join(&name);
        fs::write(&path, &upload.bytes)
            .map_err(|e| format!("Could not save plugin ZIP `{name}`: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
        }
        out.push(path);
    }
    if let Some(user) = crate::wordpress_wpcli::preferred_site_run_user() {
        let dir_s = dir.to_string_lossy().to_string();
        let spec = format!("{user}:{user}");
        let _ = std::process::Command::new("chown")
            .args(["-R", &spec, &dir_s])
            .status();
    }
    Ok(out)
}

fn sanitize_zip_filename(raw: &str) -> Result<String, String> {
    let base = Path::new(raw)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim();
    if base.is_empty() {
        return Err("Plugin ZIP filename is required".into());
    }
    let lower = base.to_ascii_lowercase();
    if !lower.ends_with(".zip") {
        return Err(format!("Plugin upload `{base}` must be a .zip file"));
    }
    if base.contains("..") || base.contains('/') || base.contains('\\') {
        return Err("Plugin ZIP filename is invalid".into());
    }
    let safe: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.len() < 5 {
        return Err("Plugin ZIP filename is too short".into());
    }
    Ok(safe)
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0x50 && bytes[1] == 0x4B
}

/// Split raw plugin input on comma, newline, semicolon, or whitespace.
/// Keeps slugs and absolute paths / http(s) URLs.
pub fn parse_plugin_sources(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in
        raw.split(|c: char| c == ',' || c == ';' || c == '\n' || c == '\r' || c.is_whitespace())
    {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_valid_plugin_source(trimmed) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn is_valid_plugin_source(value: &str) -> bool {
    if value.starts_with('/') || value.starts_with("http://") || value.starts_with("https://") {
        return !value.chars().any(|c| c.is_control());
    }
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        && !value.is_empty()
}

fn random_db_password() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect()
}

fn db_ident_from_domain(domain: &str) -> String {
    let mut base: String = domain
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    while base.contains("__") {
        base = base.replace("__", "_");
    }
    base = base.trim_matches('_').to_string();
    if base.is_empty() {
        base = "site".into();
    }
    let name = format!("wp_{base}");
    if name.len() > 64 {
        name.chars().take(64).collect()
    } else {
        name
    }
}

fn default_site_url(domain: &str) -> String {
    format!("https://{domain}")
}

fn remove_placeholder_index(docroot: &Path) -> Result<bool, String> {
    let index = docroot.join("index.html");
    if !index.is_file() {
        return Ok(false);
    }
    fs::remove_file(&index).map_err(|e| format!("Could not remove {}: {e}", index.display()))?;
    Ok(true)
}

/// Prefer curl+tar for core files (often seconds). Fall back to WP-CLI download.
fn wp_core_download(docroot: &Path) -> Result<(), String> {
    match extract_wordpress_core_via_curl(docroot) {
        Ok(()) => Ok(()),
        Err(curl_err) => wp_run(docroot, &["core", "download", "--force"])
            .map(|_| ())
            .map_err(|wp_err| {
                format!("WordPress core download failed (curl: {curl_err}; wp-cli: {wp_err})")
            }),
    }
}

fn wp_config_create(
    docroot: &Path,
    db_name: &str,
    db_user: &str,
    db_pass: &str,
) -> Result<(), String> {
    wp_run(
        docroot,
        &[
            "config",
            "create",
            "--dbname",
            db_name,
            "--dbuser",
            db_user,
            "--dbpass",
            db_pass,
            "--dbhost",
            "localhost",
            "--skip-check",
        ],
    )?;
    Ok(())
}

fn wp_core_install(
    docroot: &Path,
    url: &str,
    title: &str,
    admin_user: &str,
    admin_password: &str,
    admin_email: &str,
) -> Result<(), String> {
    wp_run(
        docroot,
        &[
            "core",
            "install",
            "--url",
            url,
            "--title",
            title,
            "--admin_user",
            admin_user,
            "--admin_password",
            admin_password,
            "--admin_email",
            admin_email,
            "--skip-email",
        ],
    )?;
    Ok(())
}

fn wp_plugin_install_activate(docroot: &Path, source: &str) -> Result<(), String> {
    wp_run(
        docroot,
        &["plugin", "install", source, "--activate", "--force"],
    )?;
    Ok(())
}

/// Full WordPress install for a CPN site docroot.
pub fn install_wordpress(req: WordpressInstallRequest) -> Result<WordpressInstallResult, String> {
    let mut notes = Vec::new();
    let domain = normalize_domain(&req.domain)?;
    if get_wordpress_site(&domain).is_some() {
        return Err(format!("WordPress is already registered for `{domain}`"));
    }
    let owner = req.owner.trim();
    if owner.is_empty() {
        return Err("Owner is required".into());
    }
    let title = req.title.trim();
    if title.is_empty() {
        return Err("Site title is required".into());
    }
    let admin_user = req.admin_user.trim();
    if admin_user.is_empty() {
        return Err("Admin username is required".into());
    }
    if req.admin_password.trim().len() < 8 {
        return Err("Admin password must be at least 8 characters".into());
    }
    let admin_email = req.admin_email.trim();
    if admin_email.is_empty() || !admin_email.contains('@') {
        return Err("A valid admin email is required".into());
    }
    let site_url = {
        let raw = req.site_url.trim();
        if raw.is_empty() {
            default_site_url(&domain)
        } else {
            raw.to_string()
        }
    };

    let _wpcli = ensure_wp_cli()?;
    notes.push("WP-CLI is ready.".into());

    let site_record = match load_site(&domain) {
        Ok(record) => record,
        Err(_) if req.create_site_if_missing => {
            let created = create_site(&domain, owner, None, None, Some("WordPress install"))?;
            notes.push(format!("Created CPN site `{domain}`."));
            created
        }
        Err(error) => return Err(error),
    };

    if site_record.owner != owner && !crate::packages::is_panel_admin(owner) {
        return Err(format!(
            "Site `{domain}` is owned by `{}`, not `{owner}`",
            site_record.owner
        ));
    }

    let docroot = std::path::PathBuf::from(&site_record.docroot);
    if is_wordpress_docroot(&docroot) {
        return Err(format!(
            "Document root already contains WordPress files at {}",
            docroot.display()
        ));
    }

    if remove_placeholder_index(&docroot)? {
        notes.push("Removed placeholder index.html.".into());
    }

    match chown_docroot_to_web_user(&docroot) {
        Ok(user) => notes.push(format!(
            "Set document root ownership to `{user}` before WordPress install."
        )),
        Err(error) => notes.push(format!("Ownership note: {error}")),
    }

    let db_name = db_ident_from_domain(&domain);
    let db_user = db_name.clone();
    let db_password = random_db_password();

    create_database_with_user(&db_name, &db_user, &db_password)?;
    notes.push(format!(
        "Created MariaDB database `{db_name}` with dedicated user."
    ));

    // Partial retries often leave a registry row after MariaDB IF NOT EXISTS succeeds.
    match resource_accounts::create_database(owner, &db_name, &domain) {
        Ok(_) => notes.push("Registered database in CPN resource registry.".into()),
        Err(error) if error.to_ascii_lowercase().contains("already exists") => {
            notes.push(format!(
                "Reusing existing CPN database registry entry `{db_name}`."
            ));
        }
        Err(error) => return Err(error),
    }

    wp_core_download(&docroot)?;
    notes.push("Downloaded WordPress core.".into());

    if let Ok(user) = chown_docroot_to_web_user(&docroot) {
        notes.push(format!(
            "Confirmed document root ownership as `{user}` after download."
        ));
    }

    wp_config_create(&docroot, &db_name, &db_user, &db_password)?;
    notes.push("Created wp-config.php.".into());

    wp_core_install(
        &docroot,
        &site_url,
        title,
        admin_user,
        req.admin_password.trim(),
        admin_email,
    )?;
    notes.push("Ran wp core install.".into());

    for source in &req.plugin_sources {
        match wp_plugin_install_activate(&docroot, source) {
            Ok(()) => notes.push(format!("Installed plugin `{source}`.")),
            Err(error) => notes.push(format!("Plugin `{source}` failed: {error}")),
        }
    }

    for zip_path in &req.plugin_zip_paths {
        let label = zip_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin.zip");
        let path_s = zip_path.display().to_string();
        match wp_plugin_install_activate(&docroot, &path_s) {
            Ok(()) => notes.push(format!("Installed uploaded plugin ZIP `{label}`.")),
            Err(error) => notes.push(format!("Uploaded plugin ZIP `{label}` failed: {error}")),
        }
    }
    if let Some(first) = req.plugin_zip_paths.first()
        && let Some(parent) = first.parent()
        && parent.to_string_lossy().contains("cpn-wp-plugin-uploads")
    {
        let _ = fs::remove_dir_all(parent);
    }

    let php_version = detect_php().version.unwrap_or_else(|| "-".into());

    let now = now_unix();
    let site = WordpressSite {
        id: new_site_id(),
        domain: domain.clone(),
        title: title.to_string(),
        docroot: site_record.docroot.clone(),
        site_url: site_url.clone(),
        admin_user: admin_user.to_string(),
        db_name: db_name.clone(),
        db_user: db_user.clone(),
        owner: owner.to_string(),
        wp_version: String::new(),
        php_version,
        theme: String::new(),
        plugin_count: 0,
        search_indexing: true,
        debugging: false,
        maintenance: false,
        password_protection: false,
        created_at_unix: now,
        updated_at_unix: now,
    };
    upsert_wordpress_site(site)?;

    let refreshed = refresh_wordpress_site(&domain)?;
    let mut site = refreshed.site;
    site.title = title.to_string();
    site.site_url = site_url;
    site.admin_user = admin_user.to_string();
    site.owner = owner.to_string();
    site.db_name = db_name;
    site.db_user = db_user;
    site.created_at_unix = now;
    site.updated_at_unix = now_unix();

    let saved = upsert_wordpress_site(site)?;
    notes.push(format!(
        "Registered WordPress site `{domain}` (passwords are not stored in the registry)."
    ));
    notes.extend(refreshed.notes);

    Ok(WordpressInstallResult { site: saved, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plugin_sources_splits_and_filters() {
        let raw = "akismet, https://example.com/a.zip\nhello-dolly; bad\x07slug /local/pkg.zip";
        let list = parse_plugin_sources(raw);
        assert!(list.contains(&"akismet".to_string()));
        assert!(list.contains(&"https://example.com/a.zip".to_string()));
        assert!(list.contains(&"hello-dolly".to_string()));
        assert!(list.contains(&"/local/pkg.zip".to_string()));
        assert!(!list.iter().any(|s| s.contains('\x07')));
    }

    #[test]
    fn db_ident_from_domain_sanitizes() {
        assert!(db_ident_from_domain("blog.example.com").starts_with("wp_"));
    }

    #[test]
    fn sanitize_zip_filename_accepts_plugin_zip() {
        let name = sanitize_zip_filename("oceanwatcher-maintenance-mode-0.5.0-beta.6.zip")
            .expect("zip name");
        assert!(name.ends_with(".zip"));
        assert!(looks_like_zip(&[0x50, 0x4B, 0x03, 0x04]));
    }
}
