//! Authorization helpers for MariaDB Manager / Databases actions.

use crate::panel_admin::is_panel_admin;
use crate::panel_ops_db::{is_system_database, list_databases as list_live_databases};
use crate::resource_accounts::{DatabaseRecord, find_database, list_databases as list_registry};
use crate::site_acl::{SitePerm, can_manage_site};

/// True when the signed-in user may manage this database name.
pub fn can_manage_database(username: &str, db_name: &str) -> bool {
    let name = db_name.trim();
    if name.is_empty() || is_system_database(name) {
        return false;
    }
    if is_panel_admin(username) {
        return true;
    }
    let Some(rec) = find_database(name) else {
        return false;
    };
    if rec.owner.eq_ignore_ascii_case(username) {
        return true;
    }
    if !rec.domain.is_empty() {
        return can_manage_site(username, &rec.domain, SitePerm::Enable).unwrap_or(false);
    }
    false
}

pub fn require_manage_database(username: &str, db_name: &str) -> Result<(), String> {
    if can_manage_database(username, db_name) {
        Ok(())
    } else {
        Err(
            "You do not have permission to manage this database (domain-jailed or owner-only)."
                .into(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct ManagedDatabase {
    pub name: String,
    pub owner: String,
    pub domain: String,
    pub in_mariadb: bool,
    pub in_registry: bool,
}

/// Databases the signed-in user may see/manage.
///
/// Admins see live MariaDB schemas (minus system) plus registry-only rows.
/// Site users see registry entries they own or whose domain they can manage.
pub fn list_managed_databases(username: &str) -> Vec<ManagedDatabase> {
    let registry = list_registry();
    let live = list_live_databases();
    let live_set: std::collections::BTreeSet<String> = live
        .databases
        .iter()
        .map(|n| n.to_ascii_lowercase())
        .collect();

    let mut out: Vec<ManagedDatabase> = Vec::new();
    let admin = is_panel_admin(username);

    if admin {
        for name in &live.databases {
            if is_system_database(name) {
                continue;
            }
            let rec = registry.iter().find(|r| r.name.eq_ignore_ascii_case(name));
            out.push(row_from(name, rec, true));
        }
        for rec in &registry {
            if out.iter().any(|r| r.name.eq_ignore_ascii_case(&rec.name)) {
                continue;
            }
            out.push(row_from(
                &rec.name,
                Some(rec),
                live_set.contains(&rec.name.to_ascii_lowercase()),
            ));
        }
    } else {
        for rec in &registry {
            if !can_manage_database(username, &rec.name) {
                continue;
            }
            out.push(row_from(
                &rec.name,
                Some(rec),
                live_set.contains(&rec.name.to_ascii_lowercase()),
            ));
        }
    }

    out.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    out
}

fn row_from(name: &str, rec: Option<&DatabaseRecord>, in_mariadb: bool) -> ManagedDatabase {
    ManagedDatabase {
        name: name.to_string(),
        owner: rec.map(|r| r.owner.clone()).unwrap_or_default(),
        domain: rec.map(|r| r.domain.clone()).unwrap_or_default(),
        in_mariadb,
        in_registry: rec.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::resource_accounts::create_database;

    #[test]
    fn non_admin_needs_registry() {
        with_test_data_dir(|| {
            assert!(!can_manage_database("alice", "missing_db"));
            let _ = create_database("alice", "alice_db", "alice.example").unwrap();
            assert!(can_manage_database("alice", "alice_db"));
            assert!(!can_manage_database("bob", "alice_db"));
        });
    }
}
