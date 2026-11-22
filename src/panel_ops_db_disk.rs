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
    let owned: Vec<String> = list_managed_databases(username)
        .into_iter()
        .map(|d| d.name)
        .filter(|n| !is_system_database(n))
        .collect();
    if owned.is_empty() {
        return 0;
    }
    let Some(bin) = mariadb_client_bin() else {
        return 0;
    };
    let mut total = 0u64;
    for name in owned {
        let safe = name.replace('`', "");
        if safe.is_empty() || safe.contains(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            continue;
        }
        let sql = format!(
            "SELECT COALESCE(SUM(data_length + index_length), 0) \
             FROM information_schema.tables WHERE table_schema = '{safe}';"
        );
        let Ok(out) = Command::new(bin).args(["-N", "-e", &sql]).output() else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Ok(n) = text.parse::<u64>() {
            total = total.saturating_add(n);
        }
    }
    total
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
