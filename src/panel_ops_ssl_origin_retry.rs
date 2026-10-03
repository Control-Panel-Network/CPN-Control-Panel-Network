//! Bounded origin Let's Encrypt backup retries (page load + background loop).
//!
//! Never runs certbot on the HTTP worker. Backoff persists `last_error` and
//! `last_origin_retry_unix` so list loads cannot spam ACME.

use crate::panel_ops_ssl_issue::issue_origin_backup;
use crate::panel_ops_ssl_provider::SiteSslSettings;
use crate::panel_ops_ssl_public::{inspect_public_ssl, offers_origin_backup};
use crate::panel_website_resources::unix_now;
use crate::sites::list_sites;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

const MAX_SITES_PER_PASS: usize = 2;
const LOOP_SECS: u64 = 15 * 60;

fn in_flight() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

fn pass_lock() -> &'static Mutex<bool> {
    static LOCK: OnceLock<Mutex<bool>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(false))
}

/// Seconds to wait after the last attempt before another auto retry.
pub fn origin_retry_wait_secs(last_error: &str) -> u64 {
    let err = last_error.to_ascii_lowercase();
    if err.contains("rate limit") || err.contains("too many") {
        24 * 3600
    } else if err.contains("certbot was not found") || err.contains("certbot still missing") {
        5 * 60
    } else if err.trim().is_empty() {
        6 * 3600
    } else {
        15 * 60
    }
}

pub fn origin_retry_due(ssl: &SiteSslSettings, now: u64) -> bool {
    if ssl.last_origin_retry_unix == 0 {
        return true;
    }
    now.saturating_sub(ssl.last_origin_retry_unix) >= origin_retry_wait_secs(&ssl.last_error)
}

fn claim_pass() -> bool {
    let Ok(mut guard) = pass_lock().lock() else {
        return false;
    };
    if *guard {
        return false;
    }
    *guard = true;
    true
}

fn release_pass() {
    if let Ok(mut guard) = pass_lock().lock() {
        *guard = false;
    }
}

fn claim_domain(domain: &str) -> bool {
    let Ok(mut guard) = in_flight().lock() else {
        return false;
    };
    guard.insert(domain.to_string())
}

fn release_domain(domain: &str) {
    if let Ok(mut guard) = in_flight().lock() {
        guard.remove(domain);
    }
}

fn run_pass() {
    if !claim_pass() {
        return;
    }
    let now = unix_now();
    let sites = list_sites().unwrap_or_default();
    let mut attempted = 0usize;
    for site in sites {
        if attempted >= MAX_SITES_PER_PASS {
            break;
        }
        if !site.enabled {
            continue;
        }
        let view = inspect_public_ssl(&site.domain);
        if !offers_origin_backup(&site, &view) {
            continue;
        }
        if !origin_retry_due(&site.ssl, now) {
            continue;
        }
        if !claim_domain(&site.domain) {
            continue;
        }
        attempted += 1;
        let domain = site.domain.clone();
        let result = issue_origin_backup(&domain);
        release_domain(&domain);
        if let Err(err) = result {
            tracing_or_eprint(&domain, &err);
        }
    }
    release_pass();
}

fn tracing_or_eprint(domain: &str, err: &str) {
    let short: String = err.chars().take(180).collect();
    eprintln!("cpn origin-backup retry `{domain}`: {short}");
}

/// Kick a background pass from `/websites` or `/subdomains` without blocking HTML.
pub fn spawn_origin_backup_pass() {
    start_retry_loop_once();
    let _ = thread::Builder::new()
        .name("cpn-origin-le".into())
        .spawn(run_pass);
}

fn start_retry_loop_once() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = thread::Builder::new()
            .name("cpn-origin-le-loop".into())
            .spawn(|| {
                loop {
                    thread::sleep(Duration::from_secs(LOOP_SECS));
                    run_pass();
                }
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_ops_ssl_provider::SiteSslSettings;

    #[test]
    fn never_tried_is_due() {
        let ssl = SiteSslSettings::default();
        assert!(origin_retry_due(&ssl, 1_700_000_000));
    }

    #[test]
    fn recent_failure_is_not_due() {
        let mut ssl = SiteSslSettings::default();
        ssl.last_origin_retry_unix = 1_700_000_000;
        ssl.last_error = "certbot failed. connection refused".into();
        assert!(!origin_retry_due(&ssl, 1_700_000_000 + 60));
        assert!(origin_retry_due(&ssl, 1_700_000_000 + 16 * 60));
    }

    #[test]
    fn rate_limit_waits_a_day() {
        assert_eq!(
            origin_retry_wait_secs("ACME rate limit likely hit"),
            24 * 3600
        );
        assert_eq!(
            origin_retry_wait_secs("certbot was not found on PATH"),
            5 * 60
        );
    }
}
