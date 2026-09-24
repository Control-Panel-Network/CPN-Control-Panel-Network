//! Product uninstall for CPN (SSH/CLI): stop panel, remove package, clean state.
//!
//! Default keeps website docroots and host MariaDB/OLS packages. Destructive flags
//! are explicit: `--purge-all`, `--purge-sites`, `--purge-stack`. Use `--keep-data`
//! to leave `/var/lib/cpn` and `/etc/cpn`. Confirmation required unless `--yes`.

use crate::cli_common::{confirm_delete, require_root_for_mutation};
use crate::cli_uninstall_ops::{
    self as ops, ETC_CPN, LIB_CPN, PACKAGE_NAME, PROFILE_MOTD, WEBMAIL_DATA, WEBMAIL_OPT,
};
use crate::panel_service::UNIT_NAME;
use crate::paths;
use crate::sites::{self, SiteRecord};
use std::path::Path;

/// Parsed uninstall options (shared by `cpn-installer --uninstall` and `cpn uninstall`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallOptions {
    pub yes: bool,
    /// Leave `/var/lib/cpn` and `/etc/cpn` (and skip CPN Docker teardown).
    pub keep_data: bool,
    /// Also tear down CPN-managed Docker volumes and webmail trees.
    pub purge_all: bool,
    /// Delete website homes listed in the site registry (never default).
    pub purge_sites: bool,
    /// Remove common host stack packages (MariaDB / OpenLiteSpeed) when present.
    pub purge_stack: bool,
    /// Print steps only; do not mutate the host.
    pub dry_run: bool,
}

impl UninstallOptions {
    pub fn from_args(args: &[String]) -> Self {
        Self {
            yes: args.iter().any(|a| a == "--yes" || a == "-y"),
            keep_data: args.iter().any(|a| a == "--keep-data"),
            purge_all: args.iter().any(|a| a == "--purge-all"),
            purge_sites: args.iter().any(|a| a == "--purge-sites"),
            purge_stack: args.iter().any(|a| a == "--purge-stack"),
            dry_run: args.iter().any(|a| a == "--dry-run"),
        }
    }
}

/// Human-readable warning lines for the operator prompt (no secrets).
pub fn warning_lines(opts: &UninstallOptions, sites: &[SiteRecord]) -> Vec<String> {
    let data_dir = paths::default_data_dir();
    let mut lines = vec![
        "WARNING: This will uninstall CPN Control Panel Network from this host.".into(),
        "This action is destructive. Review carefully before confirming.".into(),
        String::new(),
        "Will remove:".into(),
        format!("  - systemd unit {UNIT_NAME} (stop + disable)"),
        format!("  - package `{PACKAGE_NAME}` via dnf/yum/apt when installed"),
        "  - binaries: /usr/bin/cpn, /usr/bin/cpn-installer, leftover /usr/local/bin/cpn*".into(),
        format!("  - packaged MOTD/helpers: {PROFILE_MOTD}, {LIB_CPN}"),
    ];
    if opts.keep_data {
        lines.push(format!(
            "  - (kept) data dir {} and {ETC_CPN} [--keep-data]",
            data_dir.display()
        ));
        lines.push("  - (kept) CPN-managed Docker stacks [--keep-data]".into());
    } else {
        lines.push(format!(
            "  - state: {} and {ETC_CPN} (config, MFA keys, site registry, secrets)",
            data_dir.display()
        ));
        lines.push(
            "  - CPN-managed Docker: compose under <data>/docker and containers labeled com.cpn.managed=1"
                .into(),
        );
        if opts.purge_all {
            lines.push(format!(
                "  - [--purge-all] Docker volumes for those compose projects; {WEBMAIL_DATA}; {WEBMAIL_OPT}"
            ));
        } else {
            lines.push(
                "  - (kept) Docker volumes for CPN compose (pass --purge-all to remove volumes)"
                    .into(),
            );
            lines.push(format!(
                "  - (kept) {WEBMAIL_DATA} and {WEBMAIL_OPT} unless --purge-all"
            ));
        }
    }
    if opts.purge_sites {
        if sites.is_empty() {
            lines.push("  - [--purge-sites] no site registry entries found".into());
        } else {
            lines.push(format!(
                "  - [--purge-sites] website homes for {} registered site(s):",
                sites.len()
            ));
            for site in sites.iter().take(20) {
                lines.push(format!(
                    "      * {} -> {}",
                    site.domain,
                    ops::site_home_guess(site).display()
                ));
            }
            if sites.len() > 20 {
                lines.push(format!("      * ... and {} more", sites.len() - 20));
            }
        }
    } else {
        lines.push(
            "  - (kept) website document roots under /home/<domain> (pass --purge-sites to delete)"
                .into(),
        );
    }
    if opts.purge_stack {
        lines.push(
            "  - [--purge-stack] attempt to remove host MariaDB / OpenLiteSpeed packages when present"
                .into(),
        );
    } else {
        lines.push(
            "  - (kept) host MariaDB / OpenLiteSpeed packages (pass --purge-stack to remove)"
                .into(),
        );
    }
    lines.push(String::new());
    lines.push(
        "Will NOT touch: unlabeled Docker containers, unrelated OS packages, or sites unless flagged."
            .into(),
    );
    if opts.dry_run {
        lines.push("Mode: --dry-run (no changes will be applied).".into());
    }
    lines
}

