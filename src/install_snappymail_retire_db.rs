//! MariaDB side of retiring prior-webmail (SnappyMail) data after Tachyon is live.
//!
//! The prior webmail address book lives in `rainloop_*` tables. Tachyon uses its own
//! database (default `cpn_tachyon_ab`), which is never touched here. A database is only
//! dropped after a verified dump under the retire backup dir and a safety check that
//! its contacts were imported (or it holds none). No credentials are printed.

use crate::install_snappymail_lineage::parse_ini_quoted_value;
use crate::install_snappymail_migrate::discover_snappymail_data_sources;
use crate::panel_ops_db::{is_system_database, mariadb_client_bin, mariadb_dump_bin};
use std::path::{Path, PathBuf};
use std::process::Command;

const TACHYON_SECRET: &str = "/var/lib/cpn/webmail-contacts/tachyon.json";
const DEFAULT_TACHYON_DB: &str = "cpn_tachyon_ab";
const PRIOR_DB_FALLBACK: &str = "snappymail";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbInventory {
    pub name: String,
    pub table_count: u64,
    pub foreign_table_count: u64,
    pub src_contacts: u64,
    /// Prior rows (contacts, properties, users) not found in Tachyon's DB.
    /// `None` when Tachyon's tables are unavailable, so nothing can be proven.
    pub missing_in_dest: Option<u64>,
}

/// Database name from a PDO DSN such as `mysql:host=localhost;dbname=snappymail`.
pub fn dbname_from_dsn(dsn: &str) -> Option<String> {
    let body = dsn.strip_prefix("mysql:").unwrap_or(dsn);
    for part in body.split(';') {
        if let Some((k, v)) = part.split_once('=')
            && k.trim().eq_ignore_ascii_case("dbname")
        {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            return ident_ok(v).then(|| v.to_string());
        }
    }
    None
}

fn ident_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Tachyon's own address-book DB name (never a retire candidate).
pub fn tachyon_db_name() -> String {
    std::fs::read_to_string(TACHYON_SECRET)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| {
            v.get("pdo_dsn")
                .and_then(|d| d.as_str())
                .and_then(dbname_from_dsn)
        })
        .unwrap_or_else(|| DEFAULT_TACHYON_DB.to_string())
}

fn ini_db_for_data_dir(data_dir: &Path) -> Option<String> {
    let ini = data_dir.join("_data_/_default_/configs/application.ini");
    let raw = std::fs::read_to_string(ini).ok()?;
    parse_ini_quoted_value(&raw, "pdo_dsn").and_then(|d| dbname_from_dsn(&d))
}

/// Candidate prior-webmail DB names (deduped; excludes system and Tachyon DBs).
pub fn candidate_db_names() -> Vec<String> {
    let tachyon = tachyon_db_name();
    let mut raw: Vec<String> = Vec::new();
    let mut dirs: Vec<PathBuf> = discover_snappymail_data_sources();
    dirs.push(PathBuf::from("/var/lib/cpn-webmail/snappymail"));
    for d in dirs {
        if let Some(n) = ini_db_for_data_dir(&d) {
            raw.push(n);
        }
    }
    raw.push(PRIOR_DB_FALLBACK.to_string());
    filter_candidates(raw, &tachyon)
}

pub(crate) fn filter_candidates(raw: Vec<String>, tachyon_db: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in raw {
        if !ident_ok(&n) || is_system_database(&n) || n.eq_ignore_ascii_case(tachyon_db) {
            continue;
        }
        if !out.iter().any(|o| o.eq_ignore_ascii_case(&n)) {
            out.push(n);
        }
    }
    out
}

