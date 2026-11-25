//! Throttled background webmail heals for panel page views.
//!
//! `heal_webmail_loopback_config` and `ensure_snappymail_operator_defaults` can spawn
//! `systemctl`, `chown -R`, SELinux tools, and restart PHP-FPM. Running them inline on
//! `GET /email/webmail` and `GET /email/password` made those pages take 7-30 seconds on a
//! loaded host. Page renders now call [`schedule_webmail_heal`] or
//! [`schedule_webmail_prefs_heal`], which start at most one thread per interval and never
//! block the caller.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Minimum gap between loopback heals started by page views.
const HEAL_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// Minimum gap between webmail preference heals started by page views.
const PREFS_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// One throttled background job: a single in-flight thread and a minimum start interval.
struct ThrottledJob {
    running: AtomicBool,
    last_start: Mutex<Option<Instant>>,
}

impl ThrottledJob {
    const fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            last_start: Mutex::new(None),
        }
    }

    /// Start `job` on a named thread when due. Returns immediately; `force` skips the interval.
    fn schedule(
        &'static self,
        name: &'static str,
        interval: Duration,
        force: bool,
        job: fn() -> Result<(), String>,
    ) {
        if self.running.swap(true, Ordering::AcqRel) {
            return;
        }
        let due = match self.last_start.lock() {
            Ok(mut guard) => {
                if force {
                    *guard = Some(Instant::now());
                    true
                } else {
                    claim_slot(Instant::now(), &mut guard, interval)
                }
            }
            Err(_) => true,
        };
        if !due {
            self.running.store(false, Ordering::Release);
            return;
        }
        let spawned = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let _guard = RunningGuard(&self.running);
                if let Err(error) = job() {
                    eprintln!("[{name}] [warn] background job failed: {error}");
                }
            });
        if spawned.is_err() {
            self.running.store(false, Ordering::Release);
        }
    }
}

/// Clears the running flag even if the job thread panics.
struct RunningGuard<'a>(&'a AtomicBool);

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// True when enough time passed since the previous start (and records the new start).
fn claim_slot(now: Instant, last: &mut Option<Instant>, interval: Duration) -> bool {
    match *last {
        Some(prev) if now.saturating_duration_since(prev) < interval => false,
        _ => {
            *last = Some(now);
            true
        }
    }
}

static LOOPBACK_JOB: ThrottledJob = ThrottledJob::new();
static PREFS_JOB: ThrottledJob = ThrottledJob::new();

/// Start a background loopback heal when one has not run recently. Returns immediately.
///
/// `force` skips the interval (used after the operator saves webmail settings) but still
/// keeps a single heal in flight.
pub fn schedule_webmail_heal(force: bool) {
    LOOPBACK_JOB.schedule(
        "cpn-webmail-heal",
        HEAL_INTERVAL,
        force,
        crate::install_webmail_heal::heal_webmail_loopback_config,
    );
}

/// Start a background SnappyMail/Tachyon preference heal when due. Returns immediately.
pub fn schedule_webmail_prefs_heal() {
    PREFS_JOB.schedule(
        "cpn-webmail-prefs",
        PREFS_INTERVAL,
        false,
        crate::install_snappymail_prefs::ensure_snappymail_operator_defaults,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_claim_is_due_then_throttled() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(claim_slot(t0, &mut last, Duration::from_secs(60)));
        assert!(!claim_slot(
            t0 + Duration::from_secs(5),
            &mut last,
            Duration::from_secs(60)
        ));
        assert!(claim_slot(
            t0 + Duration::from_secs(61),
            &mut last,
            Duration::from_secs(60)
        ));
    }

    static TEST_JOB: ThrottledJob = ThrottledJob::new();
    static TEST_RAN: AtomicBool = AtomicBool::new(false);

    fn test_job() -> Result<(), String> {
        TEST_RAN.store(true, Ordering::SeqCst);
        Ok(())
    }

    #[test]
    fn schedule_runs_job_off_thread_and_releases_slot() {
        TEST_JOB.schedule("cpn-test-job", Duration::from_secs(3600), false, test_job);
        for _ in 0..200 {
            if TEST_RAN.load(Ordering::SeqCst) && !TEST_JOB.running.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(TEST_RAN.load(Ordering::SeqCst));
        assert!(!TEST_JOB.running.load(Ordering::SeqCst));
        // Within the interval a second schedule does not start a new run.
        TEST_RAN.store(false, Ordering::SeqCst);
        TEST_JOB.schedule("cpn-test-job", Duration::from_secs(3600), false, test_job);
        std::thread::sleep(Duration::from_millis(50));
        assert!(!TEST_RAN.load(Ordering::SeqCst));
    }
}
