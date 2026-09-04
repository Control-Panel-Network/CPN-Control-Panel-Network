//! FTP stack detection and account stubs.

use std::process::Command;

#[derive(Debug, Clone)]
pub struct FtpStatus {
    pub stack: String,
    pub detail: String,
    pub ready: bool,
}

pub fn detect_ftp() -> FtpStatus {
    let pure = Command::new("systemctl")
        .args(["is-active", "pure-ftpd"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if pure == "active" {
        return FtpStatus {
            stack: "Pure-FTPd".into(),
            detail: "Pure-FTPd is active. Account CRUD wiring is next; use system tools for now."
                .into(),
            ready: true,
        };
    }
    let vs = Command::new("systemctl")
        .args(["is-active", "vsftpd"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if vs == "active" {
        return FtpStatus {
            stack: "vsftpd".into(),
            detail: "vsftpd is active. Account CRUD wiring is next; use system tools for now."
                .into(),
            ready: true,
        };
    }
    FtpStatus {
        stack: "Not detected".into(),
        detail: "No Pure-FTPd or vsftpd service detected. Install an FTP stack to enable accounts."
            .into(),
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
