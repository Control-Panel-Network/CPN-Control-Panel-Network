//! Per-install schedule jitter and next-check timestamps for automatic updates.

use crate::auto_upgrade::{interval_secs, now_unix, AutoUpgradeConfig};
use crate::panel_phonehome::ensure_install_id;
use std::hash::{Hash, Hasher};

/// Upper bound on extra delay added to each interval (spread GitHub traffic).
pub const DEFAULT_JITTER_MAX_SECS: u64 = 45 * 60;

fn jitter_max_secs(interval: u64) -> u64 {
    std::env::var("CPN_AUTO_UPGRADE_JITTER_MAX_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or_else(|| {
            let quarter = interval / 4;
            quarter.clamp(15 * 60, DEFAULT_JITTER_MAX_SECS)
        })
}

/// Stable extra seconds for this install (derived from install UUID).
pub fn per_install_jitter_secs(interval: u64) -> u64 {
    let max = jitter_max_secs(interval);
    if max == 0 {
        return 0;
    }
    let seed = ensure_install_id().unwrap_or_else(|_| "cpn-local".to_string());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    interval.hash(&mut hasher);
    hasher.finish() % max
}

/// Interval plus per-install jitter (one wait between checks).
pub fn compute_wait_secs(cfg: &AutoUpgradeConfig) -> u64 {
    let base = interval_secs(cfg);
    let jitter = per_install_jitter_secs(base);
    base.saturating_add(jitter)
}

/// Set `next_check_at_unix` and record jitter for the UI.
pub fn schedule_next_check(cfg: &mut AutoUpgradeConfig, from_unix: u64) {
    let base = interval_secs(cfg);
    let jitter = per_install_jitter_secs(base);
    cfg.schedule_jitter_secs = jitter;
    let wait = base.saturating_add(jitter);
    cfg.next_check_at_unix = from_unix.saturating_add(wait);
}

/// Seconds until the scheduled check (0 if due now or past).
pub fn seconds_until_next_check(cfg: &AutoUpgradeConfig) -> u64 {
    let now = now_unix();
    cfg.next_check_at_unix.saturating_sub(now)
}

/// Ensure a future timestamp exists when automatic updates are enabled.
pub fn ensure_next_check_scheduled(cfg: &mut AutoUpgradeConfig) {
    if !cfg.enabled {
        return;
    }
    let now = now_unix();
    if cfg.next_check_at_unix > now.saturating_add(30) {
        return;
    }
    schedule_next_check(cfg, now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auto_upgrade::AutoUpgradeConfig;

    #[test]
    fn jitter_is_stable_for_same_interval() {
        let base = 6 * 60 * 60;
        let a = per_install_jitter_secs(base);
        let b = per_install_jitter_secs(base);
        assert_eq!(a, b);
        assert!(a <= jitter_max_secs(base));
    }

    #[test]
    fn wait_includes_interval() {
        let cfg = AutoUpgradeConfig::default();
        let wait = compute_wait_secs(&cfg);
        assert!(wait >= interval_secs(&cfg));
    }

    #[test]
    fn schedule_advances_next_check() {
        let mut cfg = AutoUpgradeConfig::default();
        cfg.enabled = true;
        let now = 1_700_000_000_u64;
        schedule_next_check(&mut cfg, now);
        assert!(cfg.next_check_at_unix > now);
        assert_eq!(
            cfg.next_check_at_unix,
            now + interval_secs(&cfg) + cfg.schedule_jitter_secs
        );
    }
}
