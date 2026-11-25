//! OpenDKIM milter: sync KeyTable/SigningTable from the CPN DKIM store and wire Postfix.

use crate::apps_pkg::{enable_now, install_packages_dnf_or_apt, rpm_or_dpkg_installed};
use crate::panel_ops_dkim_keys::{
    dkim_root, ensure_dkim_for_domain, list_dkim_domain_rows, private_key_path, selector,
};
use crate::service_detect::systemd_unit_file_exists;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const OPENDKIM_DIR: &str = "/etc/opendkim";
const KEY_TABLE: &str = "/etc/opendkim/KeyTable";
const SIGNING_TABLE: &str = "/etc/opendkim/SigningTable";
const TRUSTED_HOSTS: &str = "/etc/opendkim/TrustedHosts";
/// Postfix milter target (`inet:host:port`).
const POSTFIX_MILTER: &str = crate::panel_ops_mail_milters::OPENDKIM_POSTFIX_MILTER;
const CPN_BEGIN: &str = "# BEGIN CPN-managed OpenDKIM entries";
const CPN_END: &str = "# END CPN-managed OpenDKIM entries";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpendkimDomainEntry {
    pub domain: String,
    pub selector: String,
    pub private_key: PathBuf,
}

pub fn keytable_line(entry: &OpendkimDomainEntry) -> String {
    format!(
        "{sel}._domainkey.{domain} {domain}:{sel}:{key}",
        sel = entry.selector,
        domain = entry.domain,
        key = entry.private_key.display()
    )
}

pub fn signing_table_line(entry: &OpendkimDomainEntry) -> String {
    format!(
        "*@{domain} {sel}._domainkey.{domain}",
        domain = entry.domain,
        sel = entry.selector
    )
}

/// Replace or insert the CPN-managed block inside a table file body.
pub fn upsert_cpn_managed_block(existing: &str, managed_lines: &[String]) -> String {
    let block = {
        let mut parts = vec![CPN_BEGIN.to_string()];
        parts.extend(managed_lines.iter().cloned());
        parts.push(CPN_END.to_string());
        parts.join("\n")
    };
    let start = existing.find(CPN_BEGIN);
    let end = existing.find(CPN_END).map(|i| i + CPN_END.len());
    let mut out = if let (Some(s), Some(e)) = (start, end) {
        let mut buf = String::new();
        buf.push_str(existing[..s].trim_end());
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(&block);
        let rest = existing[e..].trim_start_matches(['\r', '\n']);
        if !rest.is_empty() {
            buf.push('\n');
            buf.push_str(rest.trim_end());
            buf.push('\n');
        } else if !buf.ends_with('\n') {
            buf.push('\n');
        }
        buf
    } else {
        let mut buf = existing.trim_end().to_string();
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(&block);
        buf.push('\n');
        buf
    };
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// True when an existing SigningTable already maps this domain (any selector).
pub fn signing_table_has_domain(existing: &str, domain: &str) -> bool {
    let needle = format!("@{}", domain.trim().to_ascii_lowercase());
    existing.lines().any(|line| {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            return false;
        }
        t.to_ascii_lowercase().contains(&needle)
    })
}

/// Install path under `/etc/opendkim/keys/<domain>/` (SELinux-friendly vs `/var/lib/cpn/dkim`).
pub fn opendkim_key_install_path(domain: &str, sel: &str) -> PathBuf {
    PathBuf::from(OPENDKIM_DIR)
        .join("keys")
        .join(domain)
        .join(format!("{sel}.private"))
}

