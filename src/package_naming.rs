//! Hosting package display names: `{owner}_{custom}` (Default stays `Default`).

use crate::packages::{DEFAULT_PACKAGE_ID, Package};

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

fn has_control_chars(value: &str) -> bool {
    value.chars().any(|ch| ch.is_control())
}

/// Sanitize the custom segment of `{owner}_{custom}` (letters, digits, `_`, `-`).
pub fn sanitize_package_custom_name(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Package custom name is required".into());
    }
    if trimmed.chars().count() > 100 {
        return Err("Package custom name is too long (max 100 characters)".into());
    }
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-' => out.push(ch),
            ' ' | '\t' => out.push('-'),
            c if c.is_control() => {
                return Err("Package name cannot include control characters".into());
            }
            _ => {
                return Err(
                    "Package name may only contain letters, numbers, underscore, and hyphen".into(),
                );
            }
        }
    }
    let cleaned = out.trim_matches(|c| c == '-' || c == '_').to_string();
    if cleaned.is_empty() {
        return Err("Package custom name is required".into());
    }
    Ok(cleaned)
}

/// Owner username inferred from an existing `{owner}_{custom}` package name.
pub fn package_owner_from_name(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || names_equal(name, "Default") {
        return None;
    }
    name.split_once('_')
        .map(|(owner, _)| owner.trim())
        .filter(|owner| !owner.is_empty())
        .map(str::to_string)
}

/// Custom part shown in edit/duplicate forms (`Default` stays `Default`).
pub fn package_custom_name_for_edit(pkg: &Package) -> String {
    if pkg.id == DEFAULT_PACKAGE_ID || names_equal(&pkg.name, "Default") {
        return "Default".into();
    }
    if let Some((_, rest)) = pkg.name.split_once('_') {
        let rest = rest.trim();
        if !rest.is_empty() {
            return rest.to_string();
        }
    }
    pkg.name.clone()
}

/// Build `{owner}_{custom}` from owner + typed name. Skips double-prefix when
/// the typed value already starts with `{owner}_`.
pub fn normalize_owned_package_name(owner: &str, typed_name: &str) -> Result<String, String> {
    let owner = owner.trim();
    if owner.is_empty() {
        return Err("Package owner username is required".into());
    }
    if has_control_chars(owner) || owner.chars().any(|c| c.is_whitespace()) {
        return Err("Invalid package owner username".into());
    }
    let typed = typed_name.trim();
    if typed.is_empty() {
        return Err("Package custom name is required".into());
    }
    let prefix = format!("{owner}_");
    let custom_src = if typed.len() >= prefix.len()
        && typed
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(&prefix))
    {
        typed.get(prefix.len()..).unwrap_or("")
    } else {
        typed
    };
    let custom = sanitize_package_custom_name(custom_src)?;
    Ok(format!("{owner}_{custom}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_name_prefixes_and_skips_double_prefix() {
        assert_eq!(
            normalize_owned_package_name("cpnowner", "test").unwrap(),
            "cpnowner_test"
        );
        assert_eq!(
            normalize_owned_package_name("cpnowner", "cpnowner_foo").unwrap(),
            "cpnowner_foo"
        );
        assert_eq!(
            normalize_owned_package_name("cpnowner", "CPNOWNER_Bar").unwrap(),
            "cpnowner_Bar"
        );
        assert!(normalize_owned_package_name("cpnowner", "   ").is_err());
        assert!(normalize_owned_package_name("cpnowner", "bad name!").is_err());
    }
}
