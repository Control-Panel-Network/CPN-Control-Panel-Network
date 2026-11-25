//! Install and enable the Tachyon VirusTotal extension by default.
//!
//! Bundled under `assets/tachyon/plugins/virustotal/` (same package as CPN-Plugins
//! `tachyonVirusTotal`). Runs on Tachyon install and every webmail heal / tip-upgrade
//! so tip upgrades and webmail-only updates keep the extension present and enabled.
//!
//! The API key stays optional (GUI-link mode works without one). No secrets are
//! written into configs.

use crate::install_snappymail_lineage::{
    application_ini_path, chown_data_tree, client_id_for_data_dir, lineage_data_dirs,
};
use std::fs;
use std::path::{Path, PathBuf};

const PLUGIN_ID: &str = "virustotal";
const BUNDLED_VERSION: &str = "1.0.0";

const INDEX_PHP: &str = include_str!("../assets/tachyon/plugins/virustotal/index.php");
const VIRUSTOTAL_JS: &str = include_str!("../assets/tachyon/plugins/virustotal/virustotal.js");
const LICENSE: &str = include_str!("../assets/tachyon/plugins/virustotal/LICENSE");
const LANG_EN: &str = include_str!("../assets/tachyon/plugins/virustotal/langs/en.json");
const LANG_NB: &str = include_str!("../assets/tachyon/plugins/virustotal/langs/nb.json");

/// Ensure VirusTotal is installed under Tachyon data and listed in `enabled_list`.
///
/// Soft no-op when Tachyon is not present. Idempotent: re-runs after tip-upgrade /
/// heal refresh plugin files when the bundled version is newer, and always re-add
/// `virustotal` to `enabled_list` if an operator removed only the enable flag while
/// leaving the plugin files (default-on policy for this free extension).
pub fn ensure_tachyon_virustotal_default() -> Result<(), String> {
    if !tachyon_present() {
        return Ok(());
    }
    let mut any = false;
    for data_dir in lineage_data_dirs() {
        if client_id_for_data_dir(&data_dir) != "tachyon" {
            continue;
        }
        any = true;
        install_plugin_files(&data_dir)?;
        enable_plugin_in_ini(&data_dir)?;
        let _ = chown_data_tree(&data_dir);
    }
    // Fresh Tachyon before first visit: data root may exist without application.ini yet.
    // Still stage plugin files under the canonical data path so the first Admin load
    // sees VirusTotal ready once configs are created and heal re-enables it.
    if !any {
        let fallback = "/var/lib/cpn-webmail/tachyon";
        if Path::new(fallback).is_dir() || Path::new("/opt/cpn-webmail/tachyon").is_dir() {
            let _ = fs::create_dir_all(format!("{fallback}/_data_/_default_/plugins"));
            install_plugin_files(fallback)?;
            enable_plugin_in_ini(fallback)?;
            let _ = chown_data_tree(fallback);
        }
    }
    Ok(())
}

fn tachyon_present() -> bool {
    Path::new("/opt/cpn-webmail/tachyon/index.php").is_file()
        || Path::new("/opt/cpn-webmail/tachyon").is_dir()
        || Path::new("/var/lib/cpn-webmail/tachyon").is_dir()
}

fn plugins_dir(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join("_data_/_default_/plugins")
}

fn plugin_dest(data_dir: &str) -> PathBuf {
    plugins_dir(data_dir).join(PLUGIN_ID)
}

fn installed_version(dest: &Path) -> Option<String> {
    let index = dest.join("index.php");
    let raw = fs::read_to_string(index).ok()?;
    parse_plugin_version(&raw)
}

fn parse_plugin_version(index_php: &str) -> Option<String> {
    for line in index_php.lines() {
        let trimmed = line.trim();
        // const VERSION = '1.0.0' / VERSION  = '1.0.0',
        if !trimmed.contains("VERSION") {
            continue;
        }
        let Some(rest) = trimmed
            .split_once("VERSION")
            .map(|(_, r)| r.trim_start_matches([' ', '=', '\t']))
        else {
            continue;
        };
        let quote = rest.chars().next().filter(|c| ['\'', '"'].contains(c))?;
        let rest = &rest[1..];
        let end = rest.find(quote)?;
        let ver = rest[..end].trim();
        if !ver.is_empty() {
            return Some(ver.to_string());
        }
    }
    None
}

fn version_needs_refresh(installed: Option<&str>) -> bool {
    match installed {
        None => true,
        Some(v) => v != BUNDLED_VERSION,
    }
}