/// Copy CPN store private keys into `/etc/opendkim/keys` when possible; fall back to store path.
pub fn collect_cpn_opendkim_entries() -> Vec<OpendkimDomainEntry> {
    let sel = selector().to_string();
    list_dkim_domain_rows()
        .into_iter()
        .filter(|row| row.has_key)
        .filter_map(|row| {
            let store_key = private_key_path(&row.domain).ok()?;
            if !store_key.is_file() {
                return None;
            }
            let install = opendkim_key_install_path(&row.domain, &sel);
            if let Some(parent) = install.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let private_key = match fs::copy(&store_key, &install) {
                Ok(_) => install,
                Err(_) => store_key,
            };
            Some(OpendkimDomainEntry {
                domain: row.domain,
                selector: sel.clone(),
                private_key,
            })
        })
        .collect()
}

fn ensure_opendkim_dirs() -> Result<(), String> {
    fs::create_dir_all(OPENDKIM_DIR).map_err(|e| format!("Cannot create {OPENDKIM_DIR}: {e}"))?;
    Ok(())
}

fn write_if_changed(path: &str, content: &str) -> Result<bool, String> {
    let prev = fs::read_to_string(path).unwrap_or_default();
    if prev == content {
        return Ok(false);
    }
    if Path::new(path).exists() {
        let bak = format!(
            "{path}.bak.cpn.{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );
        let _ = fs::copy(path, &bak);
    }
    fs::write(path, content).map_err(|e| format!("Cannot write {path}: {e}"))?;
    Ok(true)
}

fn ensure_baseline_trusted_hosts(existing: &str) -> String {
    let required = ["127.0.0.1", "::1", "localhost"];
    let mut lines: Vec<String> = existing
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    for req in required {
        if !lines.iter().any(|l| l.trim() == req) {
            lines.push(req.to_string());
        }
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn chown_opendkim_keys(entries: &[OpendkimDomainEntry]) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Root owns the CPN store; group opendkim must traverse (750) if a fallback path is used.
        let root = dkim_root();
        let _ = Command::new("chown")
            .args(["root:opendkim"])
            .arg(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = fs::set_permissions(&root, fs::Permissions::from_mode(0o750));
    }
    for entry in entries {
        let _ = Command::new("chown")
            .args(["opendkim:opendkim"])
            .arg(&entry.private_key)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&entry.private_key, fs::Permissions::from_mode(0o640));
        }
        if let Some(parent) = entry.private_key.parent() {
            let _ = Command::new("chown")
                .args(["-R", "opendkim:opendkim"])
                .arg(parent)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o750));
            }
        }
    }
    let _ = Command::new("chown")
        .args(["-R", "opendkim:opendkim", OPENDKIM_DIR])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("restorecon")
        .args(["-Rv", OPENDKIM_DIR])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Sync CPN DKIM domains into OpenDKIM tables without removing classic (CyberPanel) rows.
pub fn sync_opendkim_tables(entries: &[OpendkimDomainEntry]) -> Result<String, String> {
    ensure_opendkim_dirs()?;
    let signing_existing = fs::read_to_string(SIGNING_TABLE).unwrap_or_default();
    let mut managed_key = Vec::new();
    let mut managed_sign = Vec::new();
    let mut skipped = 0usize;
    for entry in entries {
        managed_key.push(keytable_line(entry));
        // Preserve an existing non-CPN signing map for the same domain (classic selectors).
        let in_cpn_block = signing_existing
            .split(CPN_BEGIN)
            .nth(1)
            .and_then(|rest| rest.split(CPN_END).next())
            .unwrap_or("");
        let classic_has = signing_table_has_domain(&signing_existing, &entry.domain)
            && !signing_table_has_domain(in_cpn_block, &entry.domain);
        if classic_has {
            skipped += 1;
            continue;
        }
        managed_sign.push(signing_table_line(entry));
    }
    let key_body = upsert_cpn_managed_block(
        &fs::read_to_string(KEY_TABLE).unwrap_or_else(|_| "# OPENDKIM KEY TABLE\n".into()),
        &managed_key,
    );
    let sign_body = upsert_cpn_managed_block(
        &fs::read_to_string(SIGNING_TABLE).unwrap_or_else(|_| "# OPENDKIM SIGNING TABLE\n".into()),
        &managed_sign,
    );
    let trusted = ensure_baseline_trusted_hosts(
        &fs::read_to_string(TRUSTED_HOSTS).unwrap_or_else(|_| "# OPENDKIM TRUSTED HOSTS\n".into()),
    );
    let mut notes = Vec::new();
    if write_if_changed(KEY_TABLE, &key_body)? {
        notes.push("KeyTable updated".into());
    }
    if write_if_changed(SIGNING_TABLE, &sign_body)? {
        notes.push("SigningTable updated".into());
    }
    if write_if_changed(TRUSTED_HOSTS, &trusted)? {
        notes.push("TrustedHosts updated".into());
    }
    if notes.is_empty() {
        notes.push("tables unchanged".into());
    }
    if skipped > 0 {
        notes.push(format!(
            "left {skipped} classic SigningTable domain(s) untouched"
        ));
    }
    Ok(notes.join("; "))
}

