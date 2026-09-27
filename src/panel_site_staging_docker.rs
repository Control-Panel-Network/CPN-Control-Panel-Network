//! Clone site-linked Docker Compose stacks for staging (separate project + data).

use crate::panel_ops_docker_compose::{
    CPN_MANAGED_LABEL, ComposeStackRow, compose_host_data_root, compose_projects_root,
    find_compose_file, list_compose_stacks, run_compose, validate_stack_id,
};
use crate::sites::{SiteRecord, site_home_from_record};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn stacks_linked_to_site(source: &SiteRecord) -> Vec<ComposeStackRow> {
    let Ok(stacks) = list_compose_stacks() else {
        return Vec::new();
    };
    let slug = source.domain.replace('.', "-");
    let slug_us = source.domain.replace('.', "_");
    let src_home = site_home_from_record(source);
    stacks
        .into_iter()
        .filter(|stack| {
            let compose_text = fs::read_to_string(&stack.compose_file).unwrap_or_default();
            stack.id.contains(&slug)
                || stack.id.contains(&slug_us)
                || compose_text.contains(&source.domain)
                || compose_text.contains(&src_home.display().to_string())
        })
        .collect()
}

pub fn clone_stack_for_staging(
    stack: &ComposeStackRow,
    source: &SiteRecord,
    target: &SiteRecord,
    owner: &str,
) -> Result<String, String> {
    let new_id = staging_stack_id(&stack.id, &target.domain)?;
    let new_project = compose_projects_root().join(&new_id);
    if new_project.exists() {
        return Err(format!(
            "Staging stack `{new_id}` already exists; pick another staging hostname or remove the old stack."
        ));
    }

    let dst_data = compose_host_data_root().join(&new_id).join("data");
    if stack.data_dir.is_dir() {
        fs::create_dir_all(dst_data.parent().unwrap_or(&dst_data))
            .map_err(|e| format!("mkdir docker-data: {e}"))?;
        copy_dir_unix(&stack.data_dir, &dst_data)?;
    } else {
        fs::create_dir_all(&dst_data).map_err(|e| format!("mkdir empty data: {e}"))?;
    }

    let src_home = site_home_from_record(source).display().to_string();
    let dst_home = site_home_from_record(target).display().to_string();
    let mut body =
        fs::read_to_string(&stack.compose_file).map_err(|e| format!("read compose: {e}"))?;
    body = body.replace(&src_home, &dst_home);
    body = body.replace(&source.domain, &target.domain);
    body = body.replace(
        &format!("com.cpn.stack: \"{}\"", stack.id),
        &format!("com.cpn.stack: \"{new_id}\""),
    );
    body = body.replace(
        &format!("com.cpn.stack: '{}'", stack.id),
        &format!("com.cpn.stack: '{new_id}'"),
    );
    if body.contains(CPN_MANAGED_LABEL) {
        let owner = owner.trim();
        let owner = if owner.is_empty() { "CPN" } else { owner };
        if !body.contains("com.cpn.staging_of") {
            body = body.replace(
                &format!("com.cpn.owner: \"{owner}\""),
                &format!(
                    "com.cpn.owner: \"{owner}\"\n      com.cpn.staging_of: \"{}\"",
                    source.domain
                ),
            );
        }
    }

    fs::create_dir_all(&new_project).map_err(|e| format!("mkdir stack project: {e}"))?;
    fs::write(new_project.join("compose.yml"), &body).map_err(|e| format!("write compose: {e}"))?;

    let env_src = stack.project_dir.join(".env");
    if env_src.is_file() {
        let mut env_body = fs::read_to_string(&env_src).map_err(|e| format!("read .env: {e}"))?;
        env_body = env_body.replace(&source.domain, &target.domain);
        env_body = env_body.replace(&src_home, &dst_home);
        let _ = fs::write(new_project.join(".env"), env_body);
    }

    run_compose(&new_project, &["up", "-d", "--remove-orphans"])?;
    Ok(format!(
        "Cloned compose stack `{}` to `{}` (data at `{}`). Production stack `{}` is unchanged.",
        stack.id,
        new_id,
        dst_data.display(),
        stack.id
    ))
}

fn staging_stack_id(source_id: &str, target_domain: &str) -> Result<String, String> {
    let slug = target_domain.replace('.', "-");
    let mut candidate = format!("stg-{slug}-{source_id}");
    if candidate.len() > 48 {
        let budget = 48usize.saturating_sub(slug.len() + 5);
        let trimmed: String = source_id.chars().take(budget.max(4)).collect();
        candidate = format!("stg-{slug}-{trimmed}");
    }
    validate_stack_id(&candidate)
}

fn copy_dir_unix(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        fs::create_dir_all(dst).map_err(|e| format!("mkdir: {e}"))?;
        let status = Command::new("cp")
            .arg("-a")
            .arg(format!("{}/.", src.display()))
            .arg(dst)
            .status()
            .map_err(|e| format!("cp data: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("cp -a failed while copying compose stack data".into())
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (src, dst);
        Err("Compose stack data copy requires a Unix host".into())
    }
}

/// True when compose file exists and stack id is valid (post-clone sanity).
#[allow(dead_code)]
pub fn stack_project_ready(stack_id: &str) -> bool {
    let dir = compose_projects_root().join(stack_id);
    dir.is_dir() && find_compose_file(&dir).is_some()
}
