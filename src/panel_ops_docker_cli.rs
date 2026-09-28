//! Shared helpers for matching user container references to engine-listed rows.

use crate::panel_ops_docker::{DockerContainerRow, list_containers_detailed};

pub(crate) fn is_safe_docker_container_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn validate_container_ref(user_ref: &str) -> Result<&str, String> {
    let user_ref = user_ref.trim();
    if user_ref.is_empty()
        || user_ref.contains('/')
        || user_ref.contains('\\')
        || user_ref.contains(' ')
        || user_ref.contains(';')
    {
        return Err("Invalid container name.".into());
    }
    Ok(user_ref)
}

pub(crate) fn row_matches_ref(row: &DockerContainerRow, user_ref: &str) -> bool {
    row.name == user_ref || row.id == user_ref || row.id.starts_with(user_ref)
}

pub(crate) fn with_listed_container_id<T, F>(user_ref: &str, run: F) -> Result<T, String>
where
    F: FnOnce(&DockerContainerRow) -> Result<T, String>,
{
    let user_ref = validate_container_ref(user_ref)?;
    let rows = list_containers_detailed()?;
    for row in rows {
        if !row_matches_ref(&row, user_ref) {
            continue;
        }
        if !is_safe_docker_container_id(&row.id) {
            continue;
        }
        return run(&row);
    }
    Err(format!(
        "Container `{user_ref}` was not found on this host."
    ))
}
