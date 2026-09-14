//! Shared SnappyMail-family (SnappyMail / Tachyon / NextSnapMail) data roots and admin sync.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Form value for Email > Change Password when targeting webmail admin (not a mailbox).
pub const WEBMAIL_ADMIN_CHOICE: &str = "__webmail_admin__";

const KNOWN_DATA_NAMES: &[&str] = &["snappymail", "tachyon", "nextsnapmail"];

/// Panel-proxied install roots that use SnappyMail-lineage `Actions.php` defaults.
const KNOWN_DOCROOTS: &[&str] = &[
    "/opt/cpn-webmail/snappymail",
    "/opt/cpn-webmail/tachyon",
    "/opt/nextcloud/apps/nextsnapmail",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageAdminIdentity {
    pub client_id: String,
    pub public_admin_path: String,
    pub login: String,
}

/// True when at least one SnappyMail-family client or data tree is present.
pub fn any_lineage_installed() -> bool {
    if Path::new("/opt/cpn-webmail/snappymail/index.php").is_file() {
        return true;
    }
    if Path::new("/opt/cpn-webmail/tachyon/index.php").is_file() {
        return true;
    }
    if crate::apps_nextcloud::nextsnapmail_app_present() {
        return true;
    }
    lineage_data_dirs().iter().any(|d| Path::new(d).is_dir())
}

/// Application data directories for every installed SnappyMail-family client.
///
/// Prefers `/var/lib/cpn-webmail/{snappymail,tachyon,nextsnapmail}/` and discovers any
/// sibling tree that already has `application.ini`. NextSnapMail under Nextcloud is
/// included when its RainLoop-style data root is present.
pub fn lineage_data_dirs() -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    for name in KNOWN_DATA_NAMES {
        let p = PathBuf::from(format!("/var/lib/cpn-webmail/{name}"));
        push_data_dir(&mut dirs, &p);
    }
    if let Ok(rd) = std::fs::read_dir("/var/lib/cpn-webmail") {
        for ent in rd.flatten() {
            let p = ent.path();
            if !p.is_dir() {
                continue;
            }
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if name == "snappy-repo" || name == "current" {
                continue;
            }
            if has_application_ini(&p) {
                push_data_dir(&mut dirs, &p);
            }
        }
    }
    for extra in discover_nextsnapmail_data_dirs() {
        push_data_dir(&mut dirs, &extra);
    }
    dirs
}

/// Docroots that may contain `libraries/RainLoop/Actions.php` (or Tachyon equivalent).
pub fn lineage_docroots() -> Vec<String> {
    let mut roots = Vec::new();
    for root in KNOWN_DOCROOTS {
        let path = Path::new(root);
        if path.is_dir() {
            roots.push((*root).to_string());
        }
    }
    roots
}

fn discover_nextsnapmail_data_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !crate::apps_nextcloud::nextsnapmail_app_present() {
        return out;
    }
    let nc_data = Path::new("/opt/nextcloud/data");
    if !nc_data.is_dir() {
        return out;
    }
    // Shallow scan: appdata_*/nextsnapmail and nextsnapmail/_data_
    if let Ok(rd) = std::fs::read_dir(nc_data) {
        for ent in rd.flatten() {
            let p = ent.path();
            if !p.is_dir() {
                continue;
            }
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if name.starts_with("appdata_") {
                let candidate = p.join("nextsnapmail");
                if has_application_ini(&candidate) {
                    out.push(candidate);
                }
            }
            if name == "nextsnapmail" && has_application_ini(&p) {
                out.push(p);
            }
        }
    }
    let app_data = Path::new("/opt/nextcloud/apps/nextsnapmail/data");
    if has_application_ini(app_data) {
        out.push(app_data.to_path_buf());
    }
    out
}

fn push_data_dir(dirs: &mut Vec<String>, path: &Path) {
    if !path.is_dir() {
        return;
    }
    let mut s = path.to_string_lossy().to_string();
    if !s.ends_with('/') {
        s.push('/');
    }
    if !dirs.iter().any(|d| d == &s) {
        dirs.push(s);
    }
}

