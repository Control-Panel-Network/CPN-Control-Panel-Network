//! Duplicate and bulk update/delete helpers for hosting packages.

use crate::packages::{
    Package, PackageInput, create_package, delete_package, get_package, update_package,
};

/// Optional field patch applied to every selected package id.
#[derive(Debug, Clone, Default)]
pub struct PackageBulkPatch {
    pub disk_mb: Option<i64>,
    pub bandwidth_mb: Option<i64>,
    pub domains: Option<i64>,
    pub emails: Option<i64>,
    pub databases: Option<i64>,
    pub ftp_accounts: Option<i64>,
    pub fqdn_enabled: Option<bool>,
    pub notes: Option<String>,
}

impl PackageBulkPatch {
    pub fn is_empty(&self) -> bool {
        self.disk_mb.is_none()
            && self.bandwidth_mb.is_none()
            && self.domains.is_none()
            && self.emails.is_none()
            && self.databases.is_none()
            && self.ftp_accounts.is_none()
            && self.fqdn_enabled.is_none()
            && self.notes.is_none()
    }
}

#[derive(Debug, Clone, Default)]
pub struct BulkOutcome {
    pub ok: usize,
    pub errors: Vec<String>,
}

impl BulkOutcome {
    pub fn summary(&self, verb: &str) -> String {
        if self.errors.is_empty() {
            format!("{verb} {n} package(s)", n = self.ok)
        } else if self.ok == 0 {
            self.errors.join("; ")
        } else {
            format!(
                "{verb} {ok} package(s); {failed} failed: {detail}",
                ok = self.ok,
                failed = self.errors.len(),
                detail = self.errors.join("; ")
            )
        }
    }
}

/// Copy plan limits/features into a new package with the given name.
pub fn duplicate_package(source_id: &str, new_name: &str) -> Result<Package, String> {
    let src = get_package(source_id)?;
    let name = new_name.trim();
    if name.is_empty() {
        return Err("New package name is required".into());
    }
    create_package(PackageInput {
        name: name.to_string(),
        disk_mb: src.disk_mb,
        bandwidth_mb: src.bandwidth_mb,
        domains: src.domains,
        emails: src.emails,
        databases: src.databases,
        ftp_accounts: src.ftp_accounts,
        fqdn_enabled: src.fqdn_enabled,
        notes: src.notes,
        sidebar_hidden_nav_ids: src.sidebar_hidden_nav_ids,
    })
}

fn apply_patch(pkg: &Package, patch: &PackageBulkPatch) -> PackageInput {
    PackageInput {
        name: pkg.name.clone(),
        disk_mb: patch.disk_mb.unwrap_or(pkg.disk_mb),
        bandwidth_mb: patch.bandwidth_mb.unwrap_or(pkg.bandwidth_mb),
        domains: patch.domains.unwrap_or(pkg.domains),
        emails: patch.emails.unwrap_or(pkg.emails),
        databases: patch.databases.unwrap_or(pkg.databases),
        ftp_accounts: patch.ftp_accounts.unwrap_or(pkg.ftp_accounts),
        fqdn_enabled: patch.fqdn_enabled.unwrap_or(pkg.fqdn_enabled),
        notes: patch.notes.clone().unwrap_or_else(|| pkg.notes.clone()),
        sidebar_hidden_nav_ids: pkg.sidebar_hidden_nav_ids.clone(),
    }
}

/// Apply the same optional fields to each selected package (names stay unchanged).
pub fn bulk_update_packages(ids: &[String], patch: &PackageBulkPatch) -> BulkOutcome {
    let mut out = BulkOutcome::default();
    if ids.is_empty() {
        out.errors.push("Select at least one package".into());
        return out;
    }
    if patch.is_empty() {
        out.errors.push("No bulk fields to apply".into());
        return out;
    }
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        match get_package(id).and_then(|pkg| update_package(id, apply_patch(&pkg, patch))) {
            Ok(_) => out.ok += 1,
            Err(error) => out.errors.push(format!("{id}: {error}")),
        }
    }
    if out.ok == 0 && out.errors.is_empty() {
        out.errors.push("Select at least one package".into());
    }
    out
}

/// Delete each selected package (Default and assigned packages still blocked).
pub fn bulk_delete_packages(ids: &[String]) -> BulkOutcome {
    let mut out = BulkOutcome::default();
    if ids.is_empty() {
        out.errors.push("Select at least one package".into());
        return out;
    }
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        match delete_package(id) {
            Ok(()) => out.ok += 1,
            Err(error) => out.errors.push(format!("{id}: {error}")),
        }
    }
    if out.ok == 0 && out.errors.is_empty() {
        out.errors.push("Select at least one package".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::packages::{DEFAULT_PACKAGE_ID, ensure_default_package};

    fn sample_input(name: &str) -> PackageInput {
        PackageInput {
            name: name.into(),
            disk_mb: 500,
            bandwidth_mb: 500,
            domains: 2,
            emails: 10,
            databases: 2,
            ftp_accounts: 2,
            fqdn_enabled: true,
            notes: "base".into(),
            sidebar_hidden_nav_ids: Vec::new(),
        }
    }

    #[test]
    fn duplicate_copies_limits_with_new_name() {
        with_test_data_dir(|| {
            ensure_default_package().unwrap();
            let src = create_package(sample_input("Starter")).unwrap();
            let copy = duplicate_package(&src.id, "Starter Copy").unwrap();
            assert_eq!(copy.name, "Starter Copy");
            assert_ne!(copy.id, src.id);
            assert_eq!(copy.disk_mb, src.disk_mb);
            assert_eq!(copy.domains, src.domains);
            assert_eq!(copy.fqdn_enabled, src.fqdn_enabled);
            assert_eq!(copy.notes, src.notes);
        });
    }

    #[test]
    fn bulk_update_sets_fqdn_and_disk() {
        with_test_data_dir(|| {
            ensure_default_package().unwrap();
            let a = create_package(sample_input("A")).unwrap();
            let b = create_package(sample_input("B")).unwrap();
            let out = bulk_update_packages(
                &[a.id.clone(), b.id.clone()],
                &PackageBulkPatch {
                    disk_mb: Some(2048),
                    fqdn_enabled: Some(false),
                    ..Default::default()
                },
            );
            assert_eq!(out.ok, 2);
            assert!(out.errors.is_empty());
            let a2 = get_package(&a.id).unwrap();
            let b2 = get_package(&b.id).unwrap();
            assert_eq!(a2.disk_mb, 2048);
            assert!(!a2.fqdn_enabled);
            assert_eq!(b2.disk_mb, 2048);
            assert!(!b2.fqdn_enabled);
            assert_eq!(a2.name, "A");
        });
    }

    #[test]
    fn bulk_delete_skips_default() {
        with_test_data_dir(|| {
            ensure_default_package().unwrap();
            let a = create_package(sample_input("Temp")).unwrap();
            let out = bulk_delete_packages(&[DEFAULT_PACKAGE_ID.into(), a.id.clone()]);
            assert_eq!(out.ok, 1);
            assert_eq!(out.errors.len(), 1);
            assert!(get_package(&a.id).is_err());
            assert!(get_package(DEFAULT_PACKAGE_ID).is_ok());
        });
    }
}
