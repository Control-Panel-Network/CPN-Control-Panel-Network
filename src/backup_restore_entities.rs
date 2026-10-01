//! Discover restoreable entities inside a backup archive (domains, DBs, email, …).

use crate::backups::is_subdomain_site;
use std::collections::BTreeSet;

/// Kind of restoreable payload discovered in an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Site,
    Subdomain,
    WebsiteFiles,
    Database,
    Plugins,
    Email,
    Docker,
    Dns,
    PanelConfig,
    Users,
    Acl,
    Packages,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Subdomain => "subdomain",
            Self::WebsiteFiles => "website",
            Self::Database => "database",
            Self::Plugins => "plugins",
            Self::Email => "email",
            Self::Docker => "docker",
            Self::Dns => "dns",
            Self::PanelConfig => "panel-config",
            Self::Users => "users",
            Self::Acl => "acl",
            Self::Packages => "packages",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Site => "Domain / site",
            Self::Subdomain => "Subdomain",
            Self::WebsiteFiles => "Website files",
            Self::Database => "Database (MariaDB)",
            Self::Plugins => "Plugins folder",
            Self::Email => "Email (vmail)",
            Self::Docker => "Docker / compose",
            Self::Dns => "DNS / zone data",
            Self::PanelConfig => "Panel config",
            Self::Users => "Users / accounts",
            Self::Acl => "ACL / permissions",
            Self::Packages => "Packages / plans",
        }
    }
}

/// One selectable entity for multi-restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreEntity {
    /// Stable form value, e.g. `db:news_cms`, `site:example.com`, `email`.
    pub id: String,
    pub kind: EntityKind,
    pub label: String,
    pub detail: String,
    /// Needs an extra confirmation checkbox (email / docker / dns / panel-config).
    pub needs_extra_confirm: bool,
    /// Checked by default on the plan page.
    pub default_selected: bool,
}

/// Operator selection parsed from the restore form.
#[derive(Debug, Clone, Default)]
pub struct EntitySelection {
    pub ids: BTreeSet<String>,
}

