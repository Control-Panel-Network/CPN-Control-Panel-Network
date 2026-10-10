//! List branches of the configured update repo for the Version Management
//! Branch picker. `stable` and `dev` are always offered; other branches from
//! the GitHub Branches API are appended with a "lab only" label.

use crate::releases_source::{
    DEFAULT_UPDATE_BRANCH, DEV_UPDATE_BRANCH, branch_label, is_production_branch, normalize_branch,
};
use crate::releases_stable_tip::curl_github_json;
use serde::{Deserialize, Serialize};

/// Hard cap on branches returned to the UI (GitHub page size max is 100).
const MAX_BRANCHES: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchOption {
    pub name: String,
    pub label: String,
    /// `stable` only.
    pub production: bool,
    /// `stable` or `dev` (always offered even when the API call fails).
    pub known: bool,
    /// Present in the configured repo per the GitHub API (false when unknown).
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchListing {
    pub repo: String,
    pub configured: String,
    pub default_branch: String,
    pub branches: Vec<BranchOption>,
    /// Non-fatal note when the GitHub call failed (known branches still listed).
    pub error: Option<String>,
}

/// Fetch branch names from GitHub (`/repos/{repo}/branches?per_page=100`).
pub async fn list_repo_branches(repo: &str) -> Result<Vec<String>, String> {
    let url = format!("https://api.github.com/repos/{repo}/branches?per_page={MAX_BRANCHES}");
    let (status, body) = curl_github_json(&url).await?;
    if status != 200 {
        return Err(format!(
            "GitHub branches API returned HTTP {status} for {repo}"
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse GitHub branches JSON: {error}"))?;
    let Some(items) = value.as_array() else {
        return Err("GitHub branches response was not a list".into());
    };
    let mut names: Vec<String> = items
        .iter()
        .filter_map(|item| item.get("name").and_then(|n| n.as_str()))
        .filter_map(|name| normalize_branch(name).ok())
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

/// Build the picker options: `stable`, `dev`, the configured branch (if other),
/// then remaining fetched branches alphabetically.
pub fn branch_options(fetched: &[String], configured: &str) -> Vec<BranchOption> {
    let configured = normalize_branch(configured).unwrap_or_else(|_| DEFAULT_UPDATE_BRANCH.into());
    let exists = |name: &str| fetched.iter().any(|f| f == name);
    let mut out: Vec<BranchOption> = Vec::new();
    let push = |name: &str, known: bool, out: &mut Vec<BranchOption>| {
        if out.iter().any(|o| o.name == name) {
            return;
        }
        out.push(BranchOption {
            name: name.to_string(),
            label: branch_label(name),
            production: is_production_branch(name),
            known,
            exists: exists(name),
        });
    };
    push(DEFAULT_UPDATE_BRANCH, true, &mut out);
    push(DEV_UPDATE_BRANCH, true, &mut out);
    push(&configured, false, &mut out);
    for name in fetched.iter().take(MAX_BRANCHES) {
        push(name, false, &mut out);
    }
    out
}

/// Listing for the configured repo. Never fails: on GitHub errors the known
/// branches are returned with `error` set.
pub async fn branch_listing(repo: &str, configured: &str) -> BranchListing {
    let (fetched, error) = match list_repo_branches(repo).await {
        Ok(names) => (names, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    BranchListing {
        repo: repo.to_string(),
        configured: normalize_branch(configured).unwrap_or_else(|_| DEFAULT_UPDATE_BRANCH.into()),
        default_branch: DEFAULT_UPDATE_BRANCH.to_string(),
        branches: branch_options(&fetched, configured),
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_always_offer_stable_and_dev_first() {
        let opts = branch_options(&[], "stable");
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].name, "stable");
        assert!(opts[0].production && opts[0].known && !opts[0].exists);
        assert_eq!(opts[1].name, "dev");
        assert!(!opts[1].production && opts[1].known);
    }

    #[test]
    fn options_merge_fetched_without_duplicates() {
        let fetched = vec![
            "dev".to_string(),
            "feat/x".to_string(),
            "stable".to_string(),
            "zeta".to_string(),
        ];
        let opts = branch_options(&fetched, "feat/x");
        let names: Vec<&str> = opts.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, vec!["stable", "dev", "feat/x", "zeta"]);
        assert!(opts.iter().all(|o| o.exists));
        assert!(opts[2].label.contains("lab only"));
        assert!(!opts[2].known);
    }

    #[test]
    fn invalid_configured_falls_back_to_stable() {
        let opts = branch_options(&[], "a..b");
        assert_eq!(opts.len(), 2);
        for o in &opts {
            assert!(!o.label.contains('\u{2014}'));
            assert!(!o.label.contains('\u{2013}'));
        }
    }
}
