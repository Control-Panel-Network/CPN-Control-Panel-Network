//! Monthly bandwidth metering from per-site access logs.
//!
//! Each site keeps a small ledger at `$CPN_DATA_DIR/bandwidth/<domain>.json` with the
//! calendar-month byte total plus the read offset into the access log. Refreshing only
//! reads the bytes appended since the last refresh, so the total survives log rotation
//! and large logs are never rescanned. Account usage is the sum over the sites it owns.

use crate::account::{data_dir, now_unix};
use crate::panel_website_bandwidth::parse_response_bytes;
use crate::panel_website_logs::{candidate_log_paths, first_existing_log};
use crate::sites::{SiteRecord, list_sites};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// Most bytes read from one log per refresh (keeps page loads bounded).
const MAX_CHUNK_BYTES: u64 = 32 * 1024 * 1024;
/// On the first scan of a log, only the trailing window is read.
const MAX_FIRST_SCAN_BYTES: u64 = 64 * 1024 * 1024;
const MB: u64 = 1024 * 1024;

static LEDGER_LOCK: Mutex<()> = Mutex::new(());

/// Calendar month being metered: ledger key plus the access-log date marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterPeriod {
    /// `YYYY-MM`.
    pub key: String,
    /// Access-log marker such as `/Oct/2026:`.
    pub token: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Ledger {
    #[serde(default)]
    domain: String,
    #[serde(default)]
    period: String,
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    log_path: String,
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    inode: u64,
    #[serde(default)]
    updated_at_unix: u64,
}

/// Civil `(year, month 1..=12)` for a unix timestamp (UTC).
fn civil_year_month(unix: u64) -> (i64, u32) {
    let z = (unix / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month)
}

pub fn period_from_year_month(year: i64, month: u32) -> MeterPeriod {
    let month = month.clamp(1, 12);
    MeterPeriod {
        key: format!("{year:04}-{month:02}"),
        token: format!("/{}/{year:04}:", MONTHS[(month - 1) as usize]),
    }
}

pub fn period_from_unix_utc(unix: u64) -> MeterPeriod {
    let (year, month) = civil_year_month(unix);
    period_from_year_month(year, month)
}

#[cfg(unix)]
fn local_year_month() -> Option<(i64, u32)> {
    let out = Command::new("date").arg("+%Y %m").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut parts = text.split_whitespace();
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    ((1..=12).contains(&month) && year > 2000).then_some((year, month))
}

#[cfg(not(unix))]
fn local_year_month() -> Option<(i64, u32)> {
    None
}

/// Current metering month (server local time when `date` works, else UTC).
pub fn current_period() -> MeterPeriod {
    match local_year_month() {
        Some((year, month)) => period_from_year_month(year, month),
        None => period_from_unix_utc(now_unix()),
    }
}

fn ledger_path(domain: &str) -> Option<PathBuf> {
    let safe: String = domain
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        .collect();
    if safe.is_empty() || safe.starts_with('.') || safe.contains("..") {
        return None;
    }
    Some(data_dir().join("bandwidth").join(format!("{safe}.json")))
}

fn load_ledger(domain: &str) -> Ledger {
    ledger_path(domain)
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str::<Ledger>(&raw).ok())
        .unwrap_or_default()
}

