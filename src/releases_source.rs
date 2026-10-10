//! Configurable GitHub update source (official repo or operator fork) plus the
//! branch used for "Upgrade to latest commits".
//!
//! Config: `/var/lib/cpn/update-source.json` (mode 600)
//! Token:  `/var/lib/cpn/secrets/github-token` (mode 600; also env CPN_GITHUB_TOKEN / GITHUB_TOKEN)
//!
//! Default source remains Control-Panel-Network/CPN-Control-Panel-Network and
//! the default commit branch remains `stable`. `dev` is offered for lab and
//! pre-release testing only; production hosts should stay on `stable`.

use crate::paths;
use crate::releases::OFFICIAL_GITHUB_REPO;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Production branch: released tip, the only branch supported for production hosts.
pub const DEFAULT_UPDATE_BRANCH: &str = "stable";
/// Pre-release branch for lab testing.
pub const DEV_UPDATE_BRANCH: &str = "dev";

fn default_branch_string() -> String {
    DEFAULT_UPDATE_BRANCH.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSourceConfig {
    /// `owner/repo` used for upgrade/check. Empty or missing => official.
    #[serde(default)]
    pub repo: String,
    /// Branch followed by "Upgrade to latest commits". Missing => `stable`.
    #[serde(default = "default_branch_string")]
    pub branch: String,
    #[serde(default)]
    pub updated_at_unix: u64,
}

impl Default for UpdateSourceConfig {
    fn default() -> Self {
        Self {
            repo: OFFICIAL_GITHUB_REPO.to_string(),
            branch: default_branch_string(),
            updated_at_unix: 0,
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn update_source_path() -> PathBuf {
    paths::default_data_dir().join("update-source.json")
}

pub fn github_token_path() -> PathBuf {
    paths::default_data_dir()
        .join("secrets")
        .join("github-token")
}

/// Validate and normalize `owner/repo` (no URL, no .git suffix).
pub fn normalize_repo(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/').trim_end_matches(".git");
    let trimmed = trimmed
        .trim_start_matches("https://github.com/")
        .trim_start_matches("http://github.com/")
        .trim_start_matches("github.com/");
    let trimmed = trimmed.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Ok(OFFICIAL_GITHUB_REPO.to_string());
    }
    let parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() != 2 {
        return Err(
            "GitHub repo must look like owner/repo (example: Control-Panel-Network/CPN-Control-Panel-Network)."
                .into(),
        );
    }
    let owner = parts[0];
    let name = parts[1];
    let ok = |s: &str| {
        !s.is_empty()
            && s.len() <= 100
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    };
    if !ok(owner) || !ok(name) {
        return Err(
            "GitHub owner/repo may only use letters, digits, hyphen, underscore, and dot.".into(),
        );
    }
    Ok(format!("{owner}/{name}"))
}

/// Validate and normalize a git branch name. Empty => `stable`.
/// Accepts `refs/heads/<name>`; rejects path tricks and git-invalid refs.
pub fn normalize_branch(raw: &str) -> Result<String, String> {
    let trimmed = raw
        .trim()
        .trim_start_matches("refs/heads/")
        .trim_matches('/')
        .trim();
    if trimmed.is_empty() {
        return Ok(DEFAULT_UPDATE_BRANCH.to_string());
    }
    if trimmed.len() > 120 {
        return Err("Branch name is too long (max 120 characters).".into());
    }
    if trimmed.starts_with('-')
        || trimmed.contains("..")
        || trimmed.contains("//")
        || trimmed.contains("@{")
        || trimmed.ends_with(".lock")
        || trimmed.ends_with('.')
        || trimmed
            .split('/')
            .any(|seg| seg.is_empty() || seg.starts_with('.'))
    {
        return Err("Branch name is not a valid git branch.".into());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
    {
        return Err(
            "Branch name may only use letters, digits, hyphen, underscore, dot, and slash.".into(),
        );
    }
    Ok(trimmed.to_string())
}

/// True for the production branch (`stable`).
pub fn is_production_branch(branch: &str) -> bool {
    branch.trim().eq_ignore_ascii_case(DEFAULT_UPDATE_BRANCH)
}

/// Operator-facing label for a branch option.
pub fn branch_label(branch: &str) -> String {
    if is_production_branch(branch) {
        format!("{DEFAULT_UPDATE_BRANCH} (production: releases and stable commits)")
    } else if branch.eq_ignore_ascii_case(DEV_UPDATE_BRANCH) {
        format!("{DEV_UPDATE_BRANCH} (pre-release testing, lab only)")
    } else {
        format!("{branch} (other branch, lab only)")
    }
}

pub fn load_update_source() -> UpdateSourceConfig {
    let path = update_source_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return UpdateSourceConfig::default();
    };
    let mut cfg: UpdateSourceConfig = serde_json::from_str(&raw).unwrap_or_default();
    if let Ok(normalized) = normalize_repo(&cfg.repo) {
        cfg.repo = normalized;
    } else {
        cfg.repo = OFFICIAL_GITHUB_REPO.to_string();
    }
    if cfg.repo.is_empty() {
        cfg.repo = OFFICIAL_GITHUB_REPO.to_string();
    }
    cfg.branch = normalize_branch(&cfg.branch).unwrap_or_else(|_| default_branch_string());
    cfg
}

fn write_mode_600(path: &PathBuf, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.write_all(body.as_bytes())
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Save repo and branch together. `branch_raw` None keeps the stored branch.
pub fn save_update_source(
    repo_raw: &str,
    branch_raw: Option<&str>,
) -> Result<UpdateSourceConfig, String> {
    let repo = normalize_repo(repo_raw)?;
    let branch = match branch_raw {
        Some(raw) => normalize_branch(raw)?,
        None => load_update_source().branch,
    };
    let cfg = UpdateSourceConfig {
        repo,
        branch,
        updated_at_unix: now_unix(),
    };
    let json = serde_json::to_string_pretty(&cfg)
        .map_err(|e| format!("Could not serialize update source: {e}"))?;
    write_mode_600(&update_source_path(), &format!("{json}\n"))?;
    Ok(cfg)
}

/// Repo-only save (keeps the stored branch). Kept for existing callers.
pub fn save_update_source_repo(repo_raw: &str) -> Result<UpdateSourceConfig, String> {
    save_update_source(repo_raw, None)
}

pub fn configured_github_repo() -> String {
    if let Ok(env_repo) = std::env::var("CPN_GITHUB_REPO") {
        let trimmed = env_repo.trim();
        if !trimmed.is_empty() {
            return normalize_repo(trimmed).unwrap_or_else(|_| OFFICIAL_GITHUB_REPO.to_string());
        }
    }
    load_update_source().repo
}

/// Branch used for commit upgrades: env override (`CPN_UPDATE_BRANCH`, legacy
/// `CPN_STABLE_BRANCH`) first, then the saved update source, default `stable`.
pub fn configured_update_branch() -> String {
    for key in ["CPN_UPDATE_BRANCH", "CPN_STABLE_BRANCH"] {
        if let Ok(value) = std::env::var(key)
            && let Ok(branch) = normalize_branch(&value)
            && !value.trim().is_empty()
        {
            return branch;
        }
    }
    load_update_source().branch
}

pub fn github_token_configured() -> bool {
    crate::releases_cache::github_token().is_some()
}

/// Empty token leaves the existing secret. `clear` true removes the file.
pub fn save_github_token(token: &str, clear: bool) -> Result<(), String> {
    let path = github_token_path();
    if clear {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.len() > 256 {
        return Err("GitHub token is too long.".into());
    }
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return Err("GitHub token must not contain whitespace.".into());
    }
    write_mode_600(&path, &format!("{trimmed}\n"))
}

pub fn is_official_repo(repo: &str) -> bool {
    normalize_repo(repo)
        .map(|r| r.eq_ignore_ascii_case(OFFICIAL_GITHUB_REPO))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_urls_and_defaults() {
        assert_eq!(normalize_repo("").unwrap(), OFFICIAL_GITHUB_REPO);
        assert_eq!(
            normalize_repo("https://github.com/Acme/CPN-Fork.git").unwrap(),
            "Acme/CPN-Fork"
        );
        assert!(normalize_repo("not a repo").is_err());
        assert!(normalize_repo("../evil/path").is_err());
        assert!(!normalize_repo("Acme/Fork").unwrap().contains('\u{2014}'));
        assert!(!is_official_repo("Acme/Fork"));
        assert!(is_official_repo(OFFICIAL_GITHUB_REPO));
    }

    #[test]
    fn normalizes_branches_and_rejects_bad_refs() {
        assert_eq!(normalize_branch("").unwrap(), "stable");
        assert_eq!(normalize_branch("  dev ").unwrap(), "dev");
        assert_eq!(normalize_branch("refs/heads/dev").unwrap(), "dev");
        assert_eq!(normalize_branch("feat/x-1.2_y").unwrap(), "feat/x-1.2_y");
        for bad in [
            "-rf",
            "a..b",
            "a//b",
            "x@{1}",
            "main.lock",
            "ends.",
            "/.hidden",
            "a/.b",
            "has space",
            "semi;colon",
            "tick`cmd`",
        ] {
            assert!(normalize_branch(bad).is_err(), "{bad} should be rejected");
        }
        let long = "a".repeat(121);
        assert!(normalize_branch(&long).is_err());
    }

    #[test]
    fn branch_defaults_and_labels() {
        let cfg: UpdateSourceConfig =
            serde_json::from_str(r#"{"repo":"Acme/Fork","updated_at_unix":1}"#).unwrap();
        assert_eq!(cfg.branch, "stable", "missing branch defaults to stable");
        let cfg: UpdateSourceConfig =
            serde_json::from_str(r#"{"repo":"","branch":"dev"}"#).unwrap();
        assert_eq!(cfg.branch, "dev");
        assert_eq!(UpdateSourceConfig::default().branch, "stable");
        assert!(is_production_branch("stable"));
        assert!(!is_production_branch("dev"));
        assert!(branch_label("stable").contains("production"));
        assert!(branch_label("dev").contains("pre-release"));
        assert!(branch_label("feat/x").contains("lab only"));
        for label in [
            branch_label("stable"),
            branch_label("dev"),
            branch_label("z"),
        ] {
            assert!(!label.contains('\u{2014}'));
            assert!(!label.contains('\u{2013}'));
        }
    }
}
