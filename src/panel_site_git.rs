//! Allowlisted git operations scoped to a site home / docroot.

use crate::sites::{SiteRecord, site_home_from_record};
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_OUTPUT: usize = 64 * 1024;
const MAX_COMMIT_MSG: usize = 200;
const GIT_TIMEOUT_HINT: &str = "git command failed or timed out";

fn truncate_out(raw: Vec<u8>) -> String {
    let lossy = String::from_utf8_lossy(&raw);
    let mut s = lossy.into_owned();
    if s.len() > MAX_OUTPUT {
        s.truncate(MAX_OUTPUT);
        s.push_str("\n…(truncated)");
    }
    s
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    if !cwd.is_dir() {
        return Err(format!("Working directory missing: {}", cwd.display()));
    }
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    let mut text = String::new();
    if !output.stdout.is_empty() {
        text.push_str(&truncate_out(output.stdout));
    }
    if !output.stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&truncate_out(output.stderr));
    }
    if !output.status.success() {
        if text.trim().is_empty() {
            return Err(GIT_TIMEOUT_HINT.into());
        }
        return Err(text);
    }
    if text.trim().is_empty() {
        Ok("(ok, no output)".into())
    } else {
        Ok(text)
    }
}

/// Prefer an existing `.git` under docroot, else site home, else docroot for init/clone.
pub fn git_workdir(site: &SiteRecord) -> PathBuf {
    let doc = PathBuf::from(&site.docroot);
    if doc.join(".git").exists() {
        return doc;
    }
    let home = site_home_from_record(site);
    if home.join(".git").exists() {
        return home;
    }
    if doc.is_dir() { doc } else { home }
}

fn sanitize_commit_message(raw: &str) -> Result<String, String> {
    let msg = raw.trim();
    if msg.is_empty() {
        return Err("Commit message is required".into());
    }
    if msg.len() > MAX_COMMIT_MSG {
        return Err(format!("Commit message max {MAX_COMMIT_MSG} characters"));
    }
    if msg
        .chars()
        .any(|c| c == '\n' || c == '\r' || c.is_control())
    {
        return Err("Commit message cannot include control characters".into());
    }
    Ok(msg.to_string())
}

/// Allow only https remotes with a conservative character set (no shell metacharacters).
pub fn validate_remote_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err("Remote URL is required".into());
    }
    if url.len() > 512 {
        return Err("Remote URL is too long".into());
    }
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("git@")) {
        return Err("Remote URL must start with https:// or git@".into());
    }
    if lower.starts_with("https://") {
        let rest = &url["https://".len()..];
        if rest.is_empty()
            || !rest.chars().all(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_' | ':' | '@')
            })
        {
            return Err("Remote URL contains unsupported characters".into());
        }
    } else {
        // git@host:path/repo.git
        if !url
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_' | ':' | '@'))
        {
            return Err("Remote URL contains unsupported characters".into());
        }
        if !url.contains(':') {
            return Err("git@ URL must use host:path form".into());
        }
    }
    Ok(url.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitAction {
    Status,
    Remotes,
    Branches,
    Pull,
    Push,
    Init,
    Commit,
    Clone,
}

impl GitAction {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "status" => Ok(Self::Status),
            "remotes" | "remote" => Ok(Self::Remotes),
            "branches" | "branch" => Ok(Self::Branches),
            "pull" => Ok(Self::Pull),
            "push" => Ok(Self::Push),
            "init" => Ok(Self::Init),
            "commit" => Ok(Self::Commit),
            "clone" => Ok(Self::Clone),
            _ => Err("Unknown git action".into()),
        }
    }
}

pub fn run_git_action(
    site: &SiteRecord,
    action: GitAction,
    commit_message: Option<&str>,
    remote_url: Option<&str>,
) -> Result<String, String> {
    let cwd = git_workdir(site);
    match action {
        GitAction::Status => run_git(&cwd, &["status", "--porcelain=v1", "-b"]),
        GitAction::Remotes => run_git(&cwd, &["remote", "-v"]),
        GitAction::Branches => run_git(&cwd, &["branch", "-vv"]),
        GitAction::Pull => run_git(&cwd, &["pull", "--ff-only"]),
        GitAction::Push => run_git(&cwd, &["push"]),
        GitAction::Init => {
            if cwd.join(".git").exists() {
                return Err("Repository already initialized".into());
            }
            run_git(&cwd, &["init"])
        }
        GitAction::Commit => {
            let msg = sanitize_commit_message(commit_message.unwrap_or(""))?;
            let _ = run_git(&cwd, &["add", "-A"])?;
            run_git(&cwd, &["commit", "-m", &msg])
        }
        GitAction::Clone => {
            let url = validate_remote_url(remote_url.unwrap_or(""))?;
            if cwd.join(".git").exists() {
                return Err("Cannot clone into a directory that already has .git".into());
            }
            // Clone into current workdir (must be mostly empty of .git).
            run_git(&cwd, &["clone", "--depth", "1", &url, "."])
        }
    }
}

pub fn snapshot(site: &SiteRecord) -> String {
    let cwd = git_workdir(site);
    let status = run_git(&cwd, &["status", "--porcelain=v1", "-b"])
        .unwrap_or_else(|e| format!("(status unavailable)\n{e}"));
    let remotes = run_git(&cwd, &["remote", "-v"]).unwrap_or_else(|e| format!("(no remotes)\n{e}"));
    let branches =
        run_git(&cwd, &["branch", "-vv"]).unwrap_or_else(|e| format!("(no branches)\n{e}"));
    format!(
        "Workdir: {}\n\n=== status ===\n{}\n\n=== remotes ===\n{}\n\n=== branches ===\n{}",
        cwd.display(),
        status.trim(),
        remotes.trim(),
        branches.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_url_https_ok() {
        assert!(validate_remote_url("https://github.com/org/repo.git").is_ok());
        assert!(validate_remote_url("https://evil.com/repo;rm -rf /").is_err());
        assert!(validate_remote_url("http://insecure.example/repo").is_err());
    }

    #[test]
    fn commit_message_rejects_control() {
        assert!(sanitize_commit_message("ok message").is_ok());
        assert!(sanitize_commit_message("bad\nline").is_err());
        assert!(sanitize_commit_message("").is_err());
    }

    #[test]
    fn action_parse() {
        assert_eq!(GitAction::parse("pull").unwrap(), GitAction::Pull);
        assert!(GitAction::parse("rebase").is_err());
    }
}
