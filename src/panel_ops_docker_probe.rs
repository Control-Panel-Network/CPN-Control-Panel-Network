//! Bounded container-engine probes so a stuck `podman` / `docker` cannot hang panel pages.
//!
//! `/docker`, the Host package status and upgrade verification all shell out to the engine CLI.
//! A wedged podman (broken rootless runtime, stale socket, hung storage lock) never returns, so
//! every probe here runs with a hard deadline, kills the whole process group on expiry and reports
//! a clear timeout instead of blocking the request thread for minutes.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// `docker --version` / `podman --version` (local, should be instant).
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(3);
/// `docker info` / `podman info` (talks to the engine; slow only when wedged).
pub const INFO_TIMEOUT: Duration = Duration::from_secs(5);
/// `docker compose version` style capability probes.
pub const COMPOSE_TIMEOUT: Duration = Duration::from_secs(5);
/// `ps`, `images`, `inspect` listings rendered on `/docker`.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(15);

const POLL_INTERVAL: Duration = Duration::from_millis(40);
const PIPE_DRAIN_GRACE: Duration = Duration::from_secs(2);

fn spawn_reader<R: Read + Send + 'static>(mut pipe: R) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    rx
}

fn kill_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The child leads its own process group (see `output_with_timeout`), so a negative pid
        // takes down conmon / runtime helpers that podman may have spawned as well.
        let pid = child.id() as libc::pid_t;
        if pid > 0 {
            // SAFETY: plain signal delivery to a process group we created.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Run a pre-built `Command` and collect its output, failing with `TimedOut` after `timeout`.
pub fn command_output_with_timeout(
    mut cmd: Command,
    timeout: Duration,
    label: &str,
) -> std::io::Result<Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own process group so hung helpers can be killed with the tree.
        let _ = cmd.process_group(0);
    }
    let mut child = cmd.spawn()?;
    let out_rx = child.stdout.take().map(spawn_reader);
    let err_rx = child.stderr.take().map(spawn_reader);
    let start = Instant::now();
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None => {
                if start.elapsed() >= timeout {
                    kill_tree(&mut child);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("`{label}` timed out after {}s", timeout.as_secs()),
                    ));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
        }
    };
    let collect = |rx: Option<mpsc::Receiver<Vec<u8>>>| {
        rx.and_then(|r| r.recv_timeout(PIPE_DRAIN_GRACE).ok())
            .unwrap_or_default()
    };
    Ok(Output {
        status,
        stdout: collect(out_rx),
        stderr: collect(err_rx),
    })
}

/// Run `bin args...` and collect its output, failing with `TimedOut` after `timeout`.
pub fn output_with_timeout(bin: &str, args: &[&str], timeout: Duration) -> std::io::Result<Output> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    command_output_with_timeout(cmd, timeout, &format!("{bin} {}", args.join(" ")))
}

/// True when the command exits 0 within `timeout`.
pub fn probe_ok(bin: &str, args: &[&str], timeout: Duration) -> bool {
    output_with_timeout(bin, args, timeout)
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn hung_command_is_killed_at_deadline() {
        let start = Instant::now();
        let err = output_with_timeout("sleep", &["300"], Duration::from_millis(300)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(60));
    }

    #[cfg(unix)]
    #[test]
    fn grandchild_holding_pipe_does_not_block_probe() {
        let start = Instant::now();
        let res = output_with_timeout(
            "sh",
            &["-c", "sleep 300 & sleep 300"],
            Duration::from_millis(300),
        );
        assert!(res.is_err());
        assert!(start.elapsed() < Duration::from_secs(60));
    }

    #[cfg(unix)]
    #[test]
    fn fast_command_returns_output() {
        let out = output_with_timeout("sh", &["-c", "echo ok"], Duration::from_secs(5)).unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
        assert!(probe_ok("true", &[], Duration::from_secs(5)));
        assert!(!probe_ok("false", &[], Duration::from_secs(5)));
    }

    #[test]
    fn missing_binary_is_not_ok() {
        assert!(!probe_ok(
            "cpn-definitely-not-a-binary",
            &["--version"],
            Duration::from_secs(1)
        ));
    }
}
