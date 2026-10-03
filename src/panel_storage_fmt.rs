//! Panel storage / bandwidth display: Auto (dynamic) or a forced KB/MB/GB/TB unit.
//!
//! Package quotas stay stored as MB integers (`-1` unlimited). Display converts those
//! MB values to bytes (1024-based) so Auto never prints `500000 MB`.

use crate::packages::{UNLIMITED, is_unlimited};
use crate::panel_user_prefs::{StorageUnitPref, load_user_storage_unit};

const KB: f64 = 1024.0;
const MB: f64 = KB * 1024.0;
const GB: f64 = MB * 1024.0;
const TB: f64 = GB * 1024.0;

/// Infinity glyph used when a quota is unlimited.
pub const INFINITY_MARK: &str = "∞";

/// Accessible infinity mark for HTML tables and meters.
pub fn unlimited_html() -> &'static str {
    r#"<span class="cpn-unlimited" title="Unlimited" aria-label="Unlimited">∞</span>"#
}

/// Bytes in one stored package megabyte (binary MiB).
pub fn mb_limit_to_bytes(mb: i64) -> Option<u64> {
    if is_unlimited(mb) || mb < 0 {
        return None;
    }
    u64::try_from(mb).ok()?.checked_mul(1024 * 1024)
}

pub fn used_mb_to_bytes(mb: u64) -> u64 {
    mb.saturating_mul(1024 * 1024)
}

pub fn format_bytes_auto(bytes: u64) -> String {
    format_bytes_with(StorageUnitPref::Auto, bytes)
}

pub fn format_bytes_for_user(username: &str, bytes: u64) -> String {
    format_bytes_with(load_user_storage_unit(username), bytes)
}

pub fn format_mb_limit_for_user(username: &str, limit_mb: i64) -> String {
    format_mb_limit(load_user_storage_unit(username), limit_mb)
}

pub fn format_used_limit_for_user(username: &str, used_bytes: u64, limit_mb: i64) -> String {
    format_used_limit(load_user_storage_unit(username), used_bytes, limit_mb)
}

pub fn format_used_mb_limit_for_user(username: &str, used_mb: u64, limit_mb: i64) -> String {
    format_used_limit(
        load_user_storage_unit(username),
        used_mb_to_bytes(used_mb),
        limit_mb,
    )
}

pub fn format_mb_limit(pref: StorageUnitPref, limit_mb: i64) -> String {
    if is_unlimited(limit_mb) {
        return INFINITY_MARK.into();
    }
    match mb_limit_to_bytes(limit_mb) {
        Some(bytes) => format_bytes_with(pref, bytes),
        None => format_bytes_with(pref, 0),
    }
}

pub fn format_used_limit(pref: StorageUnitPref, used_bytes: u64, limit_mb: i64) -> String {
    let used = format_bytes_with(pref, used_bytes);
    format!("{used} / {}", format_mb_limit(pref, limit_mb))
}

/// Count meters (`3 / ∞`) for domains, mailboxes, and similar quotas.
pub fn format_used_count(used: u64, limit: i64) -> String {
    if is_unlimited(limit) {
        format!("{used} / {INFINITY_MARK}")
    } else {
        format!("{used} / {limit}")
    }
}

pub fn format_bytes_with(pref: StorageUnitPref, bytes: u64) -> String {
    let unit = match pref {
        StorageUnitPref::Auto => auto_unit(bytes),
        StorageUnitPref::Kb => Unit::Kb,
        StorageUnitPref::Mb => Unit::Mb,
        StorageUnitPref::Gb => Unit::Gb,
        StorageUnitPref::Tb => Unit::Tb,
    };
    format_unit(bytes, unit, pref == StorageUnitPref::Auto)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Kb,
    Mb,
    Gb,
    Tb,
}

fn auto_unit(bytes: u64) -> Unit {
    let v = bytes as f64;
    if v >= TB {
        Unit::Tb
    } else if v >= GB {
        Unit::Gb
    } else if v >= MB {
        Unit::Mb
    } else {
        // Floor is KB: never print a bare byte unit.
        Unit::Kb
    }
}