fn install_plugin_files(data_dir: &str) -> Result<(), String> {
    let dest = plugin_dest(data_dir);
    let current = installed_version(&dest);
    if !version_needs_refresh(current.as_deref()) && dest.join("virustotal.js").is_file() {
        return Ok(());
    }
    fs::create_dir_all(dest.join("langs")).map_err(|e| format!("virustotal plugins dir: {e}"))?;
    write_text(&dest.join("index.php"), INDEX_PHP)?;
    write_text(&dest.join("virustotal.js"), VIRUSTOTAL_JS)?;
    write_text(&dest.join("LICENSE"), LICENSE)?;
    write_text(&dest.join("langs/en.json"), LANG_EN)?;
    write_text(&dest.join("langs/nb.json"), LANG_NB)?;
    Ok(())
}

fn write_text(path: &Path, body: &str) -> Result<(), String> {
    fs::write(path, body).map_err(|e| format!("write {}: {e}", path.display()))
}

fn enable_plugin_in_ini(data_dir: &str) -> Result<(), String> {
    let ini = application_ini_path(data_dir);
    if !ini.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(&ini).map_err(|e| e.to_string())?;
    let updated = ensure_plugins_enabled_list(&raw, PLUGIN_ID);
    if updated != raw {
        fs::write(&ini, updated).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Ensure `[plugins] enable = On` and `plugin_id` appears in `enabled_list`.
///
/// Only mutates the `[plugins]` section (other `enable =` keys are left alone).
pub(crate) fn ensure_plugins_enabled_list(raw: &str, plugin_id: &str) -> String {
    let mut sections = split_ini_sections(raw);
    let plugins_idx = sections
        .iter()
        .position(|(name, _)| name.eq_ignore_ascii_case("plugins"));
    let idx = match plugins_idx {
        Some(i) => i,
        None => {
            sections.push((
                "plugins".into(),
                format!("enable = On\nenabled_list = \"{plugin_id}\"\n"),
            ));
            return join_ini_sections(&sections);
        }
    };
    let body = &mut sections[idx].1;
    *body = set_section_bool(body, "enable", true);
    *body = merge_enabled_list(body, plugin_id);
    join_ini_sections(&sections)
}

fn split_ini_sections(raw: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut name = String::new();
    let mut body = String::new();
    let mut started = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
            if started || !body.is_empty() || !name.is_empty() {
                out.push((name.clone(), body.clone()));
            }
            name = trimmed[1..trimmed.len() - 1].to_string();
            body.clear();
            started = true;
            continue;
        }
        if started || !trimmed.is_empty() || !body.is_empty() {
            // Preserve preamble before the first section as an empty-named block.
            if !started && name.is_empty() && out.is_empty() && body.is_empty() {
                // keep collecting preamble into body with empty name
            }
            body.push_str(line);
            body.push('\n');
        }
    }
    if started || !body.is_empty() || !name.is_empty() {
        out.push((name, body));
    }
    if out.is_empty() {
        out.push((String::new(), String::new()));
    }
    out
}

