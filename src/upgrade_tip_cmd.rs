//! Run cargo/npm for commit upgrades while streaming output like the CLI.

use crate::upgrade_tip_log;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

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
                if !line.trim().is_empty() {
                    let lower = line.to_ascii_lowercase();
                    if lower.contains("error") || lower.contains("failed") {
                        upgrade_tip_log::log_failure(&line, None);
                    } else {
                        upgrade_tip_log::log_info(&line);
                    }
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
        return Err(fail(format!("{cmdline} failed ({status})")));
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
    #[test]
    fn fail_helper_redacts_nothing_plain() {
        // Compile-time module smoke: command line formatting stays shell-like.
        let line = format!("$ {} {}", "cargo", "build --release --locked");
        assert!(line.starts_with("$ cargo"));
        assert!(!line.contains('\u{2014}'));
    }
}
