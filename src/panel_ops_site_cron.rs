//! Per-site cron editor: registry JSON + sync to `/etc/cron.d`.

use crate::account::now_unix;
use crate::panel_session::session_secret;
use crate::sites::{SiteRecord, load_site, normalize_domain, site_home_from_record};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fs;
use std::path::{Path, PathBuf};

type HmacSha256 = Hmac<Sha256>;

const SCHEMA_VERSION: u32 = 1;
const ALLOWED_INTERPRETERS: &[&str] = &[
    "php",
    "php8.2",
    "php8.3",
    "php8.4",
    "php8.5",
    "/usr/bin/php",
    "/usr/bin/php82",
    "/usr/bin/php83",
    "/usr/bin/php84",
    "/usr/bin/php85",
    "/bin/true",
    "/usr/bin/true",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteCronJob {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub minute: String,
    pub hour: String,
    pub day: String,
    pub month: String,
    pub weekday: String,
    pub command: String,
    #[serde(default)]
    pub comment: String,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SiteCronFile {
    #[serde(default)]
    schema_version: u32,
    domain: String,
    #[serde(default)]
    jobs: Vec<SiteCronJob>,
}

fn hmac_hex(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn verify_hmac_hex(expected: &str, provided: &str) -> bool {
    let a = expected.as_bytes();
    let b = provided.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in a.iter().zip(b.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

pub fn cron_csrf_token(username: &str) -> String {
    let secret = session_secret(None);
    let hour = now_unix() / 3600;
    let payload = format!("site-cron|{username}|{hour}");
    format!("{hour}.{}", hmac_hex(&secret, &payload))
}

pub fn verify_cron_csrf(username: &str, token: &str) -> bool {
    let secret = session_secret(None);
    let Some((hour_s, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(hour) = hour_s.parse::<u64>() else {
        return false;
    };
    let current = now_unix() / 3600;
    if hour + 2 < current || hour > current + 1 {
        return false;
    }
    let payload = format!("site-cron|{username}|{hour}");
    verify_hmac_hex(&hmac_hex(&secret, &payload), sig)
}

fn cron_store_path(domain: &str) -> PathBuf {
    crate::paths::join_data("site-crons").join(format!("{domain}.json"))
}

fn cron_d_path(domain: &str) -> PathBuf {
    let safe: String = domain
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    PathBuf::from(format!("/etc/cron.d/cpn-site-{safe}"))
}

fn load_file(domain: &str) -> SiteCronFile {
    let path = cron_store_path(domain);
    let Ok(raw) = fs::read_to_string(&path) else {
        return SiteCronFile {
            schema_version: SCHEMA_VERSION,
            domain: domain.to_string(),
            jobs: Vec::new(),
        };
    };
    serde_json::from_str(&raw).unwrap_or(SiteCronFile {
        schema_version: SCHEMA_VERSION,
        domain: domain.to_string(),
        jobs: Vec::new(),
    })
}

fn save_file(file: &SiteCronFile) -> Result<(), String> {
    let path = cron_store_path(&file.domain);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create cron store: {e}"))?;
    }
    let mut out = file.clone();
    out.schema_version = SCHEMA_VERSION;
    let raw = serde_json::to_string_pretty(&out)
        .map_err(|e| format!("Could not serialize cron jobs: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Could not write cron store: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn valid_cron_field(raw: &str, max: u32) -> bool {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 32 {
        return false;
    }
    if raw == "*" {
        return true;
    }
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return false;
        }
        if let Some(step) = part.strip_prefix("*/") {
            return step
                .parse::<u32>()
                .ok()
                .filter(|n| *n >= 1 && *n <= max)
                .is_some()
                && part.split(',').count() == 1;
        }
        if let Some((a, b)) = part.split_once('-') {
            let Ok(lo) = a.parse::<u32>() else {
                return false;
            };
            let Ok(hi) = b.parse::<u32>() else {
                return false;
            };
            if lo > hi || hi > max {
                return false;
            }
            continue;
        }
        let Ok(n) = part.parse::<u32>() else {
            return false;
        };
        if n > max {
            return false;
        }
    }
    true
}

fn validate_schedule(
    minute: &str,
    hour: &str,
    day: &str,
    month: &str,
    weekday: &str,
) -> Result<(), String> {
    if !valid_cron_field(minute, 59) {
        return Err("Invalid minute field".into());
    }
    if !valid_cron_field(hour, 23) {
        return Err("Invalid hour field".into());
    }
    if !valid_cron_field(day, 31) {
        return Err("Invalid day-of-month field".into());
    }
    if !valid_cron_field(month, 12) {
        return Err("Invalid month field".into());
    }
    if !valid_cron_field(weekday, 7) {
        return Err("Invalid weekday field".into());
    }
    Ok(())
}

fn has_unsafe_shell(cmd: &str) -> bool {
    cmd.chars().any(|c| {
        matches!(
            c,
            ';' | '|' | '&' | '`' | '$' | '(' | ')' | '<' | '>' | '\n' | '\r' | '\0'
        )
    }) || cmd.contains("..")
}

/// Validate command stays jailed under the site home (optional allowlisted interpreter).
pub fn validate_cron_command(site: &SiteRecord, command: &str) -> Result<String, String> {
    let command = command.trim();
    if command.is_empty() {
        return Err("Command is required".into());
    }
    if command.len() > 400 {
        return Err("Command is too long".into());
    }
    if has_unsafe_shell(command) {
        return Err("Command may not include shell metacharacters or path traversal".into());
    }
    let home = site_home_from_record(site);
    let tokens: Vec<&str> = command.split_whitespace().collect();
    if tokens.is_empty() {
        return Err("Command is required".into());
    }
    let (bin, script) = if ALLOWED_INTERPRETERS
        .iter()
        .any(|b| b.eq_ignore_ascii_case(tokens[0]))
    {
        if tokens.len() < 2 {
            return Err("Interpreter commands need a script path under the site home".into());
        }
        (tokens[0], tokens[1])
    } else {
        ("", tokens[0])
    };
    let script_path = if Path::new(script).is_absolute() {
        PathBuf::from(script)
    } else {
        home.join(script)
    };
    let home_canon = home.canonicalize().unwrap_or(home.clone());
    let parent = script_path
        .parent()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()))
        .unwrap_or_else(|| home_canon.clone());
    if !parent.starts_with(&home_canon) && !script_path.starts_with(&home) {
        return Err("Command path must stay under the site home directory".into());
    }
    let mut rebuilt = String::new();
    if !bin.is_empty() {
        rebuilt.push_str(bin);
        rebuilt.push(' ');
    }
    rebuilt.push_str(script);
    for extra in tokens.iter().skip(if bin.is_empty() { 1 } else { 2 }) {
        if has_unsafe_shell(extra) || extra.contains("..") {
            return Err("Command arguments are not allowed to use shell metacharacters".into());
        }
        // Extra args must be relative or under home.
        if Path::new(extra).is_absolute() {
            let p = PathBuf::from(extra);
            if !p.starts_with(&home) {
                return Err("Absolute arguments must stay under the site home".into());
            }
        }
        rebuilt.push(' ');
        rebuilt.push_str(extra);
    }
    Ok(rebuilt)
}

fn render_cron_d(site: &SiteRecord, jobs: &[SiteCronJob]) -> String {
    let home = site_home_from_record(site);
    let mut out = String::from(
        "# Managed by CPN Control Panel Network. Do not edit by hand.\nSHELL=/bin/bash\nPATH=/usr/bin:/bin\nMAILTO=\"\"\n\n",
    );
    for job in jobs {
        if !job.enabled {
            continue;
        }
        let comment = job.comment.trim();
        if !comment.is_empty() {
            out.push_str(&format!("# {}\n", comment.replace('\n', " ")));
        }
        out.push_str(&format!(
            "{m} {h} {d} {mo} {w} root cd '{home}' && {cmd} >> '{home}/logs/cpn-cron.log' 2>&1\n",
            m = job.minute.trim(),
            h = job.hour.trim(),
            d = job.day.trim(),
            mo = job.month.trim(),
            w = job.weekday.trim(),
            home = home.display(),
            cmd = job.command.trim(),
        ));
    }
    out
}

fn sync_cron_d(site: &SiteRecord, jobs: &[SiteCronJob]) -> Result<String, String> {
    let home = site_home_from_record(site);
    let logs = home.join("logs");
    let _ = fs::create_dir_all(&logs);
    let body = render_cron_d(site, jobs);
    let path = cron_d_path(&site.domain);
    // Always keep a copy under CPN data for operators without /etc write.
    let mirror = crate::paths::join_data("site-crons").join(format!("{}.cron", site.domain));
    if let Some(parent) = mirror.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&mirror, &body);

    match fs::write(&path, &body) {
        Ok(()) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
            }
            Ok(format!("Synced system crontab {}", path.display()))
        }
        Err(err) => Ok(format!(
            "Saved CPN cron mirror at {}; system cron.d write failed ({err}). Run the panel as root to sync.",
            mirror.display()
        )),
    }
}

