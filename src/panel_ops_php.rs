//! PHP runtime detection for Server hub (read-only first).

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PhpRuntimeInfo {
    pub binary: Option<String>,
    pub version: Option<String>,
    pub ini_path: Option<String>,
    pub modules: Vec<String>,
    pub detail: String,
}

fn which_php() -> Option<String> {
    for candidate in [
        "php", "php82", "php83", "php84", "php8.2", "php8.3", "php8.4",
    ] {
        if let Ok(out) = Command::new(candidate).arg("-v").output()
            && out.status.success()
        {
            return Some(candidate.to_string());
        }
    }
    None
}

pub fn detect_php() -> PhpRuntimeInfo {
    let Some(bin) = which_php() else {
        return PhpRuntimeInfo {
            binary: None,
            version: None,
            ini_path: None,
            modules: vec![],
            detail: "PHP CLI not found on PATH".into(),
        };
    };
    let version = Command::new(&bin).arg("-v").output().ok().and_then(|o| {
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .next()
            .map(|s| s.trim().to_string())
    });
    let ini_path = Command::new(&bin).args(["-i"]).output().ok().and_then(|o| {
        let text = String::from_utf8_lossy(&o.stdout);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("Loaded Configuration File =>") {
                let p = rest.trim();
                if !p.is_empty() && p != "(none)" {
                    return Some(p.to_string());
                }
            }
        }
        None
    });
    let modules = Command::new(&bin)
        .args(["-m"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('['))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    PhpRuntimeInfo {
        binary: Some(bin),
        version,
        ini_path,
        modules,
        detail: "Read-only detection. Writes to php.ini require an explicit backup step.".into(),
    }
}

pub fn read_php_ini_preview(max_bytes: usize) -> Result<(PathBuf, String), String> {
    let info = detect_php();
    let Some(path) = info.ini_path else {
        return Err("No loaded php.ini path detected".into());
    };
    let p = PathBuf::from(&path);
    let raw = std::fs::read(&p).map_err(|e| format!("Cannot read {}: {e}", p.display()))?;
    let take = raw.len().min(max_bytes);
    let text = String::from_utf8_lossy(&raw[..take]).to_string();
    Ok((p, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic() {
        let info = detect_php();
        assert!(!info.detail.is_empty());
    }
}
