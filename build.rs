//! Embed the git commit SHA used to build this panel binary.

fn main() {
    let sha = std::env::var("CPN_GIT_SHA")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(git_head_sha)
        .unwrap_or_default();
    println!("cargo:rustc-env=CPN_GIT_SHA={sha}");
    println!("cargo:rerun-if-env-changed=CPN_GIT_SHA");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/stable");
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
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}