fn has_application_ini(data_dir: &Path) -> bool {
    data_dir
        .join("_data_/_default_/configs/application.ini")
        .is_file()
}

pub fn application_ini_path(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join("_data_/_default_/configs/application.ini")
}

/// Read `admin_login` from application.ini (live value operators may change in webmail admin).
pub fn read_admin_login(data_dir: &str) -> Option<String> {
    let ini = application_ini_path(data_dir);
    if !ini.is_file() {
        return None;
    }
    let raw = std::fs::read_to_string(&ini).ok()?;
    parse_ini_quoted_value(&raw, "admin_login")
}

pub fn parse_ini_quoted_value(raw: &str, key: &str) -> Option<String> {
    let prefix_eq = format!("{key} =");
    let prefix_nospace = format!("{key}=");
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with(&prefix_eq) || trimmed.starts_with(&prefix_nospace)) {
            continue;
        }
        let Some((_, rhs)) = trimmed.split_once('=') else {
            continue;
        };
        let v = rhs.trim().trim_matches('"').trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    None
}

/// Map a data dir or docroot path to a stable client id (`snappymail`, `tachyon`, `nextsnapmail`).
pub fn client_id_for_data_dir(data_dir: &str) -> String {
    let lower = data_dir.to_ascii_lowercase();
    if lower.contains("tachyon") {
        "tachyon".into()
    } else if lower.contains("nextsnap") {
        "nextsnapmail".into()
    } else {
        "snappymail".into()
    }
}

fn public_admin_path_for(client_id: &str) -> String {
    match client_id {
        "tachyon" => "/tachyon/?admin".into(),
        "nextsnapmail" => "Nextcloud NextSnapMail admin".into(),
        _ => "/snappymail/?admin".into(),
    }
}

/// Live admin usernames from each installed client's application.ini.
pub fn list_admin_identities() -> Vec<LineageAdminIdentity> {
    let mut out = Vec::new();
    for data_dir in lineage_data_dirs() {
        let login = read_admin_login(&data_dir).unwrap_or_else(|| "admin".into());
        let client_id = client_id_for_data_dir(&data_dir);
        if out
            .iter()
            .any(|i: &LineageAdminIdentity| i.client_id == client_id)
        {
            continue;
        }
        out.push(LineageAdminIdentity {
            public_admin_path: public_admin_path_for(&client_id),
            client_id,
            login,
        });
    }
    // Installed docroots without data yet still count as admin targets.
    for root in lineage_docroots() {
        let client_id = client_id_for_data_dir(&root);
        if out.iter().any(|i| i.client_id == client_id) {
            continue;
        }
        if client_id == "nextsnapmail" && !crate::apps_nextcloud::nextsnapmail_app_present() {
            continue;
        }
        if client_id != "nextsnapmail" && !Path::new(&root).join("index.php").is_file() {
            continue;
        }
        out.push(LineageAdminIdentity {
            public_admin_path: public_admin_path_for(&client_id),
            client_id,
            login: "admin".into(),
        });
    }
    out
}

/// Dropdown / help label reflecting live admin_login values (not hardcoded `admin` only).
pub fn webmail_admin_option_label() -> String {
    let ids = list_admin_identities();
    if ids.is_empty() {
        return "Webmail admin".into();
    }
    let mut logins: Vec<String> = ids.iter().map(|i| i.login.clone()).collect();
    logins.sort();
    logins.dedup();
    if logins.len() == 1 {
        return format!("Webmail admin ({})", logins[0]);
    }
    let detail = ids
        .iter()
        .map(|i| format!("{}: {}", i.client_id, i.login))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Webmail admin ({detail})")
}

