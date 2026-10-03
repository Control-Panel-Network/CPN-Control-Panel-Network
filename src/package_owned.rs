//! Create/update helpers that enforce `{owner}_{custom}` package names.

use crate::package_naming::{
    normalize_owned_package_name, package_owner_from_name,
};
use crate::packages::{
    DEFAULT_PACKAGE_ID, Package, PackageInput, create_package, get_package, update_package,
};

fn names_equal(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Create a hosting package named `{owner}_{custom}` (never renames Default).
pub fn create_package_for(owner: &str, mut input: PackageInput) -> Result<Package, String> {
    input.name = normalize_owned_package_name(owner, &input.name)?;
    create_package(input)
}

/// Update limits/name. Default (`pkg-default`) keeps the exact name `Default`.
/// Prefixed packages keep their owner prefix; only the custom part changes.
pub fn update_package_for(
    id: &str,
    owner_fallback: &str,
    mut input: PackageInput,
) -> Result<Package, String> {
    let existing = get_package(id)?;
    if existing.id == DEFAULT_PACKAGE_ID || names_equal(&existing.name, "Default") {
        input.name = "Default".into();
    } else {
        let owner = package_owner_from_name(&existing.name)
            .unwrap_or_else(|| owner_fallback.trim().to_string());
        input.name = normalize_owned_package_name(&owner, &input.name)?;
    }
    update_package(id, input)
}
