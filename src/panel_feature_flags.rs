//! Host feature flags under /var/lib/cpn/features/<slug>.enabled.
//!
//! Used by optional Email plugins (MTA-STS / BIMI) and catalog install hooks.

use crate::account::data_dir;
use std::fs;

fn features_dir() -> std::path::PathBuf {
    data_dir().join("features")
}

/// Host feature flag path: /var/lib/cpn/features/<slug>.enabled.
pub fn feature_flag_path(slug: &str) -> std::path::PathBuf {
    features_dir().join(format!("{slug}.enabled"))
}

pub fn host_feature_enabled(slug: &str) -> bool {
    feature_flag_path(slug).is_file()
}

pub fn write_host_feature_flag(slug: &str) -> Result<(), String> {
    let dir = features_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create features dir: {e}"))?;
    let path = feature_flag_path(slug);
    fs::write(&path, b"enabled\n").map_err(|e| format!("Could not write feature flag: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
    }
    Ok(())
}

pub fn clear_host_feature_flag(slug: &str) {
    let path = feature_flag_path(slug);
    let _ = fs::remove_file(path);
}
