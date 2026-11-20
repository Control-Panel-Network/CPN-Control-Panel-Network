//! Detect existing Postfix builds (AppStream, postfix3, vendor) before package install.

use crate::apps_pkg::rpm_or_dpkg_installed;
use crate::postfix_fallback::postfix_is_ready;

/// True when a local MTA is already installed or listening (do not `dnf install postfix`).
pub fn postfix_mta_present() -> bool {
    rpm_or_dpkg_installed(&["postfix", "postfix3"]) || postfix_is_ready()
}

/// Package names to pass to dnf when a fresh MTA install is required.
pub fn postfix_dnf_package_candidates() -> &'static [&'static str] {
    &["postfix", "postfix3"]
}