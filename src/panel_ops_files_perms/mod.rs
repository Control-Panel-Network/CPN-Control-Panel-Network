//! Fix permissions for File Manager selections (owner:group + secure modes).
//!
//! # Policy
//!
//! - Website trees under the hosting home (`/home/<fqdn>/...`): resolve the CPN
//!   website Unix account and set `user:group` to that account (never `root`).
//! - Mailbox homes under `/home/<domain>/e-mails/<local>/...`: prefer the mailbox
//!   Unix account when one owns that mailbox home; otherwise the site account.
//! - Paths under the hosting home that are **not** a registered site (or mailbox)
//!   home are skipped (ops trees like shared script folders). Do not invent a
//!   website Unix owner for them.
//! - Non-site paths outside hosting homes: keep the existing non-root owner and
//!   only normalize modes. Root-owned non-site paths are refused (do not invent
//!   an owner and do not leave/force `root:root` for website content).
//! - Batch Fix permissions continues after per-item skips/failures and returns a
//!   summary (fixed N, skipped M) instead of aborting on the first bad path.
//! - Modes: directories `0755`, regular files `0644` (not group/other-writable).
//! - Folders: recursive owner + mode walk. Files: single path only.
//! - Always stays inside the File Manager jail; protected system paths refuse.

mod apply;
mod batch;
mod policy;

pub use batch::fix_permissions;
pub use policy::resolve_ownership_policy;

/// Soft cap so a runaway tree cannot hang the panel forever.
pub const MAX_FIX_ENTRIES: usize = 100_000;

pub(crate) const DIR_MODE: u32 = 0o755;
pub(crate) const FILE_MODE: u32 = 0o644;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipFixPolicy {
    /// Set owner and group to this Unix account (`user:user`).
    SetUser { user: String },
    /// Keep current owner/group; only normalize modes (non-site paths).
    ModesOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixPermissionsReport {
    pub fixed: usize,
    pub policy: OwnershipFixPolicy,
    pub recursive: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_ops_path::{join_child, resolve_under_jail};
    use crate::site_unix_owner::mode_is_secure_home;
    use apply::ensure_fixable;
    use batch::{MAX_SKIP_REASON_CHARS, format_skip_summary, shorten_skip_reason};
    use policy::{mailbox_home_for_path, path_is_under};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn mailbox_home_parser() {
        assert_eq!(
            mailbox_home_for_path(Path::new("/home/example.com/e-mails/info/Maildir/cur")),
            Some(PathBuf::from("/home/example.com/e-mails/info"))
        );
        assert_eq!(
            mailbox_home_for_path(Path::new("/home/example.com/public_html")),
            None
        );
    }

    #[test]
    fn path_under_helper() {
        assert!(path_is_under(
            Path::new("/home/example.com/public_html"),
            Path::new("/home/example.com")
        ));
        assert!(path_is_under(
            Path::new("/home/example.com"),
            Path::new("/home/example.com")
        ));
        assert!(!path_is_under(
            Path::new("/home/other.com"),
            Path::new("/home/example.com")
        ));
    }

    #[test]
    fn refuse_protected_and_home_root() {
        let jail = Path::new("/");
        assert!(ensure_fixable(Path::new("/etc"), jail).is_err());
        assert!(ensure_fixable(Path::new("/"), jail).is_err());
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: test-only; locked so concurrent tests do not race on this env var.
        unsafe {
            std::env::set_var("CPN_SITES_HOME", "/home");
        }
        assert!(ensure_fixable(Path::new("/home"), jail).is_err());
        unsafe {
            std::env::remove_var("CPN_SITES_HOME");
        }
    }

    #[test]
    fn empty_selection_errors() {
        let err = fix_permissions("/tmp", &[], Path::new("/")).unwrap_err();
        assert!(err.to_ascii_lowercase().contains("nothing"));
    }

    #[test]
    fn join_stays_in_jail() {
        let jail = Path::new("/home/example.com");
        let parent = resolve_under_jail("/home/example.com/public_html", jail).unwrap();
        assert!(join_child(&parent, "..", jail).is_err());
        assert!(join_child(&parent, "ok.txt", jail).is_ok());
    }

    #[test]
    fn secure_modes_match_site_policy() {
        assert!(mode_is_secure_home(DIR_MODE));
        assert!(mode_is_secure_home(FILE_MODE));
        assert!(!mode_is_secure_home(0o775));
        assert!(!mode_is_secure_home(0o666));
    }

    #[test]
    fn policy_enum_labels() {
        let set = OwnershipFixPolicy::SetUser {
            user: "site0001".into(),
        };
        assert!(matches!(set, OwnershipFixPolicy::SetUser { .. }));
        assert_eq!(OwnershipFixPolicy::ModesOnly, OwnershipFixPolicy::ModesOnly);
    }

    #[test]
    fn skip_summary_caps_items() {
        let skipped: Vec<String> = (0..12).map(|i| format!("item{i}: reason")).collect();
        let summary = format_skip_summary(&skipped);
        assert!(summary.contains("item0: reason"));
        assert!(summary.contains("+4 more"));
        assert!(!summary.contains("item11:"));
    }

    #[test]
    fn shorten_skip_reason_truncates() {
        let long = "x".repeat(400);
        let out = shorten_skip_reason("name", &long);
        assert!(out.starts_with("name: "));
        assert!(out.ends_with("..."));
        assert!(out.chars().count() <= MAX_SKIP_REASON_CHARS);
    }

    #[test]
    fn unregistered_under_home_is_skippable_error() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: test-only; locked so concurrent tests do not race on this env var.
        unsafe {
            std::env::set_var("CPN_SITES_HOME", "/home");
        }
        let err = resolve_ownership_policy(Path::new("/home/MAS_ChangeHub")).unwrap_err();
        unsafe {
            std::env::remove_var("CPN_SITES_HOME");
        }
        let lower = err.to_ascii_lowercase();
        assert!(lower.contains("not a registered"));
        assert!(lower.contains("mas_changehub"));
    }
}
