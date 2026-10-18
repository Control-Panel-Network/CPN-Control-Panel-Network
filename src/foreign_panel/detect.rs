//! Detect other hosting panels already installed on this machine.

use std::path::{Path, PathBuf};

use super::catalog::{CATALOG, PanelSpec};
use super::scan::{Inventory, inventory_for, rel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub inventory: Inventory,
}

pub fn host_root() -> PathBuf {
    if let Ok(value) = std::env::var("CPN_FOREIGN_PANEL_ROOT") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from("/")
}

pub fn marker_present(root: &Path, spec: &PanelSpec) -> bool {
    spec.markers.iter().any(|marker| rel(root, marker).exists())
}

pub fn detect_at(root: &Path) -> Vec<Detection> {
    let mut found: Vec<&PanelSpec> = CATALOG
        .iter()
        .filter(|spec| marker_present(root, spec))
        .collect();
    let virtualmin = found.iter().any(|spec| spec.id == "virtualmin");
    if virtualmin {
        found.retain(|spec| spec.id != "webmin");
    }
    found
        .into_iter()
        .map(|spec| Detection {
            id: spec.id.to_string(),
            name: spec.name.to_string(),
            tier: match spec.tier {
                super::catalog::CopyTier::Primary => "primary".into(),
                super::catalog::CopyTier::LongTail => "long-tail".into(),
            },
            inventory: inventory_for(root, spec),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::foreign_panel::apply::{ApplyOptions, apply_panel};
    use crate::foreign_panel::wipe::wipe_panel;
    use std::fs;

    fn touch_dir(path: &Path) {
        fs::create_dir_all(path).unwrap();
    }

    #[test]
    fn detects_primary_and_long_tail_and_skips_webmin_under_virtualmin() {
        let root = std::env::temp_dir().join(format!("cpn-fp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        touch_dir(&root.join("usr/local/cpanel"));
        touch_dir(&root.join("etc/webmin/virtual-server/domains"));
        touch_dir(&root.join("usr/share/webmin"));
        touch_dir(&root.join("etc/easypanel/projects"));
        let found = detect_at(&root);
        let ids: Vec<&str> = found.iter().map(|row| row.id.as_str()).collect();
        assert!(ids.contains(&"cpanel"));
        assert!(ids.contains(&"virtualmin"));
        assert!(ids.contains(&"easypanel"));
        assert!(!ids.contains(&"webmin"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_copies_cpanel_site_and_wipe_removes_duplicate_only() {
        with_test_data_dir(|| {
            let root = std::env::temp_dir().join(format!(
                "cpn-fp-apply-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = fs::remove_dir_all(&root);
            touch_dir(&root.join("usr/local/cpanel/whostmgr"));
            touch_dir(&root.join("home/alice/public_html"));
            touch_dir(&root.join("etc"));
            touch_dir(&root.join("var/cpanel/users"));
            touch_dir(&root.join("var/named"));
            touch_dir(&root.join("home/alice/mail/example.com"));
            fs::write(root.join("home/alice/public_html/index.html"), b"hello").unwrap();
            fs::write(root.join("etc/userdomains"), "example.com: alice\n").unwrap();
            fs::write(root.join("var/cpanel/users/alice"), "DB=app_db\n").unwrap();
            fs::write(root.join("var/named/example.com.db"), b"zone").unwrap();
            fs::write(root.join("home/alice/mail/example.com/inbox"), b"mail").unwrap();

            let report = apply_panel(
                &root,
                "cpanel",
                &ApplyOptions {
                    port_ok: true,
                    register_sites: true,
                },
            )
            .unwrap();
            assert!(report.verified);
            assert_eq!(report.sites.len(), 1);
            assert!(!report.sites[0].adopt_in_place);
            assert!(
                Path::new(&report.sites[0].dest)
                    .join("index.html")
                    .is_file()
            );
            assert!(report.zones_copied.iter().any(|z| z.ends_with(".zone")));
            assert!(report.users.iter().any(|u| u == "alice"));
            assert!(root.join("usr/local/cpanel").exists());

            let removed = wipe_panel(&root, "cpanel").unwrap();
            assert!(removed.iter().any(|p| p.contains("cpanel")));
            assert!(!root.join("usr/local/cpanel").exists());
            assert!(!root.join("home/alice/public_html").exists());
            assert!(
                Path::new(&report.sites[0].dest)
                    .join("index.html")
                    .is_file()
            );
            let _ = fs::remove_dir_all(&root);
        });
    }

    #[test]
    fn hestia_and_ispmanager_inventory() {
        let root = std::env::temp_dir().join(format!("cpn-fp-tail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        touch_dir(&root.join("usr/local/hestia"));
        touch_dir(&root.join("usr/local/hestia/data/users/bob"));
        fs::write(
            root.join("usr/local/hestia/data/users/bob/web.conf"),
            "DOMAIN='school.example'\n",
        )
        .unwrap();
        touch_dir(&root.join("home/bob/web/school.example/public_html"));
        fs::write(
            root.join("home/bob/web/school.example/public_html/index.html"),
            b"h",
        )
        .unwrap();
        touch_dir(&root.join("usr/local/mgr5"));
        touch_dir(&root.join("var/www/student/data/www/class.example"));
        fs::write(
            root.join("var/www/student/data/www/class.example/index.html"),
            b"c",
        )
        .unwrap();
        let found = detect_at(&root);
        let hestia = found.iter().find(|row| row.id == "hestiacp").unwrap();
        assert_eq!(hestia.inventory.sites[0].domain, "school.example");
        let isp = found.iter().find(|row| row.id == "ispmanager").unwrap();
        assert_eq!(isp.inventory.sites[0].domain, "class.example");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ajenti_stays_unmapped() {
        let root = std::env::temp_dir().join(format!("cpn-fp-aj-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        touch_dir(&root.join("etc/ajenti"));
        let found = detect_at(&root);
        assert_eq!(found[0].id, "ajenti");
        assert!(!found[0].inventory.mapped);
        assert!(
            apply_panel(
                &root,
                "ajenti",
                &ApplyOptions {
                    port_ok: true,
                    register_sites: false,
                },
            )
            .unwrap_err()
            .contains("not mapped")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn wipe_refuses_without_report() {
        with_test_data_dir(|| {
            let root = std::env::temp_dir().join("cpn-fp-nowipe");
            let err = wipe_panel(&root, "plesk").unwrap_err();
            assert!(err.contains("No verify report"));
        });
    }
}
