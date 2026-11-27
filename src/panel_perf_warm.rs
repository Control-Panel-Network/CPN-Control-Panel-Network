//! Background warm of short-TTL panel probes so the first operator click after
//! start/restart does not pay cold-cache disk and service detection costs.

use std::time::Duration;

/// Queue a low-priority warm after the HTTP server has bound (does not block boot).
pub fn spawn_startup_warm() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(3));
        if crate::panel_perf_cache::caches_bypassed() {
            return;
        }
        let _ = crate::panel_host_info::host_sidebar_info();
        let _ = crate::service_detect::detect_database();
        let _ = crate::service_detect::detect_web_server_label();
        let _ = crate::service_detect::detect_mail_service_label();
        let _ = crate::panel_dashboard::host_gauges_snapshot();
    });
}
