//! Throttled background webmail heal for panel page views.
//!
//! `heal_webmail_loopback_config` can spawn `systemctl`, `chown -R`, SELinux tools, and
//! restart PHP-FPM. Running it inline on `GET /email/webmail` made the page take 10-30
//! seconds on a loaded host. Page renders now only call [`schedule_webmail_heal`], which
//! starts at most one heal thread per interval and never blocks the caller.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Minimum gap between background heals started by page views.
const HEAL_INTERVAL: Duration = Duration::from_secs(30 * 60);

static HEAL_RUNNING: AtomicBool = AtomicBool::new(false);
static HEAL_LAST_START: Mutex<Option<Instant>> = Mutex::new(None);

/// Clears `HEAL_RUNNING` even if the heal thread panics.
struct RunningGuard;

impl Drop for RunningGuard {
    fn drop(&mut self) {
        HEAL_RUNNING.store(false, Ordering::Release);
    }
}

/// True when enough time passed since the previous heal started (and records the new start).
fn claim_heal_slot(now: Instant, last: &mut Option<Instant>, interval: Duration) -> bool {
    match *last {
        Some(prev) if now.saturating_duration_since(prev) < interval => false,
        _ => {
            *last = Some(now);
            true
        }
    }
}

/// Start a background heal when one has not run recently. Returns immediately.
///
/// `force` skips the interval (used after the operator saves webmail settings) but still
/// keeps a single heal in flight.
pub fn schedule_webmail_heal(force: bool) {
    if HEAL_RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let due = match HEAL_LAST_START.lock() {
        Ok(mut guard) => {
            if force {
                *guard = Some(Instant::now());
                true
            } else {
                claim_heal_slot(Instant::now(), &mut guard, HEAL_INTERVAL)
            }
        }
        Err(_) => true,
    };
    if !due {
        HEAL_RUNNING.store(false, Ordering::Release);
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("cpn-webmail-heal".into())
        .spawn(|| {
            let _guard = RunningGuard;
            if let Err(error) = crate::install_webmail_heal::heal_webmail_loopback_config() {
                eprintln!("[webmail_heal] [warn] background heal failed: {error}");
            }
        });
    if spawned.is_err() {
        HEAL_RUNNING.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_claim_is_due_then_throttled() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(claim_heal_slot(t0, &mut last, Duration::from_secs(60)));
        assert!(!claim_heal_slot(
            t0 + Duration::from_secs(5),
            &mut last,
            Duration::from_secs(60)
        ));
        assert!(claim_heal_slot(
            t0 + Duration::from_secs(61),
            &mut last,
            Duration::from_secs(60)
        ));
    }
}
