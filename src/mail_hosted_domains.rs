//! Hosted-domain local delivery for the CPN Postfix + Dovecot stack.
//!
//! Panel mailboxes are system users (`smoke@example.com` is system user `smoke`,
//! Maildir under `/home/smoke/Maildir`). Postfix only delivers locally for domains
//! in `mydestination`; for any other domain it looks up MX and relays to the
//! public mail host, which bounces with `User unknown`.
//!
//! This module publishes two Postfix hash tables owned by CPN and wires them into
//! `main.cf` through `virtual_alias_domains` and `virtual_alias_maps`:
//!
//! - `/etc/postfix/cpn_virtual_domains`: every domain that has an enabled local mailbox
//! - `/etc/postfix/cpn_virtual_aliases`: `user@domain  user@<myhostname>` per mailbox
//!
//! Only explicit mailbox addresses are accepted. Unknown recipients in a hosted
//! domain are rejected at SMTP time (`User unknown in virtual alias table`),
//! and domains without any local mailbox are never intercepted, so mail for
//! sites that use an external mail provider keeps relaying normally.

use crate::mail_accounts::{MailAccount, MailSmtpMode};
use std::collections::{BTreeMap, BTreeSet};

pub const VIRTUAL_DOMAINS_PATH: &str = "/etc/postfix/cpn_virtual_domains";
pub const VIRTUAL_ALIASES_PATH: &str = "/etc/postfix/cpn_virtual_aliases";

/// Fallback map type when `postconf default_database_type` is unavailable.
/// AlmaLinux 10 ships Postfix without the `hash` type (it uses `lmdb`).
const FALLBACK_MAP_TYPE: &str = "hash";

/// Desired hosted-domain routing derived from the panel mailbox registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostedMailPlan {
    /// Lowercase domains that are served locally.
    pub domains: BTreeSet<String>,
    /// Full address to system user (local part).
    pub mailboxes: BTreeMap<String, String>,
}

/// Outcome of a sync run (never contains secrets).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostedMailReport {
    pub domains: usize,
    pub mailboxes: usize,
    pub files_changed: bool,
    pub main_cf_changed: bool,
    pub skipped: Option<String>,
}

fn valid_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// Build the routing plan from the registry.
///
/// `user_exists` reports whether the system user for a local part exists, so a
/// mailbox that was never provisioned is not accepted at SMTP time and then
/// bounced later by the local agent. `excluded_domains` are domains already in
/// `mydestination` (Postfix warns when a domain is listed in both).
pub fn plan_from_accounts(
    accounts: &[MailAccount],
    user_exists: &dyn Fn(&str) -> bool,
    excluded_domains: &BTreeSet<String>,
) -> HostedMailPlan {
    let mut plan = HostedMailPlan::default();
    for account in accounts {
        if !account.enabled || account.smtp_mode != MailSmtpMode::PostfixLocal {
            continue;
        }
        let address = account.address.trim().to_ascii_lowercase();
        let Some((user, domain)) = address.split_once('@') else {
            continue;
        };
        if !valid_domain(domain) {
            continue;
        }
        let Ok(system_user) = crate::panel_ops_mailbox_provision::local_part(&address) else {
            continue;
        };
        if user != system_user || !user_exists(&system_user) {
            continue;
        }
        if excluded_domains.contains(domain) {
            // Already a local destination; delivery works without virtual maps.
            continue;
        }
        plan.domains.insert(domain.to_string());
        plan.mailboxes.insert(address.clone(), system_user);
    }
    plan
}

/// Content of the `virtual_alias_domains` hash source.
pub fn render_virtual_domains(plan: &HostedMailPlan) -> String {
    let mut out =
        String::from("# Managed by CPN (hosted-domain local delivery). Do not edit by hand.\n");
    for domain in &plan.domains {
        out.push_str(&format!("{domain}\tOK\n"));
    }
    out
}