fn sql(bin: &str, query: &str) -> Result<String, String> {
    let out = Command::new(bin)
        .args(["-N", "-B", "-e", query])
        .output()
        .map_err(|e| format!("could not run {bin}: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr)
            .lines()
            .next()
            .unwrap_or("sql failed")
            .chars()
            .take(160)
            .collect());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn count(bin: &str, query: &str) -> Result<u64, String> {
    sql(bin, query)?
        .parse::<u64>()
        .map_err(|e| format!("bad count: {e}"))
}

fn table_exists(bin: &str, db: &str, table: &str) -> Result<bool, String> {
    Ok(count(
        bin,
        &format!(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='{db}' AND table_name='{table}'"
        ),
    )? > 0)
}

/// Address-book tables and the null-safe join keys that identify a row (Tachyon only
/// adds columns such as `prop_value_lower`, so a plain table checksum would differ).
const ADDRESS_BOOK_TABLES: &[(&str, &str)] = &[
    (
        "rainloop_ab_contacts",
        "d.id_contact=s.id_contact AND d.id_contact_str<=>s.id_contact_str AND d.id_user<=>s.id_user AND d.display<=>s.display AND d.deleted<=>s.deleted",
    ),
    (
        "rainloop_ab_properties",
        "d.id_prop=s.id_prop AND d.id_contact<=>s.id_contact AND d.id_user<=>s.id_user AND d.prop_type<=>s.prop_type AND d.prop_value<=>s.prop_value",
    ),
    (
        "rainloop_users",
        "d.id_user=s.id_user AND d.rl_email<=>s.rl_email",
    ),
];

/// Count prior address-book rows that are absent from Tachyon's database.
fn missing_rows(bin: &str, prior: &str, tachyon: &str) -> Result<u64, String> {
    let mut missing = 0u64;
    for (table, on) in ADDRESS_BOOK_TABLES {
        if !table_exists(bin, prior, table)? {
            continue;
        }
        let key = if *table == "rainloop_ab_properties" {
            "id_prop"
        } else if *table == "rainloop_users" {
            "id_user"
        } else {
            "id_contact"
        };
        missing += count(
            bin,
            &format!(
                "SELECT COUNT(*) FROM `{prior}`.{table} s LEFT JOIN `{tachyon}`.{table} d ON {on} WHERE d.{key} IS NULL"
            ),
        )?;
    }
    Ok(missing)
}

/// Inventory existing prior-webmail DBs (only schemas that hold `rainloop_*` tables).
pub fn inventory() -> Result<Vec<DbInventory>, String> {
    let Some(bin) = mariadb_client_bin() else {
        return Ok(Vec::new());
    };
    let listing = sql(bin, "SHOW DATABASES")?;
    let existing: Vec<&str> = listing.lines().map(str::trim).collect();
    let tachyon = tachyon_db_name();
    let dest_ready = existing.iter().any(|d| d.eq_ignore_ascii_case(&tachyon))
        && ADDRESS_BOOK_TABLES
            .iter()
            .all(|(t, _)| table_exists(bin, &tachyon, t).unwrap_or(false));
    let mut out = Vec::new();
    for name in candidate_db_names() {
        if !existing.iter().any(|d| d.eq_ignore_ascii_case(&name)) {
            continue;
        }
        let rain = count(
            bin,
            &format!(
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='{name}' AND table_name LIKE 'rainloop\\_%'"
            ),
        )?;
        if rain == 0 {
            continue; // not a prior-webmail schema; never touch it
        }
        let total = count(
            bin,
            &format!("SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='{name}'"),
        )?;
        let src_contacts = if table_exists(bin, &name, "rainloop_ab_contacts")? {
            count(
                bin,
                &format!("SELECT COUNT(*) FROM `{name}`.rainloop_ab_contacts"),
            )?
        } else {
            0
        };
        let missing_in_dest = if dest_ready {
            missing_rows(bin, &name, &tachyon).ok()
        } else {
            None
        };
        out.push(DbInventory {
            table_count: rain,
            foreign_table_count: total.saturating_sub(rain),
            src_contacts,
            missing_in_dest,
            name,
        });
    }
    Ok(out)
}

/// Pure safety gate: Ok when the DB may be dropped after a verified dump.
pub fn drop_allowed(inv: &DbInventory) -> Result<(), String> {
    if inv.foreign_table_count > 0 {
        return Err("schema also holds non-prior-webmail tables".into());
    }
    match inv.missing_in_dest {
        Some(0) => Ok(()),
        None if inv.src_contacts == 0 => Ok(()),
        Some(n) => Err(format!(
            "{n} prior address-book rows are not in Tachyon (dump archived, database kept)"
        )),
        None => Err(format!(
            "{} contacts could not be verified against Tachyon (dump archived, database kept)",
            inv.src_contacts
        )),
    }
}

/// Dump `name` to `<dir>/<name>.sql.gz` and verify it. Returns the dump path.
pub fn dump_verified(name: &str, dir: &Path) -> Result<PathBuf, String> {
    if !ident_ok(name) {
        return Err("unsafe database name".into());
    }
    let bin = mariadb_dump_bin().ok_or("mariadb-dump/mysqldump not found")?;
    let out = dir.join(format!("{name}.sql.gz"));
    let script = "set -o pipefail; \"$1\" --single-transaction --routines --triggers -- \"$2\" | gzip -c > \"$3\" \
                  && gzip -t \"$3\" && \
                  [ \"$(zcat \"$3\" | grep -c 'CREATE TABLE')\" -gt 0 ]";
    let status = Command::new("bash")
        .args(["-c", script, "retire-dump", bin, name])
        .arg(&out)
        .status()
        .map_err(|e| format!("dump failed to start: {e}"))?;
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    if !status.success() || size < 64 {
        let _ = std::fs::remove_file(&out);
        return Err(format!("database dump for `{name}` failed verification"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inv(src: u64, missing: Option<u64>, foreign: u64) -> DbInventory {
        DbInventory {
            name: "snappymail".into(),
            table_count: 3,
            foreign_table_count: foreign,
            src_contacts: src,
            missing_in_dest: missing,
        }
    }

    #[test]
    fn dsn_parsing() {
        assert_eq!(
            dbname_from_dsn("mysql:host=localhost;dbname=snappymail;charset=utf8mb4").as_deref(),
            Some("snappymail")
        );
        assert_eq!(
            dbname_from_dsn("host=localhost;dbname=sm_db").as_deref(),
            Some("sm_db")
        );
        assert_eq!(
            dbname_from_dsn("mysql:host=localhost;dbname=bad-name"),
            None
        );
        assert_eq!(dbname_from_dsn("mysql:host=localhost"), None);
    }

    #[test]
    fn candidates_exclude_tachyon_system_and_dupes() {
        let got = filter_candidates(
            vec![
                "snappymail".into(),
                "SnappyMail".into(),
                "cpn_tachyon_ab".into(),
                "mysql".into(),
                "bad name".into(),
            ],
            "cpn_tachyon_ab",
        );
        assert_eq!(got, vec!["snappymail".to_string()]);
    }

    #[test]
    fn drop_gate() {
        assert!(drop_allowed(&inv(0, None, 0)).is_ok());
        assert!(drop_allowed(&inv(202, Some(0), 0)).is_ok());
        assert!(drop_allowed(&inv(202, Some(3), 0)).is_err());
        assert!(drop_allowed(&inv(202, None, 0)).is_err());
        assert!(drop_allowed(&inv(0, None, 2)).is_err());
        assert!(drop_allowed(&inv(202, Some(0), 1)).is_err());
    }
}
