//! Host resource reservation: panel slice + package-driven site limits.

mod package_enforce;
mod panel_reserve;

pub use package_enforce::{
    apply_package_resources_for_id, apply_site_package_resources, host_limits_for_account,
    sync_all_site_package_resources,
};
pub use panel_reserve::{PANEL_SLICE_NAME, PANEL_SLICE_PATH, ensure_panel_resource_reserve};

/// Install/upgrade heal: panel reserve then package sync.
pub fn heal_host_resources() -> Result<String, String> {
    let panel = ensure_panel_resource_reserve()?;
    let sites = sync_all_site_package_resources().unwrap_or_else(|e| e);
    Ok(format!("{panel}; {sites}"))
}
