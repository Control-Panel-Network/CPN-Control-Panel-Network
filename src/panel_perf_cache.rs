//! Short-TTL in-process caches for panel page renders.
//!
//! Keeps expensive host probes (disk walks, openssl, MariaDB size, hostname,
//! mail health) off the critical path when operators move between pages.
//! Private HTML is never CDN-cached. Entries live only in the panel process.
//!
//! Tests set `CPN_RESERVED_USERNAMES_OFFLINE` and bypass these caches so quota
//! and site mutations stay immediately visible.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Entry<V> {
    at: Instant,
    value: V,
}

/// Tiny mutex map with TTL. Callers clone values out so the lock stays short.
pub struct TtlCache<K, V> {
    ttl: Duration,
    max_entries: usize,
    inner: Mutex<HashMap<K, Entry<V>>>,
}

impl<K, V> TtlCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            ttl,
            max_entries,
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        if cache_bypass() {
            return None;
        }
        let Ok(guard) = self.inner.lock() else {
            return None;
        };
        let entry = guard.get(key)?;
        if entry.at.elapsed() > self.ttl {
            return None;
        }
        Some(entry.value.clone())
    }

    pub fn insert(&self, key: K, value: V) {
        if cache_bypass() {
            return;
        }
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        if guard.len() >= self.max_entries && !guard.contains_key(&key) {
            guard.clear();
        }
        guard.insert(
            key,
            Entry {
                at: Instant::now(),
                value,
            },
        );
    }

    pub fn invalidate(&self, key: &K) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.remove(key);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.clear();
        }
    }
}

fn cache_bypass() -> bool {
    std::env::var_os("CPN_RESERVED_USERNAMES_OFFLINE").is_some()
}

/// Shared process caches used by panel HTML routes.
pub struct PanelPerfCaches {
    pub host_sidebar: TtlCache<&'static str, crate::panel_host_info::HostSidebarInfo>,
    pub site_disk: TtlCache<String, u64>,
    pub disk_mb: TtlCache<String, u64>,
    pub site_homes: TtlCache<&'static str, Vec<(String, PathBuf)>>,
    pub package_usage: TtlCache<String, crate::packages::PackageUsage>,
    pub db_disk: TtlCache<String, u64>,
    pub ssl_insight: TtlCache<String, crate::panel_ops_ssl_inspect::SslCertInsight>,
    pub mail_stack: TtlCache<&'static str, crate::apps_email::MailStackPublic>,
}

impl PanelPerfCaches {
    fn new() -> Self {
        Self {
            host_sidebar: TtlCache::new(Duration::from_secs(45), 4),
            site_disk: TtlCache::new(Duration::from_secs(90), 512),
            disk_mb: TtlCache::new(Duration::from_secs(90), 64),
            site_homes: TtlCache::new(Duration::from_secs(30), 4),
            package_usage: TtlCache::new(Duration::from_secs(45), 64),
            db_disk: TtlCache::new(Duration::from_secs(60), 64),
            ssl_insight: TtlCache::new(Duration::from_secs(300), 256),
            mail_stack: TtlCache::new(Duration::from_secs(20), 4),
        }
    }
}

pub fn caches() -> &'static PanelPerfCaches {
    static CACHES: OnceLock<PanelPerfCaches> = OnceLock::new();
    CACHES.get_or_init(PanelPerfCaches::new)
}

/// Drop short-TTL panel render caches (after mail start, cert issue, tests).
pub fn clear_all_panel_perf_caches() {
    let c = caches();
    c.host_sidebar.clear();
    c.site_disk.clear();
    c.disk_mb.clear();
    c.site_homes.clear();
    c.package_usage.clear();
    c.db_disk.clear();
    c.ssl_insight.clear();
    c.mail_stack.clear();
}

/// True when unit tests asked caches to stay out of the way.
pub fn caches_bypassed() -> bool {
    cache_bypass()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        crate::account::DATA_DIR_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn ttl_cache_expires() {
        let _guard = lock_env();
        let cache: TtlCache<u32, u32> = TtlCache::new(Duration::from_millis(30), 8);
        // SAFETY: exclusive DATA_DIR_TEST_LOCK held; no other test reads this var.
        unsafe {
            std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
        }
        cache.insert(1, 42);
        assert_eq!(cache.get(&1), Some(42));
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(cache.get(&1), None);
    }

    #[test]
    fn offline_test_env_bypasses_cache() {
        let _guard = lock_env();
        let cache: TtlCache<u32, u32> = TtlCache::new(Duration::from_secs(30), 8);
        // SAFETY: exclusive DATA_DIR_TEST_LOCK held; restored before the lock drops.
        unsafe {
            std::env::set_var("CPN_RESERVED_USERNAMES_OFFLINE", "1");
        }
        cache.insert(7, 9);
        assert_eq!(cache.get(&7), None);
        unsafe {
            std::env::remove_var("CPN_RESERVED_USERNAMES_OFFLINE");
        }
    }
}