impl EntitySelection {
    pub fn from_form_values(values: &[String]) -> Self {
        let mut ids = BTreeSet::new();
        for raw in values {
            let id = raw.trim();
            if !id.is_empty() {
                ids.insert(id.to_string());
            }
        }
        Self { ids }
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    pub fn wants_website(&self) -> bool {
        self.contains("website")
            || self
                .ids
                .iter()
                .any(|id| id.starts_with("site:") || id.starts_with("subdomain:"))
    }

    pub fn wants_plugins(&self) -> bool {
        self.contains("plugins")
    }

    pub fn wants_email(&self) -> bool {
        self.contains("email")
    }

    pub fn wants_docker(&self) -> bool {
        self.contains("docker")
    }

    pub fn wants_dns(&self) -> bool {
        self.contains("dns")
    }

    pub fn wants_panel_config(&self) -> bool {
        self.contains("panel-config")
    }

    pub fn wants_users(&self) -> bool {
        self.contains("users")
    }

    pub fn wants_acl(&self) -> bool {
        self.contains("acl")
    }

    pub fn wants_packages(&self) -> bool {
        self.contains("packages")
    }

    /// `None` means import all SQL (legacy full restore or "databases" dump).
    /// `Some(set)` filters by dump basename; empty set skips SQL entirely.
    pub fn database_names(&self) -> Option<BTreeSet<String>> {
        if self.is_empty() || self.contains("databases") {
            return None;
        }
        let dbs: BTreeSet<String> = self
            .ids
            .iter()
            .filter_map(|id| id.strip_prefix("db:").map(|s| s.to_string()))
            .collect();
        Some(dbs)
    }

    pub fn selected_domains(&self) -> Vec<String> {
        let mut out = Vec::new();
        for id in &self.ids {
            if let Some(d) = id.strip_prefix("site:") {
                out.push(d.to_ascii_lowercase());
            } else if let Some(d) = id.strip_prefix("subdomain:") {
                out.push(d.to_ascii_lowercase());
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Per-entity outcome for the restore summary UI.
#[derive(Debug, Clone)]
pub struct EntityStatus {
    pub id: String,
    pub label: String,
    pub status: &'static str,
    pub detail: String,
}

fn norm_member(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("./")
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn looks_like_domain(name: &str) -> bool {
    let n = name.trim().trim_end_matches('/').to_ascii_lowercase();
    if n.is_empty() || !n.contains('.') || n.len() > 253 {
        return false;
    }
    if n.contains(' ') || n.contains('/') {
        return false;
    }
    let bad = [
        "public_html",
        "homedir",
        "mysql",
        "userdata",
        "vmail",
        "plugins",
        "docker",
        "dns",
        "panel-config",
        "wp-content",
        "dup-installer",
        "meta.xml",
    ];
    if bad.iter().any(|b| n == *b) {
        return false;
    }
    n.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        && n.contains(char::is_alphabetic)
}

fn sql_db_name(path: &str) -> Option<String> {
    let base = path.rsplit('/').next().unwrap_or(path);
    let cleaned = base
        .trim_end_matches(".sql.gz")
        .trim_end_matches(".sql")
        .trim_end_matches(".gz");
    if cleaned.is_empty()
        || cleaned == "dump"
        || cleaned == "database"
        || cleaned == "db"
        || cleaned == "databases"
    {
        // `databases.sql` is an all-databases dump.
        if cleaned == "databases" {
            return Some("databases".into());
        }
        return None;
    }
    let name: String = cleaned
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let name = name.trim_matches('_').to_string();
    if name.is_empty() { None } else { Some(name) }
}

fn push_unique(entities: &mut Vec<RestoreEntity>, entity: RestoreEntity) {
    if entities.iter().any(|e| e.id == entity.id) {
        return;
    }
    entities.push(entity);
}

/// `backup-example.com-10.01.2026_19-33-23` -> `example.com`
pub fn infer_domain_from_archive_name(name: &str) -> Option<String> {
    let base = name
        .trim()
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tgz")
        .trim_end_matches(".tar")
        .trim_end_matches(".zip");
    let rest = base.strip_prefix("backup-")?;
    let candidates: Vec<&str> = rest
        .split('_')
        .flat_map(|p| p.split('-'))
        .filter(|p| p.contains('.') && p.contains(char::is_alphabetic))
        .collect();
    if rest.contains('-') {
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i + 4 < bytes.len() {
            if bytes[i] == b'-'
                && bytes[i + 1].is_ascii_digit()
                && bytes[i + 2].is_ascii_digit()
                && bytes[i + 3] == b'.'
            {
                let domain = rest[..i].trim().to_ascii_lowercase();
                if domain.contains('.') {
                    return Some(domain);
                }
            }
            i += 1;
        }
    }
    candidates
        .into_iter()
        .rev()
        .find(|c| c.matches('.').count() >= 1)
        .map(|c| c.to_ascii_lowercase())
}

/// Inspect archive member paths (and filename) and return selectable entities.
pub fn discover_entities(filename: &str, members: &[String]) -> Vec<RestoreEntity> {
    let paths: Vec<String> = members.iter().map(|m| norm_member(m)).collect();
    let mut entities = Vec::new();
    let mut domains: BTreeSet<String> = BTreeSet::new();

    if let Some(inferred) = infer_domain_from_archive_name(filename) {
        domains.insert(inferred);
    }

    let has_public_html = paths.iter().any(|p| {
        p == "public_html"
            || p.starts_with("public_html/")
            || p.contains("/public_html/")
            || p.ends_with("/public_html")
    });
    let has_homedir = paths
        .iter()
        .any(|p| p.starts_with("homedir/") || p == "homedir");
    let has_plugins = paths
        .iter()
        .any(|p| p == "plugins" || p.starts_with("plugins/"));
    let has_vmail = paths
        .iter()
        .any(|p| p == "vmail" || p.starts_with("vmail/"));
    let has_docker = paths.iter().any(|p| {
        p == "docker"
            || p.starts_with("docker/")
            || p.contains("docker-compose")
            || p.ends_with("/compose.yml")
            || p.ends_with("/compose.yaml")
    });
    let has_dns = paths.iter().any(|p| {
        p == "dns"
            || p.starts_with("dns/")
            || p.contains("/zones/")
            || p.ends_with(".zone")
            || p.contains("cloudflare")
    });
    let has_panel_config = paths
        .iter()
        .any(|p| p == "panel-config" || p.starts_with("panel-config/"));

    // cPanel userdata hostnames.
    for p in &paths {
        if let Some(rest) = p.strip_prefix("userdata/") {
            let host = rest.split('/').next().unwrap_or("").trim();
            if looks_like_domain(host) {
                domains.insert(host.to_string());
            }
        }
        // panel-config/sites/<domain>.json
        if let Some(rest) = p.strip_prefix("panel-config/sites/") {
            let file = rest.split('/').next().unwrap_or("");
            let host = file.trim_end_matches(".json");
            if looks_like_domain(host) {
                domains.insert(host.to_string());
            }
        }
        // Top-level domain directories: example.com/public_html/...
        if let Some((first, rest)) = p.split_once('/')
            && looks_like_domain(first)
            && (rest.starts_with("public_html")
                || rest == "public_html"
                || rest.ends_with(".sql")
                || rest.starts_with("mysql/"))
        {
            domains.insert(first.to_string());
        }
    }

    for domain in &domains {
        // Prefer structural FQDN depth; fall back to registry when parent already exists.
        let kind = if crate::sites::parent_domain_candidates(domain).is_empty()
            && !is_subdomain_site(domain)
        {
            EntityKind::Site
        } else {
            EntityKind::Subdomain
        };
        let prefix = if kind == EntityKind::Subdomain {
            "subdomain"
        } else {
            "site"
        };
        push_unique(
            &mut entities,
            RestoreEntity {
                id: format!("{prefix}:{domain}"),
                kind,
                label: format!("{} `{domain}`", kind.label()),
                detail: "Create or overwrite this hostname when selected.".into(),
                needs_extra_confirm: false,
                default_selected: true,
            },
        );
    }

    if has_public_html || has_homedir {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "website".into(),
                kind: EntityKind::WebsiteFiles,
                label: EntityKind::WebsiteFiles.label().into(),
                detail: if has_homedir {
                    "homedir/public_html (or public_html) into the target docroot.".into()
                } else {
                    "public_html into the target docroot.".into()
                },
                needs_extra_confirm: false,
                default_selected: true,
            },
        );
    }

    // Databases: individual dumps + all-databases marker.
    let mut db_names: BTreeSet<String> = BTreeSet::new();
    let mut has_all_db_dump = false;
    for p in &paths {
        if p.ends_with(".sql") || p.ends_with(".sql.gz") || p.ends_with("-db.gz") {
            if let Some(name) = sql_db_name(p) {
                if name == "databases" {
                    has_all_db_dump = true;
                } else {
                    db_names.insert(name);
                }
            }
        } else if p.contains("/mysql/") && p.ends_with('/') {
            // directory marker only
        }
    }
    if has_all_db_dump {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "databases".into(),
                kind: EntityKind::Database,
                label: "All databases dump (databases.sql)".into(),
                detail: "Imports the combined MariaDB dump when present.".into(),
                needs_extra_confirm: false,
                default_selected: true,
            },
        );
    }
    for name in db_names {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: format!("db:{name}"),
                kind: EntityKind::Database,
                label: format!("Database `{name}`"),
                detail: "Import this .sql dump into local MariaDB.".into(),
                needs_extra_confirm: false,
                default_selected: true,
            },
        );
    }