/// Content of the `virtual_alias_maps` hash source.
///
/// Targets are qualified with `target_host` (the local `myhostname`, always in
/// `mydestination`) so the result is delivered by the local agent and never
/// re-enters the virtual alias domain.
pub fn render_virtual_aliases(plan: &HostedMailPlan, target_host: &str) -> String {
    let mut out =
        String::from("# Managed by CPN (hosted-domain local delivery). Do not edit by hand.\n");
    for (address, user) in &plan.mailboxes {
        if target_host.is_empty() {
            out.push_str(&format!("{address}\t{user}\n"));
        } else {
            out.push_str(&format!("{address}\t{user}@{target_host}\n"));
        }
    }
    out
}

/// Map type reference such as `lmdb:/etc/postfix/cpn_virtual_domains`.
pub fn map_reference(map_type: &str, path: &str) -> String {
    format!("{map_type}:{path}")
}

/// Add `reference` to an existing Postfix list value unless already present.
///
/// Entries that point at the same file with a different map type (for example a
/// leftover `hash:` reference after the host switched to `lmdb:`) are replaced.
pub fn merge_list_value(existing: &str, reference: &str) -> String {
    let exact = existing
        .split(|c: char| c == ',' || c.is_whitespace())
        .any(|part| part == reference);
    if exact {
        return existing.trim().to_string();
    }
    let path = reference.split_once(':').map_or(reference, |(_, p)| p);
    let mut parts: Vec<String> = Vec::new();
    for part in existing
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
    {
        let same_file = part.split_once(':').is_some_and(|(_, p)| p == path);
        if same_file {
            continue;
        }
        parts.push(part.to_string());
    }
    parts.push(reference.to_string());
    parts.join(", ")
}

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::process::{Command, Stdio};

    fn postconf_value(args: &[&str]) -> String {
        Command::new("postconf")
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .unwrap_or_default()
    }

    fn mydestination_domains() -> BTreeSet<String> {
        postconf_value(&["-hx", "mydestination"])
            .split(|c: char| c == ',' || c.is_whitespace())
            .map(|d| d.trim().to_ascii_lowercase())
            .filter(|d| !d.is_empty())
            .collect()
    }

    /// Postfix map type for this host (`lmdb` on AlmaLinux 10, `hash` on older releases).
    fn map_type() -> String {
        let value = postconf_value(&["-h", "default_database_type"]);
        let known = !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if known {
            value
        } else {
            FALLBACK_MAP_TYPE.to_string()
        }
    }

    fn map_db_exists(map_type: &str, path: &str) -> bool {
        let suffix = if map_type == "lmdb" { "lmdb" } else { "db" };
        Path::new(&format!("{path}.{suffix}")).exists()
    }

    fn write_map_if_changed(map_type: &str, path: &str, body: &str) -> Result<bool, String> {
        let current = fs::read_to_string(path).unwrap_or_default();
        let db_present = map_db_exists(map_type, path);
        if current == body && db_present {
            return Ok(false);
        }
        fs::write(path, body).map_err(|e| format!("Could not write {path}: {e}"))?;
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o644));
        }
        let output = Command::new("postmap")
            .arg(map_reference(map_type, path))
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("postmap failed to start for {path}: {e}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            let detail = detail.trim().lines().last().unwrap_or("").trim();
            return Err(format!("postmap ({map_type}) failed for {path}: {detail}"));
        }
        let _ = Command::new("restorecon")
            .args(["-F", path])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        Ok(true)
    }

    fn set_postconf_list(key: &str, reference: &str) -> Result<bool, String> {
        let existing = postconf_value(&["-n", "-h", key]);
        let merged = merge_list_value(&existing, reference);
        if merged == existing {
            return Ok(false);
        }
        let status = Command::new("postconf")
            .args(["-e", &format!("{key}={merged}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("postconf failed to start for {key}: {e}"))?;
        if !status.success() {
            return Err(format!("postconf -e {key} failed"));
        }
        Ok(true)
    }

    fn reload_postfix() {
        let reloaded = Command::new("postfix")
            .arg("reload")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !reloaded {
            let _ = Command::new("systemctl")
                .args(["reload", "postfix"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    pub fn sync() -> Result<HostedMailReport, String> {
        if !Path::new("/etc/postfix/main.cf").exists() {
            return Ok(HostedMailReport {
                skipped: Some("Postfix is not installed".into()),
                ..HostedMailReport::default()
            });
        }
        let accounts = crate::mail_accounts::list_accounts();
        let excluded = mydestination_domains();
        let plan = plan_from_accounts(
            &accounts,
            &crate::panel_ops_mailbox_provision::user_exists,
            &excluded,
        );
        let domains_present = Path::new(VIRTUAL_DOMAINS_PATH).exists();
        if plan.domains.is_empty() && !domains_present {
            return Ok(HostedMailReport {
                skipped: Some("No hosted mailboxes to route".into()),
                ..HostedMailReport::default()
            });
        }
        let target_host = postconf_value(&["-h", "myhostname"]);
        let mut report = HostedMailReport {
            domains: plan.domains.len(),
            mailboxes: plan.mailboxes.len(),
            ..HostedMailReport::default()
        };
        // Aliases first so a domain is never accepted without its mailbox table.
        let map_type = map_type();
        report.files_changed |= write_map_if_changed(
            &map_type,
            VIRTUAL_ALIASES_PATH,
            &render_virtual_aliases(&plan, &target_host),
        )?;
        report.files_changed |= write_map_if_changed(
            &map_type,
            VIRTUAL_DOMAINS_PATH,
            &render_virtual_domains(&plan),
        )?;
        report.main_cf_changed |= set_postconf_list(
            "virtual_alias_maps",
            &map_reference(&map_type, VIRTUAL_ALIASES_PATH),
        )?;
        report.main_cf_changed |= set_postconf_list(
            "virtual_alias_domains",
            &map_reference(&map_type, VIRTUAL_DOMAINS_PATH),
        )?;
        if report.files_changed || report.main_cf_changed {
            reload_postfix();
        }
        Ok(report)
    }
}

/// Write the hosted-domain tables from the panel registry and wire them into Postfix.
///
/// Safe to call repeatedly: no-op when nothing changed, when Postfix is absent,
/// or when no local mailboxes exist yet.
pub fn sync_hosted_mail_delivery() -> Result<HostedMailReport, String> {
    #[cfg(unix)]
    {
        unix_impl::sync()
    }
    #[cfg(not(unix))]
    {
        Ok(HostedMailReport {
            skipped: Some("Hosted mail delivery is Linux-only".into()),
            ..HostedMailReport::default()
        })
    }
}

/// Best-effort sync that logs a short non-secret line on failure.
pub fn sync_hosted_mail_delivery_logged(context: &str) {
    match sync_hosted_mail_delivery() {
        Ok(report) => {
            if report.files_changed || report.main_cf_changed {
                eprintln!(
                    "cpn-installer: hosted mail delivery synced ({context}): {} domain(s), {} mailbox(es)",
                    report.domains, report.mailboxes
                );
            }
        }
        Err(error) => {
            eprintln!("cpn-installer: hosted mail delivery sync failed ({context}): {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smtp_settings::SmtpTlsMode;

    fn account(address: &str, enabled: bool, mode: MailSmtpMode) -> MailAccount {
        MailAccount {
            id: format!("id-{address}"),
            address: address.into(),
            domain: String::new(),
            enabled,
            smtp_mode: mode,
            smtp_host: String::new(),
            smtp_port: 587,
            smtp_tls: SmtpTlsMode::Starttls,
            smtp_username: String::new(),
            smtp_password: String::new(),
            mailbox_password: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
        }
    }

    #[test]
    fn plan_includes_enabled_local_mailboxes_only() {
        let accounts = vec![
            account("smoke@newstargeted.com", true, MailSmtpMode::PostfixLocal),
            account("off@newstargeted.com", false, MailSmtpMode::PostfixLocal),
            account("ext@external.example", true, MailSmtpMode::External),
            account("ghost@ghost.example", true, MailSmtpMode::PostfixLocal),
        ];
        let plan = plan_from_accounts(
            &accounts,
            &|user| user == "smoke" || user == "off",
            &BTreeSet::new(),
        );
        assert_eq!(plan.domains.len(), 1);
        assert!(plan.domains.contains("newstargeted.com"));
        assert_eq!(
            plan.mailboxes
                .get("smoke@newstargeted.com")
                .map(String::as_str),
            Some("smoke")
        );
        assert!(!plan.mailboxes.contains_key("off@newstargeted.com"));
        assert!(!plan.mailboxes.contains_key("ghost@ghost.example"));
    }

    #[test]
    fn plan_skips_domains_already_in_mydestination() {
        let accounts = vec![account(
            "a@host.local.example",
            true,
            MailSmtpMode::PostfixLocal,
        )];
        let mut excluded = BTreeSet::new();
        excluded.insert("host.local.example".to_string());
        let plan = plan_from_accounts(&accounts, &|_| true, &excluded);
        assert!(plan.domains.is_empty());
        assert!(plan.mailboxes.is_empty());
    }

    #[test]
    fn plan_rejects_unsafe_addresses() {
        let accounts = vec![
            account("bad user@example.com", true, MailSmtpMode::PostfixLocal),
            account("x@bad domain.com", true, MailSmtpMode::PostfixLocal),
            account("y@nodot", true, MailSmtpMode::PostfixLocal),
            account("Mixed@Example.COM", true, MailSmtpMode::PostfixLocal),
        ];
        let plan = plan_from_accounts(&accounts, &|_| true, &BTreeSet::new());
        assert_eq!(plan.mailboxes.len(), 1);
        assert!(plan.mailboxes.contains_key("mixed@example.com"));
    }

    #[test]
    fn render_tables_are_stable_and_qualified() {
        let accounts = vec![
            account("b@two.example", true, MailSmtpMode::PostfixLocal),
            account("a@one.example", true, MailSmtpMode::PostfixLocal),
        ];
        let plan = plan_from_accounts(&accounts, &|_| true, &BTreeSet::new());
        let domains = render_virtual_domains(&plan);
        assert!(domains.contains("one.example\tOK\n"));
        assert!(domains.find("one.example").unwrap() < domains.find("two.example").unwrap());
        let aliases = render_virtual_aliases(&plan, "mx.lab.local");
        assert!(aliases.contains("a@one.example\ta@mx.lab.local\n"));
        let bare = render_virtual_aliases(&plan, "");
        assert!(bare.contains("a@one.example\ta\n"));
    }

    #[test]
    fn merge_list_value_is_idempotent_and_preserves_existing() {
        let domains = map_reference("lmdb", VIRTUAL_DOMAINS_PATH);
        assert_eq!(merge_list_value("", &domains), domains);
        let merged = merge_list_value("hash:/etc/postfix/virtual", &domains);
        assert_eq!(merged, format!("hash:/etc/postfix/virtual, {domains}"));
        assert_eq!(merge_list_value(&merged, &domains), merged);
    }

    #[test]
    fn merge_list_value_replaces_stale_map_type_for_same_file() {
        let lmdb = map_reference("lmdb", VIRTUAL_ALIASES_PATH);
        let stale = map_reference("hash", VIRTUAL_ALIASES_PATH);
        let merged = merge_list_value(&format!("hash:/etc/postfix/virtual, {stale}"), &lmdb);
        assert_eq!(merged, format!("hash:/etc/postfix/virtual, {lmdb}"));
    }
}
