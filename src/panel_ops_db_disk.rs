//! MariaDB schema size metering for owned panel databases.

use crate::panel_db_acl::list_managed_databases;
use crate::panel_ops_db::{is_system_database, mariadb_client_bin};
use std::process::Command;

/// Sum `information_schema` data + index length for databases the account may manage.
///
/// Uses the same inventory as the Databases UI (live MariaDB for panel admin,
/// registry-owned rows for other accounts). Returns 0 when MariaDB is unavailable
/// or no managed schemas exist.
pub fn database_disk_bytes_for_owner(username: &str) -> u64 {
    let key = username.trim().to_ascii_lowercase();
    if let Some(hit) = crate::panel_perf_cache::caches().db_disk.get(&key) {
        return hit;
    }
    match database_disk_bytes_uncached(username) {
        Some(total) => {
            crate::panel_perf_cache::caches()
                .db_disk
                .insert(key, total);
            total
        }
        None => 0,
    }
}

fn schema_name_safe(name: &str) -> Option<String> {
    let safe = name.replace('`', "");
    if safe.is_empty() || safe.contains(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        None
    } else {
        Some(safe)
    }
}

/// One information_schema query for every owned schema (not one client per database).
fn database_disk_bytes_uncached(username: &str) -> Option<u64> {
    let owned: Vec<String> = list_managed_databases(username)
        .into_iter()
        .map(|d| d.name)
        .filter(|n| !is_system_database(n))
        .filter_map(|n| schema_name_safe(&n))
        .collect();
    if owned.is_empty() {
        return Some(0);
    }
    let bin = mariadb_client_bin()?;
    let in_list = owned
        .iter()
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT COALESCE(SUM(data_length + index_length), 0)          FROM information_schema.tables WHERE table_schema IN ({in_list});"
    );
    let out = Command::new(bin).args(["-N", "-e", &sql]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<u64>()
            .unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn empty_owner_is_zero() {
        with_test_data_dir(|| {
            assert_eq!(database_disk_bytes_for_owner("nobody-yet"), 0);
        });
    }
}