/// Public admin paths for UI copy (installed clients only).
pub fn webmail_admin_paths_summary() -> String {
    let ids = list_admin_identities();
    if ids.is_empty() {
        return "<code>/snappymail/?admin</code> / <code>/tachyon/?admin</code>".into();
    }
    ids.iter()
        .map(|i| {
            if i.public_admin_path.starts_with('/') {
                format!("<code>{}</code>", i.public_admin_path)
            } else {
                html_escape_basic(&i.public_admin_path)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn html_escape_basic(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Set bcrypt admin password on every installed SnappyMail-family application.ini.
pub fn sync_lineage_admin_password(password: &str) -> Result<u32, String> {
    if password.len() < 8 {
        return Err("Webmail admin password must be at least 8 characters".into());
    }
    let hash = php_password_hash(password)?;
    let mut synced = 0u32;
    for data_dir in lineage_data_dirs() {
        let ini = application_ini_path(&data_dir);
        if !ini.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
        let updated = replace_ini_quoted(&raw, "admin_password", &hash);
        if updated != raw {
            std::fs::write(&ini, updated).map_err(|e| e.to_string())?;
        }
        let txt = Path::new(&data_dir).join("_data_/_default_/admin_password.txt");
        if txt.is_file() {
            let _ = std::fs::remove_file(&txt);
        }
        synced += 1;
        let _ = chown_data_tree(&data_dir);
    }
    Ok(synced)
}

pub fn php_password_hash(password: &str) -> Result<String, String> {
    let output = Command::new("php")
        .args([
            "-r",
            "echo password_hash(getenv('CPN_SNAPPY_PASS'), PASSWORD_DEFAULT);",
        ])
        .env("CPN_SNAPPY_PASS", password)
        .output()
        .map_err(|e| format!("php password_hash failed to start: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "php password_hash failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !hash.starts_with("$2y$") && !hash.starts_with("$2a$") && !hash.starts_with("$argon") {
        return Err("php password_hash returned unexpected output".into());
    }
    Ok(hash)
}

pub fn replace_ini_line(raw: &str, key: &str, new_line: &str) -> String {
    let prefix_eq = format!("{key} =");
    let prefix_nospace = format!("{key}=");
    let mut out = String::with_capacity(raw.len() + new_line.len());
    let mut replaced = false;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&prefix_eq) || trimmed.starts_with(&prefix_nospace) {
            out.push_str(new_line);
            out.push('\n');
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(new_line);
        out.push('\n');
    }
    out
}

pub fn replace_ini_quoted(raw: &str, key: &str, value: &str) -> String {
    replace_ini_line(raw, key, &format!("{key} = \"{value}\""))
}

pub fn replace_ini_bool(raw: &str, key: &str, on: bool) -> String {
    let word = if on { "On" } else { "Off" };
    replace_ini_line(raw, key, &format!("{key} = {word}"))
}

pub fn chown_data_tree(data_dir: &str) -> Result<(), String> {
    let tree = Path::new(data_dir).join("_data_");
    if tree.is_dir() {
        let _ = Command::new("chown")
            .args([
                "-R",
                "cpn-webmail:cpn-webmail",
                tree.to_string_lossy().as_ref(),
            ])
            .status();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_admin_login() {
        let raw = "[security]\nadmin_login = \"kimops\"\nadmin_password = \"x\"\n";
        assert_eq!(
            parse_ini_quoted_value(raw, "admin_login").as_deref(),
            Some("kimops")
        );
    }

    #[test]
    fn replaces_admin_password_line() {
        let raw = "[security]\nadmin_login = \"admin\"\nadmin_password = \"old\"\n";
        let out = replace_ini_quoted(raw, "admin_password", "$2y$10$abc");
        assert!(out.contains("admin_password = \"$2y$10$abc\""));
        assert!(out.contains("admin_login = \"admin\""));
    }

    #[test]
    fn client_id_from_paths() {
        assert_eq!(
            client_id_for_data_dir("/var/lib/cpn-webmail/tachyon/"),
            "tachyon"
        );
        assert_eq!(
            client_id_for_data_dir("/opt/nextcloud/data/appdata_x/nextsnapmail"),
            "nextsnapmail"
        );
        assert_eq!(
            client_id_for_data_dir("/var/lib/cpn-webmail/snappymail/"),
            "snappymail"
        );
    }
}
