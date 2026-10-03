//! Per-site app snapshot (CMS Made Simple, Redis attach, Node, Python).

use crate::apps::{AppId, AppStateKind, detect_app};
use crate::apps_site::bindings_for_domain;
use crate::panel_site_apps_cmsms::CmsmsStatus;
use crate::panel_site_apps_runtime::{RuntimeKind, RuntimeStatus, load_runtime};
use crate::sites::SiteRecord;
use crate::wordpress_wpcli::is_wordpress_docroot;

#[derive(Debug, Clone)]
pub struct SiteAppsSnapshot {
    pub cmsms: CmsmsStatus,
    pub wordpress_in_docroot: bool,
    pub redis_host: AppStateKind,
    pub redis_detail: String,
    pub redis_attached: bool,
    pub node: RuntimeStatus,
    pub python: RuntimeStatus,
}

pub fn snapshot_site_apps(site: &SiteRecord) -> SiteAppsSnapshot {
    let redis = detect_app(AppId::Redis);
    let redis_attached = bindings_for_domain(&site.domain)
        .iter()
        .any(|b| b.app == AppId::Redis.as_str());
    SiteAppsSnapshot {
        cmsms: crate::panel_site_apps_cmsms::detect_cmsms(site),
        wordpress_in_docroot: is_wordpress_docroot(std::path::Path::new(&site.docroot)),
        redis_host: redis.state,
        redis_detail: redis.detail,
        redis_attached,
        node: load_runtime(site, RuntimeKind::Node),
        python: load_runtime(site, RuntimeKind::Python),
    }
}

pub fn jail_rel_under_home(home: &std::path::Path, rel: &str) -> Result<std::path::PathBuf, String> {
    let trimmed = rel.trim().trim_start_matches('/').trim_start_matches('\\');
    if trimmed.is_empty() {
        return Err("App path is required".into());
    }
    if trimmed.contains('\0') || trimmed.contains("..") {
        return Err("App path is not allowed".into());
    }
    let mut out = home.to_path_buf();
    for part in trimmed.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err("App path is not allowed".into());
        }
        out.push(part);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn jail_rejects_parent_segments() {
        let home = Path::new("/home/example.com");
        assert!(jail_rel_under_home(home, "../etc").is_err());
        assert!(jail_rel_under_home(home, "apps/node").is_ok());
    }
}