fn format_unit(bytes: u64, unit: Unit, auto: bool) -> String {
    if bytes == 0 && unit == Unit::Kb {
        return "0 KB".into();
    }
    let (div, label, default_decimals) = match unit {
        Unit::Kb => {
            let decimals = if auto && (bytes as f64) < KB {
                2
            } else if auto {
                0
            } else {
                1
            };
            (KB, "KB", decimals)
        }
        Unit::Mb => (MB, "MB", 1),
        Unit::Gb => (GB, "GB", 1),
        Unit::Tb => (TB, "TB", 2),
    };
    let scaled = bytes as f64 / div;
    let mut decimals = default_decimals;
    if !auto && scaled > 0.0 && scaled < 0.01 {
        decimals = 4;
    } else if !auto && unit == Unit::Gb && scaled < 1.0 {
        decimals = 2;
    }
    let text = format_scaled(scaled, decimals);
    if !auto && scaled > 0.0 && text.chars().all(|c| c == '0' || c == '.' || c == ',') {
        let text = format_scaled(scaled, 4);
        return format!("{text} {label}");
    }
    format!("{text} {label}")
}

fn format_scaled(value: f64, decimals: u32) -> String {
    let factor = 10f64.powi(decimals as i32);
    let rounded = (value * factor).round() / factor;
    let int_part = rounded.trunc() as u64;
    let mut out = group_u64(int_part);
    if decimals == 0 {
        return out;
    }
    let frac = ((rounded.fract() * factor).round() as u64).min(factor as u64 - 1);
    if frac == 0 && decimals == 1 {
        // Keep one decimal for GB/MB Auto so 19.0 GB stays 19.0 when wanted; drop trailing
        // .0 for KB-style whole numbers only when decimals were 0 (handled above).
        out.push('.');
        out.push_str(&format!("{:0width$}", frac, width = decimals as usize));
        return out;
    }
    out.push('.');
    out.push_str(&format!("{:0width$}", frac, width = decimals as usize));
    out
}

fn group_u64(n: u64) -> String {
    let raw = n.to_string();
    let mut out = String::new();
    for (i, ch) in raw.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

pub fn storage_unit_options_html(selected: StorageUnitPref) -> String {
    let mut out = String::new();
    for (value, label) in [
        (StorageUnitPref::Auto, "Auto (dynamic)"),
        (StorageUnitPref::Kb, "KB"),
        (StorageUnitPref::Mb, "MB"),
        (StorageUnitPref::Gb, "GB"),
        (StorageUnitPref::Tb, "TB"),
    ] {
        let sel = if value == selected { " selected" } else { "" };
        out.push_str(&format!(
            r#"<option value="{v}"{sel}>{label}</option>"#,
            v = value.as_str(),
            sel = sel,
            label = label,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_scales_quota_mb_to_gb() {
        // 500000 MiB = 488.28125 GiB
        let label = format_mb_limit(StorageUnitPref::Auto, 500_000);
        assert!(
            label.contains("488.3 GB") || label.contains("488.28 GB"),
            "got {label}"
        );
        assert!(!label.contains("500000"));
        assert!(!label.ends_with(" MB") || label.contains("GB"));
    }

    #[test]
    fn auto_keeps_small_values_in_kb() {
        assert_eq!(format_bytes_with(StorageUnitPref::Auto, 5 * 1024), "5 KB");
        assert_eq!(format_bytes_with(StorageUnitPref::Auto, 987), "0.96 KB");
        assert_eq!(format_bytes_with(StorageUnitPref::Auto, 0), "0 KB");
        let tiny = format_bytes_with(StorageUnitPref::Auto, 512);
        assert!(tiny.ends_with(" KB"), "{tiny}");
        assert!(!tiny.contains(" B"), "{tiny}");
    }

    #[test]
    fn used_over_limit_pairs_units() {
        let s = format_used_limit(StorageUnitPref::Auto, 5 * 1024, 500_000);
        assert!(s.starts_with("5 KB / "), "{s}");
        assert!(s.contains("GB"), "{s}");
        assert!(!s.contains("500000"), "{s}");
    }

    #[test]
    fn forced_gb_honors_pick() {
        let s = format_bytes_with(StorageUnitPref::Gb, 5 * 1024);
        assert!(s.ends_with(" GB"), "{s}");
        assert!(!s.contains("KB"), "{s}");
    }

    #[test]
    fn forced_mb_for_large_quota() {
        let s = format_mb_limit(StorageUnitPref::Mb, 500_000);
        assert!(s.contains("500,000") || s.contains("500000"), "{s}");
        assert!(s.ends_with(" MB"), "{s}");
    }

    #[test]
    fn unlimited_stays_unlimited() {
        assert_eq!(format_mb_limit(StorageUnitPref::Auto, UNLIMITED), "∞");
        assert_eq!(format_mb_limit(StorageUnitPref::Auto, 0), "∞");
        assert_eq!(
            format_used_limit(StorageUnitPref::Auto, 1024, UNLIMITED),
            "1 KB / ∞"
        );
        assert_eq!(format_used_count(3, 0), "3 / ∞");
        assert_eq!(format_used_count(3, 10), "3 / 10");
    }
}
