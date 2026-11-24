//! Short-TTL cache for the firewall status probe used by the firewall manager page.
//!
//! `firewall_status()` shells out several times (`systemctl`, `firewall-cmd --state`,
//! `--list-services`, `--list-all`). The manager page used to call it three times per
//! render, so every tab click paid for a dozen subprocesses. The page now reads this
//! cache once per render and every mutating action invalidates it.

use crate::panel_ops_security::{FirewallStatus, firewall_status};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(10);

static CACHE: Mutex<Option<(Instant, FirewallStatus)>> = Mutex::new(None);

/// Cached `firewall_status()`; bypassed in unit tests so mutations stay visible.
pub fn firewall_status_cached() -> FirewallStatus {
    if !crate::panel_perf_cache::caches_bypassed()
        && let Ok(guard) = CACHE.lock()
        && let Some((at, st)) = guard.as_ref()
        && at.elapsed() < TTL
    {
        return st.clone();
    }
    let fresh = firewall_status();
    if !crate::panel_perf_cache::caches_bypassed()
        && let Ok(mut guard) = CACHE.lock()
    {
        *guard = Some((Instant::now(), fresh.clone()));
    }
    fresh
}

/// Drop the cached probe (call after start, stop, reload or any rule change).
pub fn invalidate_firewall_status_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidate_clears_cached_entry() {
        invalidate_firewall_status_cache();
        let guard = CACHE.lock().unwrap();
        assert!(guard.is_none());
    }
}