fn new_id() -> String {
    format!("{:x}", now_unix().wrapping_mul(9973) ^ 0xa5a5)
}

pub fn list_site_cron_jobs(domain_raw: &str) -> Result<(SiteRecord, Vec<SiteCronJob>), String> {
    let domain = normalize_domain(domain_raw)?;
    let site = load_site(&domain)?;
    let file = load_file(&domain);
    Ok((site, file.jobs))
}

pub fn add_site_cron_job(
    domain_raw: &str,
    minute: &str,
    hour: &str,
    day: &str,
    month: &str,
    weekday: &str,
    command: &str,
    comment: &str,
) -> Result<(SiteRecord, String), String> {
    let domain = normalize_domain(domain_raw)?;
    let site = load_site(&domain)?;
    validate_schedule(minute, hour, day, month, weekday)?;
    let command = validate_cron_command(&site, command)?;
    let now = now_unix();
    let job = SiteCronJob {
        id: new_id(),
        enabled: true,
        minute: minute.trim().into(),
        hour: hour.trim().into(),
        day: day.trim().into(),
        month: month.trim().into(),
        weekday: weekday.trim().into(),
        command,
        comment: comment.trim().chars().take(120).collect(),
        created_at_unix: now,
        updated_at_unix: now,
    };
    let mut file = load_file(&domain);
    file.domain = domain.clone();
    if file.jobs.len() >= 50 {
        return Err("Maximum of 50 cron jobs per site".into());
    }
    file.jobs.push(job);
    save_file(&file)?;
    let sync = sync_cron_d(&site, &file.jobs)?;
    Ok((site, format!("Cron job added. {sync}")))
}

