//! Host packages store-like catalog metadata (dates, Featured, categories).

use crate::apps::{AppId, AppStateKind, AppStatus};
use crate::plugins_catalog::{FEATURED_MIN_INSTALLS, FEATURED_TOP_N, format_iso_date_eu};

#[derive(Debug, Clone, Copy)]
pub struct HostPackageMeta {
    pub category: &'static str,
    pub version: &'static str,
    pub pricing: &'static str,
    pub released_on: &'static str,
    pub updated_on: &'static str,
    pub install_count: u64,
    pub featured: bool,
    pub description: &'static str,
    /// LIVE install vs honest non-LIVE status note shown on the card.
    pub install_status: HostInstallStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostInstallStatus {
    Live,
    /// Shown in UI; Install returns a clear error / gate message.
    RequiresNextcloud,
    /// Registry + UI only; not a working host install yet.
    Scaffold,
}

impl HostInstallStatus {
    pub fn badge(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::RequiresNextcloud => "Needs Nextcloud",
            Self::Scaffold => "SCAFFOLD",
        }
    }
}

pub fn meta_for(id: AppId) -> HostPackageMeta {
    match id {
        AppId::Mariadb => HostPackageMeta {
            category: "Database",
            version: "system",
            pricing: "free",
            released_on: "2024-06-01",
            updated_on: "2026-08-15",
            install_count: 200,
            featured: true,
            description: "Default MariaDB server for CPN hosting (MySQL-compatible; CPN does not install Oracle MySQL).",
            install_status: HostInstallStatus::Live,
        },
        AppId::Postgresql => HostPackageMeta {
            category: "Database",
            version: "system",
            pricing: "free",
            released_on: "2025-11-01",
            updated_on: "2026-07-20",
            install_count: 35,
            featured: false,
            description: "Opt-in PostgreSQL. Can coexist with MariaDB.",
            install_status: HostInstallStatus::Live,
        },
        AppId::Phpmyadmin => HostPackageMeta {
            category: "Utility",
            version: "system",
            pricing: "free",
            released_on: "2024-06-01",
            updated_on: "2026-09-01",
            install_count: 180,
            featured: true,
            description: "phpMyAdmin UI reverse-proxied under the panel for MariaDB.",
            install_status: HostInstallStatus::Live,
        },
        AppId::Email => HostPackageMeta {
            category: "Email",
            version: "system",
            pricing: "free",
            released_on: "2024-08-01",
            updated_on: "2026-09-10",
            install_count: 150,
            featured: true,
            description: "Postfix + Dovecot IMAP/SMTP stack used by webmail clients.",
            install_status: HostInstallStatus::Live,
        },
        AppId::Rabbitmq => HostPackageMeta {
            category: "Utility",
            version: "system",
            pricing: "free",
            released_on: "2025-03-01",
            updated_on: "2026-06-01",
            install_count: 12,
            featured: false,
            description: "RabbitMQ message broker (AMQP) as an optional host package.",
            install_status: HostInstallStatus::Live,
        },
        AppId::Snappymail => HostPackageMeta {
            category: "Email",
            version: "2.38.2",
            pricing: "free",
            released_on: "2024-10-09",
            updated_on: "2026-09-01",
            install_count: 160,
            featured: true,
            description: "Default CPN webmail (SnappyMail) under /opt/cpn-webmail with panel proxy.",
            install_status: HostInstallStatus::Live,
        },
        AppId::Tachyon => HostPackageMeta {
            category: "Email",
            version: "4.2.3",
            pricing: "free",
            released_on: "2025-01-01",
            updated_on: "2026-09-11",
            install_count: 20,
            featured: false,
            description: "Modern SnappyMail fork (Tachyon): mail, contacts, calendars. PHP 8.2+, no DB required.",
            install_status: HostInstallStatus::Live,
        },
        AppId::Nextsnapmail => HostPackageMeta {
            category: "Email",
            version: "app",
            pricing: "free",
            released_on: "2025-06-01",
            updated_on: "2026-08-01",
            install_count: 8,
            featured: false,
            description: "NextSnapMail is a Nextcloud App Store webmail (SnappyMail fork). CPN does not offer a standalone host install without Nextcloud.",
            install_status: HostInstallStatus::RequiresNextcloud,
        },
        AppId::Sogo => HostPackageMeta {
            category: "Email",
            version: "scaffold",
            pricing: "free",
            released_on: "2025-01-01",
            updated_on: "2026-09-01",
            install_count: 5,
            featured: false,
            description: "SOGo groupware (webmail + CalDAV/CardDAV). Full Inverse package install is not LIVE yet; listed for registry and installer selection.",
            install_status: HostInstallStatus::Scaffold,
        },
    }
}

pub fn host_package_is_featured(id: AppId, all: &[AppStatus]) -> bool {
    let meta = meta_for(id);
    if meta.featured {
        return true;
    }
    if meta.install_count >= FEATURED_MIN_INSTALLS {
        return true;
    }
    if meta.install_count == 0 {
        return false;
    }
    let mut ranked: Vec<(AppId, u64)> = all
        .iter()
        .map(|s| (s.id, meta_for(s.id).install_count))
        .filter(|(_, c)| *c > 0)
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.as_str().cmp(b.0.as_str())));
    ranked
        .into_iter()
        .take(FEATURED_TOP_N)
        .any(|(aid, _)| aid == id)
}

pub fn format_host_dates(meta: &HostPackageMeta) -> String {
    let mut parts = Vec::new();
    if !meta.released_on.is_empty() {
        parts.push(format!(
            "Released: {}",
            format_iso_date_eu(meta.released_on)
        ));
    }
    if !meta.updated_on.is_empty() {
        parts.push(format!("Updated: {}", format_iso_date_eu(meta.updated_on)));
    }
    parts.join(" · ")
}

pub fn filter_host_packages<'a>(
    apps: &'a [AppStatus],
    query: &str,
    category: &str,
) -> Vec<&'a AppStatus> {
    let q = query.trim().to_ascii_lowercase();
    let cat = category.trim().to_ascii_lowercase();
    apps.iter()
        .filter(|status| {
            let meta = meta_for(status.id);
            let cat_ok = if cat.is_empty() || cat == "all" {
                true
            } else if cat == "featured" {
                host_package_is_featured(status.id, apps)
            } else {
                meta.category.eq_ignore_ascii_case(category.trim())
            };
            if !cat_ok {
                return false;
            }
            if q.is_empty() {
                return true;
            }
            status.id.label().to_ascii_lowercase().contains(&q)
                || status.id.as_str().contains(&q)
                || meta.description.to_ascii_lowercase().contains(&q)
                || meta.category.to_ascii_lowercase().contains(&q)
        })
        .collect()
}

pub fn host_categories(apps: &[AppStatus]) -> Vec<&'static str> {
    let mut cats: Vec<&'static str> = apps.iter().map(|a| meta_for(a.id).category).collect();
    cats.sort_by_key(|c| c.to_ascii_lowercase());
    cats.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    cats
}

pub fn state_label(state: AppStateKind) -> &'static str {
    state.label()
}