    if has_plugins {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "plugins".into(),
                kind: EntityKind::Plugins,
                label: EntityKind::Plugins.label().into(),
                detail: "Restore site plugins/ folder.".into(),
                needs_extra_confirm: false,
                default_selected: true,
            },
        );
    }
    if has_vmail {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "email".into(),
                kind: EntityKind::Email,
                label: EntityKind::Email.label().into(),
                detail: "Present in archive. Requires confirmation; import is best-effort / may be skipped.".into(),
                needs_extra_confirm: true,
                default_selected: false,
            },
        );
    }
    if has_docker {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "docker".into(),
                kind: EntityKind::Docker,
                label: EntityKind::Docker.label().into(),
                detail:
                    "Present in archive. Requires confirmation; stacks are not auto-recreated yet."
                        .into(),
                needs_extra_confirm: true,
                default_selected: false,
            },
        );
    }
    if has_dns {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "dns".into(),
                kind: EntityKind::Dns,
                label: EntityKind::Dns.label().into(),
                detail: "Present in archive. Requires confirmation; Cloudflare/DNS apply is best-effort.".into(),
                needs_extra_confirm: true,
                default_selected: false,
            },
        );
    }
    if has_panel_config {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "panel-config".into(),
                kind: EntityKind::PanelConfig,
                label: EntityKind::PanelConfig.label().into(),
                detail: "Panel config payload. Requires confirmation; site restore does not overwrite live MFA keys.".into(),
                needs_extra_confirm: true,
                default_selected: false,
            },
        );
    }

    let (has_users, has_acl, has_packages) =
        crate::backup_restore_accounts::members_suggest_accounts_payload(members);
    if has_users {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "users".into(),
                kind: EntityKind::Users,
                label: EntityKind::Users.label().into(),
                detail: "Owner account from classic meta.xml and/or optional users.json. Requires confirmation; passwords are reset (source hashes are not portable). MFA is never wiped.".into(),
                needs_extra_confirm: true,
                default_selected: false,
            },
        );
    }
    if has_acl {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "acl".into(),
                kind: EntityKind::Acl,
                label: EntityKind::Acl.label().into(),
                detail: "Best-effort map of source aclName (and optional site-acl.json) into CPN site ACL grants. Not 1:1 source-panel ACL parity.".into(),
                needs_extra_confirm: true,
                default_selected: false,
            },
        );
    }
    if has_packages {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "packages".into(),
                kind: EntityKind::Packages,
                label: EntityKind::Packages.label().into(),
                detail: "Best-effort package from websites limit in meta.xml and/or optional packages.json. Typical website archives lack a full package catalog.".into(),
                needs_extra_confirm: true,
                default_selected: false,
            },
        );
    }

    // Always expose at least website when nothing structured was found but members exist.
    if entities.is_empty() && !paths.is_empty() {
        push_unique(
            &mut entities,
            RestoreEntity {
                id: "website".into(),
                kind: EntityKind::WebsiteFiles,
                label: "Archive contents (manual)".into(),
                detail: "Format could not be fully inventoried; restore will use format-specific defaults.".into(),
                needs_extra_confirm: false,
                default_selected: true,
            },
        );
    }

    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_classic_multi_db_and_email() {
        let members = vec![
            "./meta.xml".into(),
            "./public_html/index.html".into(),
            "./news_cms.sql".into(),
            "./news_disco.sql".into(),
            "./vmail/user/Maildir/cur/1".into(),
        ];
        let ents = discover_entities("backup-example.com-01.02.2026_12-00-00.tar.gz", &members);
        let ids: Vec<_> = ents.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"site:example.com") || ids.contains(&"subdomain:example.com"));
        assert!(ids.contains(&"website"));
        assert!(ids.contains(&"db:news_cms"));
        assert!(ids.contains(&"db:news_disco"));
        assert!(ids.contains(&"email"));
        assert!(ids.contains(&"users"));
        assert!(ids.contains(&"acl"));
        assert!(ids.contains(&"packages"));
        let email = ents.iter().find(|e| e.id == "email").unwrap();
        assert!(email.needs_extra_confirm);
        assert!(!email.default_selected);
        let users = ents.iter().find(|e| e.id == "users").unwrap();
        assert!(users.needs_extra_confirm);
        assert!(!users.default_selected);
    }

    #[test]
    fn discovers_cpanel_userdata_domains_and_mysql() {
        let members = vec![
            "./homedir/public_html/index.php".into(),
            "./mysql/user_wp.sql".into(),
            "./userdata/example.com".into(),
            "./userdata/blog.example.com".into(),
            "./dns/example.com.zone".into(),
        ];
        let ents = discover_entities("cpmove-user.tar.gz", &members);
        let ids: Vec<_> = ents.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.iter().any(|id| id.contains("example.com")));
        assert!(ids.contains(&"db:user_wp"));
        assert!(ids.contains(&"website"));
        assert!(ids.contains(&"dns"));
    }

    #[test]
    fn selection_filters_databases() {
        let sel = EntitySelection::from_form_values(&[
            "website".into(),
            "db:news_cms".into(),
            "site:example.com".into(),
        ]);
        assert!(sel.wants_website());
        assert_eq!(
            sel.database_names()
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["news_cms".to_string()]
        );
        assert_eq!(sel.selected_domains(), vec!["example.com".to_string()]);
    }

    #[test]
    fn empty_selection_means_legacy_all_databases() {
        let sel = EntitySelection::default();
        assert!(sel.database_names().is_none());
    }

    #[test]
    fn infer_domain_from_classic_backup_name() {
        assert_eq!(
            infer_domain_from_archive_name("backup-newstargeted.com-10.01.2026_19-33-23.tar.gz")
                .as_deref(),
            Some("newstargeted.com")
        );
        assert_eq!(
            infer_domain_from_archive_name("backup-example.co.uk-01.02.2026_12-00-00").as_deref(),
            Some("example.co.uk")
        );
        assert!(infer_domain_from_archive_name("site-files.zip").is_none());
    }
}
