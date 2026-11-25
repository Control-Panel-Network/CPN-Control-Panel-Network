//! CPN-managed per-website Unix ownership (`user:group` = same account).
//!
//! # What "CPN user" means here
//!
//! The **website Unix account** CPN ensures for each primary site home under
//! `/home/<fqdn>` (not the panel login name, and never `root`). Subdomains
//! inherit the parent's Unix account. Owner and primary group are the same
//! account (`siteuser:siteuser`) so homes are not left `root:root`.
//!
//! OpenSSH jailed SFTP uses a separate root-owned jail under
//! `/var/lib/cpn/sftp-jails/` (see `panel_ops_sftp`); it must not reset the
//! website home back to `root:root`.

mod account;
mod heal;
mod perms;

pub use account::{
    ensure_primary_unix_user, find_unix_user_by_home, propose_unix_username,
    resolve_unix_user_for_site,
};
pub use heal::{heal_all_root_owned_site_homes, heal_site_ownership, root_owned_site_homes};
pub use perms::{
    OwnershipApplyReport, apply_site_ownership, ensure_owned_site_tree, mode_is_secure_home,
    verify_site_ownership,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn propose_username_shape() {
        let seed_a = 40u32 + 2;
        let name = propose_unix_username("example.com", seed_a);
        assert!(name.starts_with("examp"), "prefix mismatch");
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(name.len() <= 32);
        let seed_b = 3u32 + 4;
        let digit_start = propose_unix_username("123.example", seed_b);
        assert!(
            digit_start.chars().next().unwrap().is_ascii_alphabetic(),
            "digit-leading domain must get letter prefix"
        );
    }

    #[test]
    fn secure_home_modes() {
        assert!(mode_is_secure_home(0o755));
        assert!(mode_is_secure_home(0o750));
        assert!(mode_is_secure_home(0o700));
        assert!(!mode_is_secure_home(0o775));
        assert!(!mode_is_secure_home(0o757));
        assert!(!mode_is_secure_home(0o777));
    }

    #[test]
    fn ownership_report_equality() {
        let a = OwnershipApplyReport {
            unix_user: "site0001".into(),
            home: PathBuf::from("/home/example.com"),
            docroot: PathBuf::from("/home/example.com/public_html"),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn apply_rejects_root_account() {
        let err = apply_site_ownership(
            Path::new("/tmp/cpn-ownership-home"),
            Path::new("/tmp/cpn-ownership-doc"),
            "root",
        )
        .expect_err("root must be rejected");
        let lower = err.to_ascii_lowercase();
        assert!(lower.contains("root"), "reject-root message missing");
    }

    #[test]
    fn apply_rejects_empty_account() {
        let err = apply_site_ownership(
            Path::new("/tmp/cpn-ownership-home"),
            Path::new("/tmp/cpn-ownership-doc"),
            "  ",
        )
        .expect_err("empty must be rejected");
        let lower = err.to_ascii_lowercase();
        assert!(
            lower.contains("root") || lower.contains("empty"),
            "reject-empty message missing"
        );
    }
}