/// Run product uninstall. Returns process exit code (0 ok, 1 failure).
pub fn run(opts: UninstallOptions) -> i32 {
    if let Err(error) = require_root_for_mutation() {
        eprintln!("error: {error}");
        return 1;
    }

    let sites = sites::list_sites().unwrap_or_default();
    for line in warning_lines(&opts, &sites) {
        eprintln!("{line}");
    }

    if let Err(error) = confirm_delete(
        "Type yes or y to uninstall CPN (no or n to abort).",
        opts.yes,
    ) {
        eprintln!("error: {error}");
        return 1;
    }

    if opts.purge_sites && !sites.is_empty() {
        if let Err(error) = confirm_delete(
            "SECOND CONFIRMATION: delete website document roots listed above?",
            opts.yes,
        ) {
            eprintln!("error: {error}");
            return 1;
        }
    }

    let data_dir = paths::default_data_dir();
    let mut failed = false;

    ops::stop_and_disable_unit(opts.dry_run);

    if !opts.keep_data {
        ops::tear_down_cpn_docker(&data_dir, opts.purge_all, opts.dry_run);
    }

    if let Err(error) = ops::remove_package(opts.dry_run) {
        ops::log_step(&format!("warning: package remove: {error}"));
        failed = true;
    }

    if let Err(error) = ops::remove_binaries_and_helpers(opts.dry_run) {
        eprintln!("error: {error}");
        failed = true;
    }

    if !opts.keep_data {
        if let Err(error) = ops::remove_path(&data_dir, opts.dry_run) {
            eprintln!("error: {error}");
            failed = true;
        }
        if let Err(error) = ops::remove_path(Path::new(ETC_CPN), opts.dry_run) {
            eprintln!("error: {error}");
            failed = true;
        }
        if opts.purge_all {
            let _ = ops::remove_path(Path::new(WEBMAIL_DATA), opts.dry_run);
            let _ = ops::remove_path(Path::new(WEBMAIL_OPT), opts.dry_run);
        }
    }

    if opts.purge_sites {
        if let Err(error) = ops::purge_site_homes(&sites, opts.dry_run) {
            eprintln!("error: {error}");
            failed = true;
        }
    }

    if opts.purge_stack {
        ops::purge_stack_packages(opts.dry_run);
    }

    ops::daemon_reload(opts.dry_run);

    if failed {
        eprintln!("error: uninstall finished with errors (see steps above)");
        return 1;
    }

    if opts.dry_run {
        eprintln!("dry-run complete: no changes applied");
    } else {
        eprintln!("uninstall complete");
        eprintln!(
            "note: website docroots and host MariaDB/OLS were kept unless you passed purge flags"
        );
        eprintln!("tip: run `hash -r` in open shells so PATH drops removed binaries");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_site(domain: &str, docroot: &str) -> SiteRecord {
        SiteRecord {
            schema_version: 1,
            domain: domain.into(),
            owner: "admin".into(),
            docroot: docroot.into(),
            enabled: true,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
            ssl: Default::default(),
            internal_ip: None,
            owner_suspend_message: String::new(),
            suspended_by: None,
            php_version: None,
            aliases: vec![],
        }
    }

    #[test]
    fn parse_flags() {
        let args: Vec<String> = ["--uninstall", "--yes", "--purge-all", "--dry-run"]
            .into_iter()
            .map(String::from)
            .collect();
        let opts = UninstallOptions::from_args(&args);
        assert!(opts.yes);
        assert!(opts.purge_all);
        assert!(opts.dry_run);
        assert!(!opts.keep_data);
        assert!(!opts.purge_sites);
    }

    #[test]
    fn warning_mentions_kept_sites_by_default() {
        let lines = warning_lines(&UninstallOptions::default(), &[]);
        let joined = lines.join("\n");
        assert!(joined.contains("website document roots"));
        assert!(joined.contains("--purge-sites"));
        assert!(joined.contains("MariaDB"));
        assert!(!joined.contains("password"));
    }

    #[test]
    fn warning_lists_purge_sites() {
        let site = sample_site("example.com", "/home/example.com/public_html");
        let opts = UninstallOptions {
            purge_sites: true,
            ..UninstallOptions::default()
        };
        let lines = warning_lines(&opts, &[site]);
        let joined = lines.join("\n");
        assert!(joined.contains("example.com"));
        assert!(joined.contains("/home/example.com"));
    }

    #[test]
    fn site_home_from_public_html() {
        let site = sample_site("a.test", "/home/a.test/public_html");
        assert_eq!(ops::site_home_guess(&site), PathBuf::from("/home/a.test"));
    }
}