fn save_ledger(ledger: &Ledger) -> Result<(), String> {
    let Some(path) = ledger_path(&ledger.domain) else {
        return Err("Invalid domain for bandwidth ledger".into());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(ledger)
        .map_err(|e| format!("Could not serialize bandwidth ledger: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Could not save {}: {e}", tmp.display()))?;
    drop(file);
    fs::rename(&tmp, &path).map_err(|e| format!("Could not replace {}: {e}", path.display()))
}

#[cfg(unix)]
fn inode_of(meta: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.ino()
}

#[cfg(not(unix))]
fn inode_of(_meta: &fs::Metadata) -> u64 {
    0
}

/// Add up the bytes of lines that belong to `token`'s month in `text`.
pub fn sum_month_bytes(text: &str, token: &str) -> u64 {
    let mut total = 0u64;
    for line in text.lines() {
        if !line.contains(token) {
            continue;
        }
        if let Some(bytes) = parse_response_bytes(line) {
            total = total.saturating_add(bytes);
        }
    }
    total
}

/// Advance `ledger` over any new complete lines in `path`. Returns the updated ledger.
fn scan_log(path: &Path, period: &MeterPeriod, mut ledger: Ledger) -> Result<Ledger, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.file_type().is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    let len = meta.len();
    let inode = inode_of(&meta);
    let path_text = path.to_string_lossy().into_owned();

    if ledger.period != period.key {
        ledger.bytes = 0;
        ledger.period = period.key.clone();
    }
    let known = !ledger.log_path.is_empty();
    if known && ledger.log_path != path_text {
        // A different log file is now authoritative; do not mix totals.
        ledger.bytes = 0;
        ledger.offset = 0;
    } else if known && (len < ledger.offset || (ledger.inode != 0 && ledger.inode != inode)) {
        // Rotated or truncated: keep this month's total, restart at the top of the new file.
        ledger.offset = 0;
    }
    let first_scan = !known;
    if first_scan {
        ledger.offset = len.saturating_sub(MAX_FIRST_SCAN_BYTES);
    }
    ledger.log_path = path_text;
    ledger.inode = inode;

    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    file.seek(SeekFrom::Start(ledger.offset))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let want = (len - ledger.offset.min(len)).min(MAX_CHUNK_BYTES);
    let mut buf = Vec::with_capacity(want as usize);
    file.take(want)
        .read_to_end(&mut buf)
        .map_err(|e| format!("{}: {e}", path.display()))?;

    let mut start = 0usize;
    if first_scan && ledger.offset > 0 {
        // Starting mid-file: drop the partial first line.
        start = buf
            .iter()
            .position(|b| *b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(buf.len());
    }
    let complete_end = buf
        .iter()
        .rposition(|b| *b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(if (buf.len() as u64) >= MAX_CHUNK_BYTES {
            buf.len()
        } else {
            start
        });
    let end = complete_end.max(start);
    let text = String::from_utf8_lossy(&buf[start..end]);
    ledger.bytes = ledger
        .bytes
        .saturating_add(sum_month_bytes(&text, &period.token));
    ledger.offset = ledger.offset.saturating_add(end as u64);
    ledger.updated_at_unix = now_unix();
    Ok(ledger)
}

/// Bytes transferred this month by one site (refreshes the ledger from the access log).
pub fn site_month_bytes(site: &SiteRecord, period: &MeterPeriod) -> u64 {
    let _guard = LEDGER_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut ledger = load_ledger(&site.domain);
    ledger.domain = site.domain.clone();
    let (access, _) = candidate_log_paths(site);
    let Some(path) = first_existing_log(site, &access) else {
        return if ledger.period == period.key {
            ledger.bytes
        } else {
            0
        };
    };
    match scan_log(&path, period, ledger.clone()) {
        Ok(updated) => {
            if let Err(err) = save_ledger(&updated) {
                eprintln!("bandwidth: {err}");
            }
            updated.bytes
        }
        Err(err) => {
            eprintln!("bandwidth: {err}");
            if ledger.period == period.key {
                ledger.bytes
            } else {
                0
            }
        }
    }
}

/// Bytes transferred this month across every site owned by `username`.
pub fn account_month_bytes(username: &str) -> u64 {
    let Ok(sites) = list_sites() else {
        return 0;
    };
    let period = current_period();
    sites
        .iter()
        .filter(|s| s.owner.trim().eq_ignore_ascii_case(username.trim()))
        .fold(0u64, |acc, site| {
            acc.saturating_add(site_month_bytes(site, &period))
        })
}

/// Whole megabytes (rounded up) for quota comparison with package MB limits.
pub fn bytes_to_mb_ceil(bytes: u64) -> u64 {
    bytes.div_ceil(MB)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_log(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cpn-bw-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("access.log")
    }

    fn line(day: &str, bytes: u64) -> String {
        format!("1.1.1.1 - - [{day}:10:00:00 +0000] \"GET / HTTP/1.1\" 200 {bytes}\n")
    }

    #[test]
    fn period_from_unix_handles_month_edges() {
        // 2026-10-01 00:00:00 UTC
        let p = period_from_unix_utc(1_790_812_800);
        assert_eq!(p.key, "2026-10");
        assert_eq!(p.token, "/Oct/2026:");
        // one second earlier is still September
        assert_eq!(period_from_unix_utc(1_790_812_799).key, "2026-09");
        // leap day
        assert_eq!(period_from_unix_utc(1_709_164_800).key, "2024-02");
    }

    #[test]
    fn counts_only_current_month() {
        let text = format!(
            "{}{}{}",
            line("30/Sep/2026", 900),
            line("01/Oct/2026", 100),
            line("02/Oct/2026", 50)
        );
        assert_eq!(sum_month_bytes(&text, "/Oct/2026:"), 150);
    }

    #[test]
    fn incremental_scan_survives_rotation_and_partial_lines() {
        let path = temp_log("inc");
        let period = period_from_year_month(2026, 10);
        fs::write(
            &path,
            format!("{}{}", line("01/Oct/2026", 100), line("30/Sep/2026", 7)),
        )
        .unwrap();
        let l1 = scan_log(&path, &period, Ledger::default()).unwrap();
        assert_eq!(l1.bytes, 100);

        // Append one full line and one partial line without newline.
        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(line("02/Oct/2026", 40).as_bytes()).unwrap();
        f.write_all(b"1.1.1.1 - - [03/Oct/2026:10:00:00 +0000] \"GET / HTTP/1.1\" 200 5")
            .unwrap();
        drop(f);
        let l2 = scan_log(&path, &period, l1).unwrap();
        assert_eq!(l2.bytes, 140);

        // Finish the partial line; it is counted exactly once.
        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"0\n").unwrap();
        drop(f);
        let l3 = scan_log(&path, &period, l2).unwrap();
        assert_eq!(l3.bytes, 190);

        // Truncate (rotation): total kept, new content added.
        fs::write(&path, line("04/Oct/2026", 10)).unwrap();
        let l4 = scan_log(&path, &period, l3).unwrap();
        assert_eq!(l4.bytes, 200);

        // New month resets the total.
        let nov = period_from_year_month(2026, 11);
        let l5 = scan_log(&path, &nov, l4).unwrap();
        assert_eq!(l5.bytes, 0);
    }

    #[test]
    fn mb_rounds_up() {
        assert_eq!(bytes_to_mb_ceil(0), 0);
        assert_eq!(bytes_to_mb_ceil(1), 1);
        assert_eq!(bytes_to_mb_ceil(MB), 1);
        assert_eq!(bytes_to_mb_ceil(MB + 1), 2);
    }
}