pub fn update_site_cron_job(
    domain_raw: &str,
    job_id: &str,
    minute: &str,
    hour: &str,
    day: &str,
    month: &str,
    weekday: &str,
    command: &str,
    comment: &str,
    enabled: bool,
) -> Result<(SiteRecord, String), String> {
    let domain = normalize_domain(domain_raw)?;
    let site = load_site(&domain)?;
    validate_schedule(minute, hour, day, month, weekday)?;
    let command = validate_cron_command(&site, command)?;
    let mut file = load_file(&domain);
    let Some(job) = file.jobs.iter_mut().find(|j| j.id == job_id) else {
        return Err("Cron job not found".into());
    };
    job.minute = minute.trim().into();
    job.hour = hour.trim().into();
    job.day = day.trim().into();
    job.month = month.trim().into();
    job.weekday = weekday.trim().into();
    job.command = command;
    job.comment = comment.trim().chars().take(120).collect();
    job.enabled = enabled;
    job.updated_at_unix = now_unix();
    save_file(&file)?;
    let sync = sync_cron_d(&site, &file.jobs)?;
    Ok((site, format!("Cron job updated. {sync}")))
}

pub fn delete_site_cron_job(
    domain_raw: &str,
    job_id: &str,
) -> Result<(SiteRecord, String), String> {
    let domain = normalize_domain(domain_raw)?;
    let site = load_site(&domain)?;
    let mut file = load_file(&domain);
    let before = file.jobs.len();
    file.jobs.retain(|j| j.id != job_id);
    if file.jobs.len() == before {
        return Err("Cron job not found".into());
    }
    save_file(&file)?;
    let sync = sync_cron_d(&site, &file.jobs)?;
    if file.jobs.is_empty() {
        let path = cron_d_path(&domain);
        let _ = fs::remove_file(&path);
    }
    Ok((site, format!("Cron job deleted. {sync}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_site() -> SiteRecord {
        SiteRecord {
            schema_version: 4,
            domain: "example.com".into(),
            owner: "Admin".into(),
            docroot: "/home/example.com/public_html".into(),
            enabled: true,
            engine: None,
            notes: String::new(),
            created_at_unix: 0,
            updated_at_unix: 0,
            vhost_wired: false,
            ssl: Default::default(),
            internal_ip: None,
            owner_suspend_message: String::new(),
            suspended_by: None,
            php_version: None,
            aliases: Vec::new(),
        }
    }

    #[test]
    fn csrf_roundtrip() {
        let t = cron_csrf_token("Admin");
        assert!(verify_cron_csrf("Admin", &t));
        assert!(!verify_cron_csrf("x", &t));
    }

    #[test]
    fn rejects_shell_metacharacters() {
        let site = sample_site();
        assert!(validate_cron_command(&site, "php public_html/a.php; rm -rf /").is_err());
        assert!(validate_cron_command(&site, "php ../etc/passwd").is_err());
    }

    #[test]
    fn accepts_relative_php() {
        let site = sample_site();
        let ok = validate_cron_command(&site, "php public_html/cron.php").unwrap();
        assert_eq!(ok, "php public_html/cron.php");
    }

    #[test]
    fn schedule_fields() {
        assert!(validate_schedule("*/5", "*", "*", "*", "*").is_ok());
        assert!(validate_schedule("60", "*", "*", "*", "*").is_err());
    }
}
