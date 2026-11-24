//! Runtime mailbox operations against `cpn_mail` for the existing mail code paths.
//!
//! `mail_legacy_sql` calls these so password changes and mailbox lookups keep
//! working in all three states: classic passdb only, both (dual-write while
//! migrating), and `cpn_mail` only after cutover. Everything here uses the
//! dedicated panel account, never the administrative one.

use super::client::{PanelSql, SqlExec};
use super::config::PanelDbConfig;
use super::mail::{DOVECOT_SQL_CONF, PassdbBackend, backend_of};
use super::sqlfmt::text_lit;
use std::fs;

/// True when Dovecot currently authenticates against `cpn_mail`.
pub fn cpn_mail_active() -> bool {
    fs::read_to_string(DOVECOT_SQL_CONF)
        .map(|c| backend_of(&c) == PassdbBackend::CpnMail)
        .unwrap_or(false)
}

fn client() -> Result<(PanelDbConfig, PanelSql), String> {
    let cfg = PanelDbConfig::load()?.ok_or("Panel database is not provisioned")?;
    let sql = PanelSql::new(&cfg);
    Ok((cfg, sql))
}

/// True when the panel database is provisioned (dual-write target exists).
pub fn provisioned() -> bool {
    PanelDbConfig::load().ok().flatten().is_some()
}

pub fn mailbox_exists(email: &str) -> Result<bool, String> {
    let (cfg, sql) = client()?;
    exists_with(&sql, &cfg, email)
}

fn exists_with(sql: &dyn SqlExec, cfg: &PanelDbConfig, email: &str) -> Result<bool, String> {
    let rows = sql.query(
        Some(&cfg.mail_db),
        &format!(
            "SELECT 1 FROM mailboxes WHERE email = {} AND deleted_at IS NULL LIMIT 1",
            text_lit(&email.to_ascii_lowercase())
        ),
    )?;
    Ok(!rows.is_empty())
}

/// Upsert statement for a mailbox password hash (creates the row when missing).
pub fn upsert_sql(email: &str, hash: &str) -> Result<String, String> {
    let email = email.trim().to_ascii_lowercase();
    let (local, domain) = email.split_once('@').ok_or("Mailbox address is invalid")?;
    if local.is_empty() || domain.is_empty() || !hash.starts_with('{') {
        return Err("Mailbox hash or address is invalid".into());
    }
    Ok(format!(
        "INSERT INTO mail_domains (domain) VALUES ({domain_l}) ON DUPLICATE KEY UPDATE deleted_at = NULL;\n\
         INSERT INTO mailboxes (email, local_part, domain, password_hash, mail_location, home, source) \
         VALUES ({email_l}, {local_l}, {domain_l}, {hash_l}, \
           CONCAT('maildir:/home/vmail/', {domain_l}, '/', {local_l}, '/Maildir'), \
           CONCAT('/home/vmail/', {domain_l}, '/', {local_l}), 'panel') \
         ON DUPLICATE KEY UPDATE password_hash = VALUES(password_hash), deleted_at = NULL;\n",
        email_l = text_lit(&email),
        local_l = text_lit(local),
        domain_l = text_lit(domain),
        hash_l = text_lit(hash),
    ))
}

/// Store a Dovecot-scheme hash in `cpn_mail`.
pub fn set_password_hash(email: &str, hash: &str) -> Result<(), String> {
    let (cfg, sql) = client()?;
    sql.run(Some(&cfg.mail_db), &upsert_sql(email, hash)?)?;
    Ok(())
}

/// Mailboxes known to `cpn_mail` as `(email, domain)`.
pub fn list_mailboxes() -> Result<Vec<(String, String)>, String> {
    let (cfg, sql) = client()?;
    let rows = sql.query(
        Some(&cfg.mail_db),
        "SELECT email, domain FROM mailboxes WHERE deleted_at IS NULL ORDER BY email",
    )?;
    Ok(rows
        .into_iter()
        .filter_map(|r| Some((r.first()?.clone(), r.get(1)?.clone())))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::super::client::fake::FakeSql;
    use super::*;

    #[test]
    fn upsert_never_embeds_the_hash_as_a_quoted_string() {
        let sql = upsert_sql(
            "Info@Example.COM",
            "{BLF-CRYPT}$2y$05$abcdefghijklmnopqrstuv",
        )
        .unwrap();
        assert!(!sql.contains("$2y$05"));
        assert!(sql.contains("ON DUPLICATE KEY UPDATE password_hash"));
        assert!(!sql.contains("Info@"));
    }

    #[test]
    fn upsert_rejects_bad_input() {
        assert!(upsert_sql("nope", "{X}a").is_err());
        assert!(upsert_sql("a@b.c", "plaintext").is_err());
    }

    #[test]
    fn exists_lowercases_and_reads_rows() {
        let sql = FakeSql::default();
        sql.reply("SELECT 1 FROM mailboxes", Ok("1\n"));
        let cfg = PanelDbConfig::generate(1);
        assert!(exists_with(&sql, &cfg, "A@B.C").unwrap());
        let none = FakeSql::default();
        assert!(!exists_with(&none, &cfg, "a@b.c").unwrap());
    }
}