fn rspamd_milter_listening() -> bool {
    Path::new("/etc/rspamd").is_dir()
        && (systemd_unit_file_exists("rspamd")
            && Command::new("systemctl")
                .args(["is-active", "--quiet", "rspamd"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false))
}

pub fn ensure_postfix_opendkim_milters() -> Result<String, String> {
    let smtpd = crate::panel_ops_mail_milters::compose_smtpd_milters(
        POSTFIX_MILTER,
        rspamd_milter_listening(),
    );
    let status = Command::new("postconf")
        .args([
            "-e",
            &format!("smtpd_milters={smtpd}"),
            &format!("non_smtpd_milters={POSTFIX_MILTER}"),
            "milter_default_action=accept",
            "milter_protocol=6",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("postconf milters failed: {e}"))?;
    if !status.success() {
        return Err("postconf could not set OpenDKIM milters".into());
    }
    let _ = Command::new("systemctl")
        .args(["reload", "postfix"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(format!(
        "Postfix milters -> smtpd={smtpd}; non_smtpd={POSTFIX_MILTER}"
    ))
}

/// EPEL OpenDKIM needs `libmemcached.so.11` from CRB/PowerTools on EL9/10.
fn ensure_opendkim_el_repos() {
    if !Path::new("/usr/bin/dnf").is_file() && !Path::new("/bin/dnf").is_file() {
        return;
    }
    let _ = Command::new("dnf")
        .args(["install", "-y", "-q", "epel-release", "dnf-plugins-core"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    // Alma/Rocky 9+: `/usr/bin/crb enable` or dnf config-manager.
    let _ = Command::new("/usr/bin/crb")
        .arg("enable")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    for repo in ["crb", "powertools", "PowerTools"] {
        let _ = Command::new("dnf")
            .args(["config-manager", "--set-enabled", repo])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = Command::new("dnf")
        .args(["install", "-y", "-q", "libmemcached"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn install_opendkim_package() -> Result<String, String> {
    if rpm_or_dpkg_installed(&["opendkim"]) {
        return Ok("opendkim package already installed".into());
    }
    ensure_opendkim_el_repos();
    install_packages_dnf_or_apt(&["opendkim", "opendkim-tools"], &["opendkim"])?;
    if !rpm_or_dpkg_installed(&["opendkim"]) {
        return Err("opendkim package install did not leave the package on disk".into());
    }
    Ok("installed opendkim".into())
}

/// Install OpenDKIM (if needed), sync CPN keys, enable service, wire Postfix milters.
pub fn ensure_opendkim_stack() -> Result<String, String> {
    let mut steps = Vec::new();
    steps.push(install_opendkim_package()?);
    ensure_opendkim_dirs()?;
    if crate::panel_ops_opendkim_conf::ensure_opendkim_conf()? {
        steps.push("opendkim.conf updated".into());
    }
    // Touch table files so refile: paths exist before first sync.
    for path in [KEY_TABLE, SIGNING_TABLE, TRUSTED_HOSTS] {
        if !Path::new(path).exists() {
            let starter = match path {
                KEY_TABLE => "# OPENDKIM KEY TABLE\n",
                SIGNING_TABLE => "# OPENDKIM SIGNING TABLE\n",
                _ => "# OPENDKIM TRUSTED HOSTS\n127.0.0.1\n::1\nlocalhost\n",
            };
            fs::write(path, starter).map_err(|e| format!("Cannot create {path}: {e}"))?;
        }
    }
    let entries = collect_cpn_opendkim_entries();
    steps.push(sync_opendkim_tables(&entries)?);
    chown_opendkim_keys(&entries);
    if !systemd_unit_file_exists("opendkim") {
        return Err("opendkim systemd unit missing after package install".into());
    }
    enable_now(&["opendkim"])?;
    steps.push("opendkim enabled".into());
    // Soft-heal rspamd so Auth-Results (SPF/DMARC) stay available for Tachyon.
    if let Ok(msg) = crate::panel_ops_email_antispam::heal_rspamd_for_auth_results() {
        steps.push(msg);
    }
    steps.push(ensure_postfix_opendkim_milters()?);
    Ok(format!(
        "OpenDKIM ready ({} CPN domain key(s)): {}",
        entries.len(),
        steps.join(" | ")
    ))
}

/// Ensure keys for one domain, then refresh OpenDKIM tables/service.
/// DNS publish stays in `provision_mail_dns` / Email Start / DKIM Manager (avoids double CF push on site ready).
pub fn ensure_opendkim_for_domain(domain: &str) -> Result<String, String> {
    let key_msg = ensure_dkim_for_domain(domain)?;
    let stack = ensure_opendkim_stack()?;
    Ok(format!("{key_msg} {stack}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_ops_dkim_keys::ensure_dkim_root;
    use std::path::PathBuf;

    #[test]
    fn key_and_signing_lines_format() {
        let entry = OpendkimDomainEntry {
            domain: "example.com".into(),
            selector: "default".into(),
            private_key: PathBuf::from("/var/lib/cpn/dkim/example.com/default.private"),
        };
        assert_eq!(
            keytable_line(&entry),
            "default._domainkey.example.com example.com:default:/var/lib/cpn/dkim/example.com/default.private"
        );
        assert_eq!(
            signing_table_line(&entry),
            "*@example.com default._domainkey.example.com"
        );
    }

    #[test]
    fn upsert_replaces_managed_block_only() {
        let existing = "# header\nclassic.example.com classic:default:/old\n";
        let merged = upsert_cpn_managed_block(
            existing,
            &["default._domainkey.a.com a.com:default:/k".into()],
        );
        assert!(merged.contains("classic.example.com"));
        assert!(merged.contains(CPN_BEGIN));
        assert!(merged.contains("default._domainkey.a.com"));
        let again = upsert_cpn_managed_block(
            &merged,
            &["default._domainkey.b.com b.com:default:/k2".into()],
        );
        assert!(!again.contains("default._domainkey.a.com"));
        assert!(again.contains("default._domainkey.b.com"));
        assert!(again.contains("classic.example.com"));
    }

    #[test]
    fn detects_existing_signing_domain() {
        let table = "*@newstargeted.com nt20261009._domainkey.newstargeted.com\n";
        assert!(signing_table_has_domain(table, "newstargeted.com"));
        assert!(!signing_table_has_domain(table, "other.com"));
    }

    #[test]
    fn collect_entries_from_store() {
        with_test_data_dir(|| {
            let _ = ensure_dkim_root().unwrap();
            let domain = "opendkim-test.example";
            let dir = dkim_root().join(domain);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("default.private"), "dummy-key").unwrap();
            fs::write(dir.join("default.dnsvalue"), "v=DKIM1; k=rsa; p=ABC").unwrap();
            let entries = collect_cpn_opendkim_entries();
            assert!(entries.iter().any(|e| e.domain == domain));
        });
    }
}
