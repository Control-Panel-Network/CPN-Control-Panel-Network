//! Pattern-based mail forwarding (glob/regex → destination) with Postfix virtual maps.

use crate::paths::join_data;
use crate::postfix_fallback::postfix_is_ready;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternForward {
    pub id: String,
    /// Glob (`*@example.com`) or regex (`^sales-.*@example\\.com$`).
    pub pattern: String,
    pub destination: String,
    /// `glob` or `regex`.
    pub kind: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PatternFile {
    #[serde(default)]
    rules: Vec<PatternForward>,
}

fn store_path() -> PathBuf {
    join_data("mail-pattern-forwards.json")
}

fn virtual_map_path() -> PathBuf {
    join_data("mail/virtual_pattern")
}

pub fn load_pattern_rules() -> Vec<PatternForward> {
    fs::read_to_string(store_path())
        .ok()
        .and_then(|raw| serde_json::from_str::<PatternFile>(&raw).ok())
        .map(|f| f.rules)
        .unwrap_or_default()
}

fn save_pattern_rules(rules: &[PatternForward]) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(&PatternFile {
        rules: rules.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn validate_pattern(kind: &str, pattern: &str) -> Result<(), String> {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern.len() > 200 {
        return Err("Pattern is required (max 200 characters)".into());
    }
    if pattern.contains('\n') || pattern.contains('\0') {
        return Err("Pattern must be a single line".into());
    }
    match kind {
        "glob" => {
            if !pattern.contains('@') {
                return Err(
                    "Glob patterns should include @domain (e.g. sales-*@example.com)".into(),
                );
            }
            Ok(())
        }
        "regex" => {
            if regex_lite_is_safe(pattern) {
                Ok(())
            } else {
                Err("Regex looks unsafe or empty".into())
            }
        }
        _ => Err("Kind must be glob or regex".into()),
    }
}

fn regex_lite_is_safe(pattern: &str) -> bool {
    !pattern.is_empty()
        && !pattern.contains("(?")
        && pattern.chars().filter(|c| *c == '(').count() < 20
}

fn validate_destination(dest: &str) -> Result<String, String> {
    let dest = dest.trim().to_ascii_lowercase();
    if dest.is_empty() || !dest.contains('@') || dest.contains(' ') {
        return Err("Destination must be a valid email address".into());
    }
    Ok(dest)
}

/// Convert a simple glob (`*@domain`, `prefix*@domain`) into a Postfix virtual left-hand side.
fn glob_to_virtual_lhs(pattern: &str) -> Option<String> {
    let p = pattern.trim().to_ascii_lowercase();
    if p.starts_with('*') && p.matches('*').count() == 1 {
        // *@example.com → @example.com (domain catch-style; Postfix virtual)
        return Some(p.replacen('*', "", 1));
    }
    if p.contains('*') {
        // sales-*@example.com → approximate as exact prefix without wildcard for map safety
        let flat = p.replace('*', "");
        if flat.contains('@') {
            return Some(flat);
        }
    }
    if p.contains('@') && !p.contains('*') {
        return Some(p);
    }
    None
}

pub fn add_pattern_rule(kind: &str, pattern: &str, destination: &str) -> Result<String, String> {
    let kind = kind.trim().to_ascii_lowercase();
    validate_pattern(&kind, pattern)?;
    let destination = validate_destination(destination)?;
    let mut rules = load_pattern_rules();
    let id = format!("pf-{}", crate::account::now_unix());
    rules.push(PatternForward {
        id: id.clone(),
        pattern: pattern.trim().to_string(),
        destination,
        kind,
        enabled: true,
    });
    save_pattern_rules(&rules)?;
    let apply = apply_pattern_maps()?;
    Ok(format!("Pattern rule {id} saved. {apply}"))
}

pub fn remove_pattern_rule(id: &str) -> Result<String, String> {
    let id = id.trim();
    let mut rules = load_pattern_rules();
    let before = rules.len();
    rules.retain(|r| r.id != id);
    if rules.len() == before {
        return Err(format!("Rule `{id}` not found"));
    }
    save_pattern_rules(&rules)?;
    let apply = apply_pattern_maps()?;
    Ok(format!("Rule removed. {apply}"))
}

pub fn apply_pattern_maps() -> Result<String, String> {
    let rules = load_pattern_rules();
    let map_path = virtual_map_path();
    if let Some(parent) = map_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create mail map dir: {e}"))?;
    }
    let mut lines = String::from("# CPN pattern forwarding virtual map\n");
    let mut skipped = 0u32;
    for rule in rules.iter().filter(|r| r.enabled) {
        if rule.kind == "regex" {
            skipped += 1;
            continue;
        }
        if let Some(lhs) = glob_to_virtual_lhs(&rule.pattern) {
            lines.push_str(&format!("{lhs}\t{}\n", rule.destination));
        } else {
            skipped += 1;
        }
    }
    fs::write(&map_path, &lines).map_err(|e| format!("Cannot write virtual map: {e}"))?;

    if !postfix_is_ready() {
        return Ok(format!(
            "Map written to {} (Postfix not ready; {} regex/complex rules stored panel-only).",
            map_path.display(),
            skipped
        ));
    }

    let hash = Command::new("postmap")
        .arg(map_path.to_string_lossy().as_ref())
        .output();
    match hash {
        Ok(out) if out.status.success() => {
            let map_arg = format!("hash:{}", map_path.to_string_lossy());
            let existing = Command::new("postconf")
                .args(["-h", "virtual_alias_maps"])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            let new_maps = if existing.is_empty() || existing == "{}" {
                map_arg.clone()
            } else if existing.split(',').any(|p| p.trim() == map_arg) {
                existing
            } else {
                format!("{existing}, {map_arg}")
            };
            let _ = Command::new("postconf")
                .args(["-e", &format!("virtual_alias_maps={new_maps}")])
                .status();
            let _ = Command::new("systemctl")
                .args(["reload", "postfix"])
                .status();
            Ok(format!(
                "Applied hash map at {} ({} regex/complex rules panel-only).",
                map_path.display(),
                skipped
            ))
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            Ok(format!(
                "Map saved at {}; postmap note: {}",
                map_path.display(),
                err.trim()
            ))
        }
        Err(e) => Ok(format!(
            "Map saved at {}; postmap unavailable: {e}",
            map_path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn pattern_roundtrip() {
        with_test_data_dir(|| {
            add_pattern_rule("glob", "news-*@example.com", "inbox@example.com").unwrap();
            let rules = load_pattern_rules();
            assert_eq!(rules.len(), 1);
            remove_pattern_rule(&rules[0].id).unwrap();
            assert!(load_pattern_rules().is_empty());
        });
    }
}
