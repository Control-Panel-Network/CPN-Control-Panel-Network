//! PHP branch lifecycle checks so CPN never selects an EOL runtime (issue #4).

use crate::model::MailSystem;

/// Upstream end-of-security-support dates (PHP.net). Keep updated per release.
const PHP_BRANCH_EOL: &[(&str, &str)] = &[
    ("8.0", "2023-11-26"),
    ("8.1", "2025-12-31"),
    ("8.2", "2026-12-31"),
    ("8.3", "2027-12-31"),
    ("8.4", "2028-12-31"),
];

/// Minimum CPN-supported PHP major.minor while this release is published.
pub const CPN_MIN_PHP_BRANCH: &str = "8.2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhpCompatRange {
    pub min: &'static str,
    pub max: &'static str,
}

pub fn webmail_php_compat(mail: MailSystem) -> Option<PhpCompatRange> {
    match mail {
        MailSystem::Snappymail => Some(PhpCompatRange {
            min: "8.1",
            max: "8.4",
        }),
        MailSystem::Roundcube => Some(PhpCompatRange {
            min: "8.1",
            max: "8.4",
        }),
        MailSystem::Thunderbird => None,
    }
}

fn parse_ymd(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    Some((year, month, day))
}

fn branch_major_minor(branch: &str) -> Option<(u32, u32)> {
    let mut parts = branch.trim().trim_start_matches('v').split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Returns true when `branch` (e.g. "8.2") still has upstream security support on `today_ymd`.
pub fn php_branch_supported_on(branch: &str, today_ymd: &str) -> bool {
    let Some((_, eol)) = PHP_BRANCH_EOL
        .iter()
        .find(|(name, _)| *name == branch.trim())
    else {
        // Unknown future branch: allow, but require CPN_MIN_PHP_BRANCH floor separately.
        return true;
    };
    let Some(today) = parse_ymd(today_ymd) else {
        return false;
    };
    let Some(eol_day) = parse_ymd(eol) else {
        return false;
    };
    today <= eol_day
}

pub fn assert_php_branch_not_eol(branch: &str, today_ymd: &str) -> Result<(), String> {
    let Some((major, minor)) = branch_major_minor(branch) else {
        return Err(format!("Invalid PHP branch: {branch}"));
    };
    let Some((min_major, min_minor)) = branch_major_minor(CPN_MIN_PHP_BRANCH) else {
        return Err("Internal CPN_MIN_PHP_BRANCH is invalid".into());
    };
    if (major, minor) < (min_major, min_minor) {
        return Err(format!(
            "PHP {branch} is below CPN minimum {CPN_MIN_PHP_BRANCH}"
        ));
    }
    if !php_branch_supported_on(branch, today_ymd) {
        return Err(format!(
            "PHP {branch} is past upstream EOL on {today_ymd}; refuse install (issue #4)"
        ));
    }
    Ok(())
}

/// Map GuestOs module stream / Remi tag to a PHP branch label.
pub fn branch_from_module_stream(stream: &str) -> Option<&'static str> {
    let value = stream.trim();
    if value.contains("8.4") {
        return Some("8.4");
    }
    if value.contains("8.3") {
        return Some("8.3");
    }
    if value.contains("8.2") {
        return Some("8.2");
    }
    if value.contains("8.1") {
        return Some("8.1");
    }
    if value.contains("8.0") {
        return Some("8.0");
    }
    None
}

pub fn assert_selected_runtime_ok(stream: &str, today_ymd: &str) -> Result<(), String> {
    let branch = branch_from_module_stream(stream).ok_or_else(|| {
        format!("Cannot derive PHP branch from module stream '{stream}'")
    })?;
    assert_php_branch_not_eol(branch, today_ymd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_eol_81_in_2026() {
        assert!(assert_php_branch_not_eol("8.1", "2026-09-04").is_err());
        assert!(assert_php_branch_not_eol("8.0", "2026-09-04").is_err());
    }

    #[test]
    fn accepts_82_before_eol() {
        assert!(assert_php_branch_not_eol("8.2", "2026-09-04").is_ok());
    }

    #[test]
    fn remi_and_module_streams_map() {
        assert_eq!(branch_from_module_stream("remi-8.2"), Some("8.2"));
        assert_eq!(branch_from_module_stream("php:8.2"), Some("8.2"));
        assert!(assert_selected_runtime_ok("php:8.2", "2026-09-04").is_ok());
        assert!(assert_selected_runtime_ok("php:8.1", "2026-09-04").is_err());
    }

    #[test]
    fn webmail_declares_php_range() {
        let snappy = webmail_php_compat(MailSystem::Snappymail).expect("range");
        assert_eq!(snappy.min, "8.1");
        let roundcube = webmail_php_compat(MailSystem::Roundcube).expect("range");
        assert_eq!(roundcube.max, "8.4");
        assert!(webmail_php_compat(MailSystem::Thunderbird).is_none());
    }
}
