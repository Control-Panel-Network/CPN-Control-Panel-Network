//! Embed the git commit SHA used to build this panel binary.

fn main() {
    let sha = first_sha(&[
        env_sha("CPN_BUILD_SHA"),
        env_sha("CPN_GIT_SHA"),
        file_sha(),
        git_head_sha(),
    ])
    .unwrap_or_default();
    println!("cargo:rustc-env=CPN_GIT_SHA={sha}");
    println!("cargo:rustc-env=CPN_BUILD_SHA={sha}");
    println!("cargo:rerun-if-env-changed=CPN_GIT_SHA");
    println!("cargo:rerun-if-env-changed=CPN_BUILD_SHA");
    println!("cargo:rerun-if-changed=.cpn-git-sha");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/stable");
}

fn env_sha(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn first_sha(candidates: &[Option<String>]) -> Option<String> {
    candidates.iter().cloned().find_map(|item| item)
}

fn file_sha() -> Option<String> {
    let text = std::fs::read_to_string(".cpn-git-sha").ok()?;
    let sha = text.trim().to_string();
    if sha.is_empty() { None } else { Some(sha) }
}

fn git_head_sha() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() { None } else { Some(sha) }
}
