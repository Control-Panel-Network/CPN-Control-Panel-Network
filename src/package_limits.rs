//! Package quota sentinels: only `-1` means unlimited. `0` means none allowed.

/// Sentinel stored for unlimited resource limits.
pub const UNLIMITED: i64 = -1;

/// True when the quota is unlimited (`-1` only).
pub fn is_unlimited(limit: i64) -> bool {
    limit == UNLIMITED
}

/// Pass-through normalizer: `0` stays `0` (hard zero); `-1` stays unlimited.
pub fn normalize_limit(value: i64) -> i64 {
    if value == UNLIMITED { UNLIMITED } else { value }
}

/// Serde default for newly added package limit fields (unlimited).
pub fn default_unlimited() -> i64 {
    UNLIMITED
}

/// Historical packages used `0` as unlimited. Convert those zeros to `-1`.
pub fn migrate_legacy_zero_unlimited(value: i64) -> i64 {
    if value == 0 { UNLIMITED } else { value }
}

pub fn format_limit_display(limit: i64, unit: &str) -> String {
    if is_unlimited(limit) {
        "∞".into()
    } else if unit.is_empty() {
        limit.to_string()
    } else {
        format!("{limit} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_minus_one_is_unlimited() {
        assert!(!is_unlimited(0));
        assert!(is_unlimited(-1));
        assert!(!is_unlimited(1));
        assert_eq!(normalize_limit(0), 0);
        assert_eq!(normalize_limit(-1), UNLIMITED);
        assert_eq!(normalize_limit(50), 50);
        assert_eq!(format_limit_display(0, "MB"), "0 MB");
        assert_eq!(format_limit_display(-1, "MB"), "∞");
        assert_eq!(migrate_legacy_zero_unlimited(0), UNLIMITED);
        assert_eq!(migrate_legacy_zero_unlimited(5), 5);
    }
}
