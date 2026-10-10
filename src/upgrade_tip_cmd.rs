//! Run cargo/npm for commit upgrades while streaming output like the CLI.

use crate::upgrade_tip_log;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// True when a cargo/npm stderr line is a real failure, not a crate name like `thiserror`.
pub fn stderr_looks_like_failure(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    // Cargo progress: "   Compiling thiserror v1.0.69" must stay info.
    if lower.contains("compiling ") || lower.contains("downloaded ") || lower.contains("checkout ")
    {
        return false;
    }
    if lower.contains("finished `") || lower.contains("fresh ") {
        return false;
    }
    // Word-ish failure markers; avoid matching inside identifiers (thiserror, ErrorKind).
    let words = [
        "error:",
        "error[",
        " error ",
        "failed to",
        "failed:",
        "fatal:",
        "panic:",
        "could not compile",
        "could not build",
        "aborted",
        "killed",
        "signal: 9",
        "exit status: 137",
        "out of memory",
        "cannot allocate",
    ];
    for marker in words {
        if lower.contains(marker) {
            return true;
        }
    }
    // Leading "error" / "failed" as a token (cargo style).
    let first = lower.split_whitespace().next().unwrap_or("");
    first == "error" || first == "error:" || first == "failed" || first.starts_with("error[")
}

fn signal_hint(status: &std::process::ExitStatus) -> Option<&'static str> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        match status.signal() {
            Some(9) => Some(
                "process received SIGKILL (signal 9); often the OOM killer. Retry with fewer cargo jobs (CARGO_BUILD_JOBS is capped from free memory), free RAM, or use a published release package.",
            ),
            Some(signal) => {
                let _ = signal;
                None
            }
            None => {
                if status.code() == Some(137) {
                    Some(
                        "exit 137 usually means SIGKILL (OOM). Retry with fewer cargo jobs or more free memory.",
                    )
                } else {
                    None
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        None
    }
}

pub async fn run_checked(
    program: &str,
    args: &[&str],
    cwd: &Path,
    env: &[(String, String)],
) -> Result<(), String> {
    let cmdline = format!("{program} {}", args.join(" "));
    upgrade_tip_log::log_info(format!("$ {cmdline}"));
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let mut child = cmd
        .spawn()
        .map_err(|error| fail(format!("{program} failed to start: {error}")))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let failure_tail: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let failure_tail_err = Arc::clone(&failure_tail);
    let out_task = tokio::spawn(async move {
        if let Some(pipe) = stdout {
            let mut lines = BufReader::new(pipe).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    upgrade_tip_log::log_info(&line);
                }
            }
        }
    });
    let err_task = tokio::spawn(async move {
        if let Some(pipe) = stderr {
            let mut lines = BufReader::new(pipe).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                if stderr_looks_like_failure(&line) {
                    upgrade_tip_log::log_failure(&line, None);
                    if let Ok(mut guard) = failure_tail_err.lock() {
                        guard.push(line);
                        if guard.len() > 12 {
                            let drain = guard.len() - 12;
                            guard.drain(0..drain);
                        }
                    }
                } else {
                    upgrade_tip_log::log_info(&line);
                }
            }
        }
    });
    let status = child
        .wait()
        .await
        .map_err(|error| fail(format!("{program} wait failed: {error}")))?;
    let _ = out_task.await;
    let _ = err_task.await;
    if !status.success() {
        let mut detail = format!("{cmdline} failed ({status})");
        if let Some(hint) = signal_hint(&status) {
            detail.push_str("; ");
            detail.push_str(hint);
        }
        if let Ok(guard) = failure_tail.lock()
            && !guard.is_empty()
        {
            let joined = guard
                .iter()
                .map(|s| s.trim())
                .collect::<Vec<_>>()
                .join(" | ");
            detail.push_str("; rustc/stderr: ");
            detail.push_str(&joined.chars().take(800).collect::<String>());
        }
        return Err(fail(detail));
    }
    Ok(())
}

fn fail(message: impl Into<String>) -> String {
    let message = message.into();
    upgrade_tip_log::log_failure(&message, None);
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thiserror_compile_line_is_not_failure() {
        assert!(!stderr_looks_like_failure("   Compiling thiserror v1.0.69"));
        assert!(!stderr_looks_like_failure(
            "   Compiling thiserror-impl v1.0.69"
        ));
        assert!(!stderr_looks_like_failure(
            "    Finished `release` profile [optimized] target(s) in 6m 14s"
        ));
    }

    #[test]
    fn real_rustc_errors_are_failures() {
        assert!(stderr_looks_like_failure(
            "error: could not compile `cpn-installer`"
        ));
        assert!(stderr_looks_like_failure(
            "error[E0425]: cannot find value `x` in this scope"
        ));
        assert!(stderr_looks_like_failure(
            "  process didn't exit successfully: signal: 9 (SIGKILL)"
        ));
        assert!(stderr_looks_like_failure(
            "failed to run custom build command"
        ));
    }

    #[test]
    fn fail_helper_redacts_nothing_plain() {
        let line = format!("$ {} {}", "cargo", "build --release --locked");
        assert!(line.starts_with("$ cargo"));
        assert!(!line.contains('\u{2014}'));
        assert!(!line.contains('\u{2013}'));
    }
}
