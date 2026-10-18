//! Fixed catalog of other panels CPN can detect on this host.
//!
//! Display names are safe for the panel UI. The classic layout is never given
//! that product's brand name here.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyTier {
    Primary,
    LongTail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanKind {
    UserDomains,
    DomainHomes,
    VhostHttpdocs,
    DirectAdmin,
    /// `base` is the data/users directory (Hestia or Vesta).
    WebConf,
    Virtualmin,
    WwwRoot,
    Ispconfig,
    Ispmanager,
    ApisCp,
    Sentora,
    BlueOnyx,
    Froxlor,
    Ehcp,
    Easypanel,
    Zesle,
    /// Marker hit only. Inventory stays empty until a layout is confirmed.
    MarkerOnly,
}

#[derive(Clone, Copy, Debug)]
pub struct PanelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub tier: CopyTier,
    pub markers: &'static [&'static str],
    pub scan: ScanKind,
    /// Default listen port. `2087` conflicts with CPN unless the operator confirms.
    pub listen_port: u16,
    /// Install prefixes removed on wipe (relative, at least three components).
    pub wipe_rel: &'static [&'static str],
    /// Relative directory that holds per-user `web.conf` when `scan` is `WebConf`.
    pub web_conf_users: &'static str,
}

pub const CATALOG: &[PanelSpec] = &[
    PanelSpec {
        id: "cpanel",
        name: "cPanel",
        tier: CopyTier::Primary,
        markers: &["usr/local/cpanel", "usr/local/cpanel/whostmgr"],
        scan: ScanKind::UserDomains,
        listen_port: 2087,
        wipe_rel: &["usr/local/cpanel"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "classic",
        name: "Classic control panel",
        tier: CopyTier::Primary,
        markers: &["usr/local/CyberCP"],
        scan: ScanKind::DomainHomes,
        listen_port: 8090,
        wipe_rel: &["usr/local/CyberCP"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "plesk",
        name: "Plesk",
        tier: CopyTier::Primary,
        markers: &["usr/local/psa", "opt/psa"],
        scan: ScanKind::VhostHttpdocs,
        listen_port: 8443,
        wipe_rel: &["usr/local/psa", "opt/psa"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "directadmin",
        name: "DirectAdmin",
        tier: CopyTier::Primary,
        markers: &["usr/local/directadmin"],
        scan: ScanKind::DirectAdmin,
        listen_port: 2222,
        wipe_rel: &["usr/local/directadmin"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "hestiacp",
        name: "HestiaCP",
        tier: CopyTier::Primary,
        markers: &["usr/local/hestia"],
        scan: ScanKind::WebConf,
        listen_port: 8083,
        wipe_rel: &["usr/local/hestia"],
        web_conf_users: "usr/local/hestia/data/users",
    },
    PanelSpec {
        id: "virtualmin",
        name: "Virtualmin",
        tier: CopyTier::Primary,
        markers: &["etc/webmin/virtual-server", "usr/sbin/virtualmin"],
        scan: ScanKind::Virtualmin,
        listen_port: 10000,
        wipe_rel: &["etc/webmin/virtual-server"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "aapanel",
        name: "aaPanel",
        tier: CopyTier::Primary,
        markers: &["www/server/panel"],
        scan: ScanKind::WwwRoot,
        listen_port: 7800,
        wipe_rel: &["www/server/panel"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "cwp",
        name: "Control Web Panel",
        tier: CopyTier::Primary,
        markers: &["usr/local/cwpsrv"],
        scan: ScanKind::UserDomains,
        listen_port: 2031,
        wipe_rel: &["usr/local/cwpsrv"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "ispconfig",
        name: "ISPConfig",
        tier: CopyTier::Primary,
        markers: &["usr/local/ispconfig"],
        scan: ScanKind::Ispconfig,
        listen_port: 8080,
        wipe_rel: &["usr/local/ispconfig"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "ispmanager",
        name: "ispmanager",
        tier: CopyTier::LongTail,
        markers: &["usr/local/mgr5"],
        scan: ScanKind::Ispmanager,
        listen_port: 1500,
        wipe_rel: &["usr/local/mgr5"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "vesta",
        name: "Vesta Control Panel",
        tier: CopyTier::LongTail,
        markers: &["usr/local/vesta"],
        scan: ScanKind::WebConf,
        listen_port: 8083,
        wipe_rel: &["usr/local/vesta"],
        web_conf_users: "usr/local/vesta/data/users",
    },
    PanelSpec {
        id: "ajenti",
        name: "Ajenti",
        tier: CopyTier::LongTail,
        markers: &["etc/ajenti", "var/lib/ajenti"],
        scan: ScanKind::MarkerOnly,
        listen_port: 8000,
        wipe_rel: &["etc/ajenti", "var/lib/ajenti"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "sentora",
        name: "Sentora",
        tier: CopyTier::LongTail,
        markers: &["etc/sentora"],
        scan: ScanKind::Sentora,
        listen_port: 0,
        wipe_rel: &["etc/sentora"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "blueonyx",
        name: "BlueOnyx",
        tier: CopyTier::LongTail,
        markers: &["usr/sausalito"],
        scan: ScanKind::BlueOnyx,
        listen_port: 444,
        wipe_rel: &["usr/sausalito"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "froxlor",
        name: "Froxlor",
        tier: CopyTier::LongTail,
        markers: &["etc/froxlor", "var/www/froxlor"],
        scan: ScanKind::Froxlor,
        listen_port: 0,
        wipe_rel: &["etc/froxlor", "var/www/froxlor"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "ehcp",
        name: "Easy Hosting Control Panel",
        tier: CopyTier::LongTail,
        markers: &["etc/ehcp", "var/www/new/ehcp"],
        scan: ScanKind::Ehcp,
        listen_port: 0,
        wipe_rel: &["etc/ehcp", "var/www/new/ehcp"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "webmin",
        name: "Webmin",
        tier: CopyTier::LongTail,
        markers: &["etc/webmin", "usr/share/webmin"],
        scan: ScanKind::MarkerOnly,
        listen_port: 10000,
        wipe_rel: &["usr/share/webmin"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "apiscp",
        name: "ApisCP",
        tier: CopyTier::LongTail,
        markers: &["usr/local/apnscp"],
        scan: ScanKind::ApisCp,
        listen_port: 0,
        wipe_rel: &["usr/local/apnscp"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "zesle",
        name: "Zesle Control Panel",
        tier: CopyTier::LongTail,
        markers: &["usr/local/zesle", "etc/zesle", "opt/zesle"],
        scan: ScanKind::Zesle,
        listen_port: 0,
        wipe_rel: &["usr/local/zesle", "etc/zesle", "opt/zesle"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "kloxo",
        name: "Kloxo",
        tier: CopyTier::LongTail,
        markers: &["usr/local/lxlabs"],
        scan: ScanKind::DomainHomes,
        listen_port: 7777,
        wipe_rel: &["usr/local/lxlabs"],
        web_conf_users: "",
    },
    PanelSpec {
        id: "easypanel",
        name: "Easypanel",
        tier: CopyTier::LongTail,
        markers: &["etc/easypanel", "var/lib/easypanel"],
        scan: ScanKind::Easypanel,
        listen_port: 3000,
        wipe_rel: &["etc/easypanel", "var/lib/easypanel"],
        web_conf_users: "",
    },
];

pub fn spec_by_id(id: &str) -> Option<&'static PanelSpec> {
    let key = id.trim().to_ascii_lowercase();
    CATALOG.iter().find(|spec| spec.id == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_every_requested_panel() {
        let ids = [
            "plesk",
            "cwp",
            "ispconfig",
            "virtualmin",
            "ispmanager",
            "vesta",
            "directadmin",
            "ajenti",
            "sentora",
            "blueonyx",
            "cpanel",
            "froxlor",
            "ehcp",
            "webmin",
            "apiscp",
            "aapanel",
            "zesle",
            "kloxo",
            "classic",
            "easypanel",
            "hestiacp",
        ];
        for id in ids {
            assert!(spec_by_id(id).is_some(), "missing {id}");
        }
        assert_eq!(CATALOG.len(), ids.len());
    }

    #[test]
    fn classic_ui_name_has_no_product_brand() {
        let classic = spec_by_id("classic").unwrap();
        let lower = classic.name.to_ascii_lowercase();
        assert!(!lower.contains("cyber"));
        assert_eq!(classic.name, "Classic control panel");
    }
}