fn join_ini_sections(sections: &[(String, String)]) -> String {
    let mut out = String::new();
    for (i, (name, body)) in sections.iter().enumerate() {
        if name.is_empty() {
            out.push_str(body);
            continue;
        }
        if i > 0 && !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push('[');
        out.push_str(name);
        out.push_str("]\n");
        out.push_str(body);
        if !body.is_empty() && !body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// True when `trimmed` is an assignment for exact `key` (`key =` / `key=`).
///
/// Rejects longer keys that share a prefix (for example `enable` must not match
/// `enabled_list`).
fn ini_assignment_key_matches(trimmed: &str, key: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix(key) else {
        return false;
    };
    let rest = rest.trim_start();
    rest.starts_with('=')
}

fn set_section_bool(section_body: &str, key: &str, on: bool) -> String {
    let word = if on { "On" } else { "Off" };
    let mut out = String::with_capacity(section_body.len() + 16);
    let mut replaced = false;
    for line in section_body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(';') || trimmed.starts_with('#') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if ini_assignment_key_matches(trimmed, key) {
            out.push_str(&format!("{key} = {word}\n"));
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(&format!("{key} = {word}\n"));
    }
    out
}

fn merge_enabled_list(section_body: &str, plugin_id: &str) -> String {
    let mut out = String::with_capacity(section_body.len() + plugin_id.len() + 8);
    let mut replaced = false;
    for line in section_body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(';') || trimmed.starts_with('#') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if ini_assignment_key_matches(trimmed, "enabled_list") {
            let rhs = trimmed.split_once('=').map(|(_, r)| r.trim()).unwrap_or("");
            let raw = rhs.trim_matches('"').trim_matches('\'').trim();
            let mut parts: Vec<String> = raw
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if !parts.iter().any(|p| p == plugin_id) {
                parts.push(plugin_id.to_string());
            }
            out.push_str("enabled_list = \"");
            out.push_str(&parts.join(","));
            out.push_str("\"\n");
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        out.push_str(&format!("enabled_list = \"{plugin_id}\"\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bundled_version() {
        assert_eq!(
            parse_plugin_version(INDEX_PHP).as_deref(),
            Some(BUNDLED_VERSION)
        );
    }

    #[test]
    fn adds_plugins_section_when_missing() {
        let raw = "[security]\nadmin_login = \"owner\"\n";
        let out = ensure_plugins_enabled_list(raw, "virustotal");
        assert!(out.contains("[plugins]"));
        assert!(out.contains("enable = On"));
        assert!(out.contains("enabled_list = \"virustotal\""));
        assert!(out.contains("admin_login = \"owner\""));
    }

    #[test]
    fn merges_into_existing_enabled_list() {
        let raw = "[plugins]\nenable = Off\nenabled_list = \"demo,other\"\n";
        let out = ensure_plugins_enabled_list(raw, "virustotal");
        assert!(out.contains("enable = On"));
        assert!(out.contains("enabled_list = \"demo,other,virustotal\""));
    }

    #[test]
    fn idempotent_when_already_enabled() {
        let raw = "[plugins]\nenable = On\nenabled_list = \"virustotal,demo\"\n";
        let out = ensure_plugins_enabled_list(raw, "virustotal");
        assert_eq!(out.matches("virustotal").count(), 1);
        assert!(out.contains("enabled_list = \"virustotal,demo\""));
    }

    #[test]
    fn does_not_flip_enable_outside_plugins() {
        let raw = "[cache]\nenable = Off\n\n[plugins]\nenable = Off\nenabled_list = \"\"\n";
        let out = ensure_plugins_enabled_list(raw, "virustotal");
        assert!(out.contains("[cache]\nenable = Off"));
        assert!(out.contains("[plugins]"));
        assert!(out.contains("enable = On"));
        assert!(out.contains("enabled_list = \"virustotal\""));
    }

    #[test]
    fn version_refresh_detects_mismatch() {
        assert!(version_needs_refresh(None));
        assert!(version_needs_refresh(Some("0.9.0")));
        assert!(!version_needs_refresh(Some(BUNDLED_VERSION)));
    }

    #[test]
    fn enable_key_does_not_match_enabled_list() {
        assert!(ini_assignment_key_matches("enable = Off", "enable"));
        assert!(ini_assignment_key_matches("enable=On", "enable"));
        assert!(!ini_assignment_key_matches("enabled_list = \"virustotal\"", "enable"));
        assert!(ini_assignment_key_matches(
            "enabled_list = \"virustotal\"",
            "enabled_list"
        ));
    }

    #[test]
    fn tachyon_commented_plugins_section_turns_enable_on() {
        // Real Tachyon application.ini layout (comments between keys).
        let raw = "\
[plugins]
; Enable plugin support
enable = Off

; Comma-separated list of enabled plugins
enabled_list = \"\"
";
        let out = ensure_plugins_enabled_list(raw, "virustotal");
        let plugins = out
            .split('[')
            .find(|s| s.starts_with("plugins]"))
            .expect("plugins section");
        assert!(
            plugins.lines().any(|l| {
                let t = l.trim();
                !t.starts_with(';') && ini_assignment_key_matches(t, "enable") && t.contains("On")
            }),
            "plugins.enable must be On, got:\n{plugins}"
        );
        assert!(
            plugins.lines().any(|l| {
                let t = l.trim();
                !t.starts_with(';')
                    && ini_assignment_key_matches(t, "enabled_list")
                    && t.contains("virustotal")
            }),
            "enabled_list must include virustotal, got:\n{plugins}"
        );
        // Unrelated Off keys outside [plugins] stay Off when present.
        let with_logs = format!("[logs]\nenable = Off\n\n{raw}");
        let out2 = ensure_plugins_enabled_list(&with_logs, "virustotal");
        assert!(out2.contains("[logs]\nenable = Off"));
        assert!(out2.contains("enabled_list = \"virustotal\""));
    }
}
