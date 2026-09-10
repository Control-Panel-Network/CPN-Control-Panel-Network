//! FTP stack detection (prefers CPN OpenSSH jailed SFTP).

use crate::panel_ops_sftp::detect_sftp_stack;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct FtpStatus {
    pub stack: String,
    pub detail: String,
    pub ready: bool,
}

pub fn detect_ftp() -> FtpStatus {
    let sftp = detect_sftp_stack();
    if sftp.ready {
        return FtpStatus {
            stack: sftp.stack,
            detail: sftp.detail,
            ready: true,
        };
    }
    let pure = Command::new("systemctl")
        .args(["is-active", "pure-ftpd"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if pure == "active" {
        return FtpStatus {
            stack: "Pure-FTPd".into(),
            detail: "Pure-FTPd is active. Prefer CPN jailed SFTP (Reset SFTP) for chrooted site access."
                .into(),
            ready: true,
        };
    }
    let vs = Command::new("systemctl")
        .args(["is-active", "sshd"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if vs == "active" || sftp.stack == "OpenSSH" {
        return FtpStatus {
            stack: sftp.stack,
            detail: sftp.detail,
            ready: false,
        };
    }
    FtpStatus {
        stack: sftp.stack,
        detail: sftp.detail,
        ready: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_status() {
        let s = detect_ftp();
        assert!(!s.stack.is_empty());
    }
}
