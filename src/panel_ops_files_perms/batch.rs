//! Batch Fix permissions with per-item skip summary.

use super::apply::{ensure_fixable, fix_one_path, running_as_root};
use super::policy::resolve_ownership_policy;
use super::{DIR_MODE, FILE_MODE, OwnershipFixPolicy};
use crate::panel_ops_path::{join_child, resolve_under_jail};
use std::path::Path;

/// Cap skip reasons so redirect `notice` / `error` query strings stay short.
pub(super) const MAX_SKIP_REASON_CHARS: usize = 280;
const MAX_SKIP_ITEMS_IN_REPORT: usize = 8;

pub(super) fn shorten_skip_reason(name: &str, err: &str) -> String {
    let raw = format!("{name}: {err}");
    if raw.chars().count() <= MAX_SKIP_REASON_CHARS {
        return raw;
    }
    let truncated: String = raw
        .chars()
        .take(MAX_SKIP_REASON_CHARS.saturating_sub(3))
        .collect();
    format!("{truncated}...")
}

pub(super) fn format_skip_summary(skipped: &[String]) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    let shown: Vec<&str> = skipped
        .iter()
        .take(MAX_SKIP_ITEMS_IN_REPORT)
        .map(String::as_str)
        .collect();
    let extra = skipped.len().saturating_sub(shown.len());
    let mut out = shown.join("; ");
    if extra > 0 {
        out.push_str(&format!("; +{extra} more"));
    }
    out
}

/// Fix permissions for selected names under `parent` inside `jail`.
///
/// Continues after per-item skips/failures so Root FM Select All under `/home`
/// can fix registered site homes without aborting on ops/non-site folders.
pub fn fix_permissions(parent: &str, names: &[String], jail: &Path) -> Result<String, String> {
    if names.is_empty() {
        return Err("Nothing selected".into());
    }
    if !running_as_root() {
        return Err("Fix permissions requires root on the panel host".into());
    }
    let parent = resolve_under_jail(parent, jail)?;
    let mut total = 0usize;
    let mut fixed_items = 0usize;
    let mut any_recursive = false;
    let mut set_users: Vec<String> = Vec::new();
    let mut modes_only = 0usize;
    let mut skipped: Vec<String> = Vec::new();

    for name in names {
        let target = match join_child(&parent, name, jail) {
            Ok(p) => p,
            Err(e) => {
                skipped.push(shorten_skip_reason(name, &e));
                continue;
            }
        };
        if let Err(e) = ensure_fixable(&target, jail) {
            skipped.push(shorten_skip_reason(name, &e));
            continue;
        }
        let policy = match resolve_ownership_policy(&target) {
            Ok(p) => p,
            Err(e) => {
                skipped.push(shorten_skip_reason(name, &e));
                continue;
            }
        };
        match &policy {
            OwnershipFixPolicy::SetUser { user } => {
                if !set_users.iter().any(|u| u == user) {
                    set_users.push(user.clone());
                }
            }
            OwnershipFixPolicy::ModesOnly => modes_only += 1,
        }
        match fix_one_path(&target, &policy) {
            Ok((count, recursive)) => {
                total += count;
                fixed_items += 1;
                any_recursive |= recursive;
            }
            Err(e) => skipped.push(shorten_skip_reason(name, &e)),
        }
    }

    let skip_summary = format_skip_summary(&skipped);
    if fixed_items == 0 {
        return Err(if skip_summary.is_empty() {
            "Could not fix permissions on any selected item".into()
        } else {
            format!(
                "Could not fix permissions on any of {} selected item(s). Skipped: {skip_summary}",
                names.len()
            )
        });
    }

    let policy_label = if !set_users.is_empty() && modes_only == 0 {
        format!(
            "owner `{}`",
            set_users
                .iter()
                .map(|u| format!("{u}:{u}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else if set_users.is_empty() && modes_only > 0 {
        "modes only (existing non-root owner kept)".into()
    } else {
        format!(
            "mixed policy (site users {}; {} modes-only)",
            set_users
                .iter()
                .map(|u| format!("{u}:{u}"))
                .collect::<Vec<_>>()
                .join(", "),
            modes_only
        )
    };
    let scope = if any_recursive {
        "recursive for folders"
    } else {
        "selected files"
    };
    let mut msg = format!(
        "Fixed permissions on {total} path(s) across {fixed_items} item(s) ({scope}; dirs {DIR_MODE:o}, files {FILE_MODE:o}; {policy_label})"
    );
    if !skipped.is_empty() {
        msg.push_str(&format!(
            ". Skipped {} item(s): {skip_summary}",
            skipped.len()
        ));
    }
    Ok(msg)
}
