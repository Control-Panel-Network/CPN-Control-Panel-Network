//! Package quota sentinels: `-1` and `0` both mean unlimited.

/// Sentinel stored for unlimited resource limits.
pub const UNLIMITED: i64 = -1;

/// True when the quota is unlimited (`-1` or `0`).
pub fn is_unlimited(limit: i64) -> bool {
    limit == UNLIMITED || limit == 0
}

/// Store unlimited as `-1` so list and edit share one sentinel.
pub fn normalize_limit(value: i64) -> i64 {
    if is_unlimited(value) {
        UNLIMITED
    } else {
        value
    }
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
    fn zero_and_minus_one_are_unlimited() {
        assert!(is_unlimited(0));
        assert!(is_unlimited(-1));
        assert!(!is_unlimited(1));
        assert_eq!(normalize_limit(0), UNLIMITED);
        assert_eq!(normalize_limit(50), 50);
        assert_eq!(format_limit_display(0, "MB"), "∞");
        assert_eq!(format_limit_display(20, ""), "20");
    }
}
