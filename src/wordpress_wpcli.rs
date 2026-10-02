//! WP-CLI detection, bootstrap, and command runner for CPN WordPress tooling.

use crate::panel_ops_docker_probe::command_output_with_timeout;
use crate::paths;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

const WP_CLI_VERSION_TIMEOUT: Duration = Duration::from_secs(4);
/// Core download/install and plugin ZIP installs need minutes, not seconds.
/// Lab/NAT links often need longer than five minutes for `wp core download`.
const WP_CLI_RUN_TIMEOUT: Duration = Duration::from_secs(900);
const CHOWN_TIMEOUT: Duration = Duration::from_secs(8);

/// Skip recursive `chown` / live WP-CLI refresh on restore-sized trees.
pub fn skip_heavy_docroot(path: &Path) -> bool {
    if path == Path::new("/") || path == Path::new("/home") {
        return true;
    }
    path.join("wp-content").join("uploads").is_dir()
        || path.join("home").is_dir()
        || path
            .join("public_html")
            .join("wp-content")
            .join("uploads")
            .is_dir()
}

const WP_CLI_PHAR_URL: &str =
    "https://raw.githubusercontent.com/wp-cli/builds/gh-pages/phar/wp-cli.phar";

/// Raise PHP CLI memory for WP-CLI phar runs (core download OOMs on 128M hosts).
const WP_CLI_PHP_MEMORY_LIMIT: &str = "512M";

#[derive(Debug, Clone)]
pub struct WpCliStatus {
    pub available: bool,
    pub binary: Option<String>,
    pub version: Option<String>,
    pub detail: String,
}

fn php_memory_arg() -> String {
    format!("-d memory_limit={WP_CLI_PHP_MEMORY_LIMIT}")
}

fn apply_wp_cli_php_args(cmd: &mut Command) {
    // System `wp` wrappers honor WP_CLI_PHP_ARGS for the underlying php binary.
    let existing = std::env::var("WP_CLI_PHP_ARGS").unwrap_or_default();
    if existing.contains("memory_limit=") {
        cmd.env("WP_CLI_PHP_ARGS", existing);
        return;
    }
    let mem = php_memory_arg();
    let combined = if existing.trim().is_empty() {
        mem
    } else {
        format!("{existing} {mem}")
    };
    cmd.env("WP_CLI_PHP_ARGS", combined);
}

fn which_wp() -> Option<String> {
    for candidate in ["wp", "/usr/local/bin/wp", "/usr/bin/wp"] {
        // Existence / PATH probe only; version runs through probe_wp_cli_version.
        let looks_present = Path::new(candidate).is_file()
            || Command::new("sh")
                .args(["-c", &format!("command -v '{candidate}' >/dev/null 2>&1")])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
        if looks_present {
            return Some(candidate.to_string());
        }
    }
    let phar = bundled_phar_path();
    if phar.is_file() {
        return Some(phar_bin_spec(&phar));
    }
    None
}

pub fn bundled_phar_path() -> PathBuf {
    paths::join_data("bin").join("wp-cli.phar")
}

fn bundled_version_cache_path() -> PathBuf {
    paths::join_data("bin").join("wp-cli.version")
}

fn phar_bin_spec(phar: &Path) -> String {
    format!("php:{}", phar.display())
}

/// Operator-facing binary label (`php /path` instead of internal `php:/path`).
pub fn format_wp_cli_binary(bin_spec: &str) -> String {
    if let Some(phar) = bin_spec.strip_prefix("php:") {
        format!("php {phar}")
    } else {
        bin_spec.to_string()
    }
}

/// Bundled phar lives under the private CPN data dir (often mode 700). Web users cannot read it.
fn bin_is_private_phar(bin_spec: &str) -> bool {
    let Some(phar) = bin_spec.strip_prefix("php:") else {
        return false;
    };
    let phar_path = Path::new(phar);
    phar_path.starts_with(paths::default_data_dir()) || phar_path == bundled_phar_path()
}

fn write_version_cache(version: &str) {
    let path = bundled_version_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, format!("{}\n", version.trim()));
}

fn read_version_cache() -> Option<String> {
    let raw = fs::read_to_string(bundled_version_cache_path()).ok()?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Parse a WP-CLI version line (`WP-CLI 2.12.0`) from command output.
fn parse_wp_cli_version(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("yikes") || lower.contains("--allow-root") {
            continue;
        }
        if lower.starts_with("wp-cli") {
            return Some(trimmed.to_string());
        }
    }
    let trimmed = output.trim();
    if !trimmed.is_empty()
        && !trimmed.to_ascii_lowercase().contains("yikes")
        && !trimmed.to_ascii_lowercase().contains("--allow-root")
    {
        // Single-line fallback (some builds print only the version number).
        return Some(trimmed.lines().next().unwrap_or(trimmed).trim().to_string());
    }
    None
}

/// Probe `wp --version` / `wp cli version` with root-safe flags. Never surfaces YIKES.
fn probe_wp_cli_version(bin_spec: &str) -> Option<String> {
    // Prefer --version; fall back to `cli version` for odd wrappers.
    for args in [&["--version"][..], &["cli", "version"][..]] {
        if let Ok(out) = run_wp_raw(bin_spec, args, None)
            && let Some(version) = parse_wp_cli_version(&out)
        {
            if bin_is_private_phar(bin_spec) {
                write_version_cache(&version);
            }
            return Some(version);
        }
    }
    // Cached version only when the bundled phar is still present.
    if bin_is_private_phar(bin_spec) && bundled_phar_path().is_file() {
        return read_version_cache();
    }
    None
}

fn php_bin() -> Option<String> {
    // Absolute paths first: panel systemd units often have a narrow PATH without /usr/bin.
    for candidate in [
        "/usr/bin/php",
        "/usr/bin/php85",
        "/usr/bin/php8.5",
        "/usr/bin/php84",
        "/usr/bin/php8.4",
        "/usr/bin/php83",
        "/usr/bin/php8.3",
        "/usr/bin/php82",
        "/usr/bin/php8.2",
        "php",
        "php85",
        "php8.5",
        "php84",
        "php8.4",
        "php83",
        "php8.3",
        "php82",
        "php8.2",
    ] {
        if crate::panel_ops_docker_probe::probe_ok(candidate, &["-v"], Duration::from_secs(2)) {
            return Some(candidate.to_string());
        }
    }
    None
}

fn running_as_root() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn system_user_exists(name: &str) -> bool {
    if name.is_empty()
        || name.contains(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    {
        return false;
    }
    Command::new("getent")
        .args(["passwd", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Web / PHP runtime user for site files (OLS typically `nobody`).
pub fn preferred_site_run_user() -> Option<String> {
    for candidate in ["nobody", "apache", "nginx", "www-data"] {
        if system_user_exists(candidate) {
            return Some(candidate.to_string());
        }
    }
    None
}

/// Recursively chown a docroot to the web runtime user so WP-CLI and PHP can write.
pub fn chown_docroot_to_web_user(path: &Path) -> Result<String, String> {
    let user = preferred_site_run_user().ok_or_else(|| {
        "Could not determine a web runtime user for WordPress file ownership".to_string()
    })?;
    let path_s = path
        .to_str()
        .ok_or_else(|| "Document root path is not valid UTF-8".to_string())?;
    let spec = format!("{user}:{user}");
    if skip_heavy_docroot(path) {
        return Err("Skipped recursive ownership change on a large document root".to_string());
    }
    let mut chown_cmd = Command::new("chown");
    chown_cmd.args(["-R", &spec, path_s]);
    let out = command_output_with_timeout(chown_cmd, CHOWN_TIMEOUT, "chown -R")
        .map_err(|e| format!("Could not set document root ownership: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "Could not set document root ownership to `{spec}`: {}",
            err.trim()
        ));
    }
    Ok(user)
}

/// Map raw WP-CLI stderr/stdout into a short operator-facing message (never YIKES).
pub fn sanitize_wp_cli_error(raw: &str) -> String {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("yikes")
        || lower.contains("running this as root")
        || lower.contains("--allow-root")
        || lower.contains("you probably meant to run this as the user")
    {
        return "WordPress install could not run site tooling with the correct file owner. Check document root permissions and try again.".into();
    }
    if lower.contains("error establishing a database connection") {
        return "WordPress could not connect to MariaDB. Check database credentials and that MariaDB is running.".into();
    }
    if lower.contains("permission denied") || lower.contains("read-only file system") {
        return "WordPress could not write to the document root. Check ownership and permissions."
            .into();
    }
    // Pass through CPN validation / orchestration messages unchanged.
    if !lower.contains("wp-cli")
        && !lower.contains("phar")
        && !lower.starts_with("error:")
        && !lower.contains("fatal error")
    {
        return trimmed.to_string();
    }
    let mut compact = trimmed
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join(" ");
    if compact.len() > 280 {
        compact.truncate(277);
        compact.push('…');
    }
    if compact.is_empty() {
        "WordPress tooling failed. Check logs and try again.".into()
    } else if compact.to_ascii_lowercase().starts_with("wordpress") {
        compact
    } else {
        format!("WordPress tooling failed: {compact}")
    }
}

fn build_wp_argv(
    bin_spec: &str,
    args: &[&str],
    path: Option<&Path>,
) -> Result<Vec<String>, String> {
    let mut argv = Vec::new();
    if let Some(phar) = bin_spec.strip_prefix("php:") {
        let php = php_bin().ok_or_else(|| "PHP CLI not found".to_string())?;
        argv.push(php);
        argv.push(php_memory_arg());
        argv.push(phar.to_string());
    } else {
        argv.push(bin_spec.to_string());
    }
    if let Some(p) = path {
        argv.push(format!("--path={}", p.display()));
    }
    for a in args {
        argv.push((*a).to_string());
    }
    Ok(argv)
}

fn spawn_wp_command(
    bin_spec: &str,
    args: &[&str],
    path: Option<&Path>,
    run_as: Option<&str>,
    allow_root: bool,
) -> Result<Command, String> {
    let mut argv = build_wp_argv(bin_spec, args, path)?;
    if allow_root {
        argv.push("--allow-root".into());
    }

    let mut cmd = if let Some(user) = run_as {
        let mut c = Command::new("sudo");
        c.args(["-n", "-u", user, "-H", "--"]);
        c.arg(&argv[0]);
        if argv.len() > 1 {
            c.args(&argv[1..]);
        }
        c
    } else {
        let mut c = Command::new(&argv[0]);
        if argv.len() > 1 {
            c.args(&argv[1..]);
        }
        c
    };

    if !bin_spec.starts_with("php:") {
        apply_wp_cli_php_args(&mut cmd);
    }
    Ok(cmd)
}

fn should_fall_through_to_allow_root(combined: &str) -> bool {
    let lower = combined.to_ascii_lowercase();
    lower.contains("sudo:")
        || lower.contains("could not open input file")
        || lower.contains("failed to open stream")
        || lower.contains("no such file or directory")
        || lower.contains("permission denied")
        || lower.contains("yikes")
        || lower.contains("running this as root")
}

fn run_wp_raw(bin_spec: &str, args: &[&str], path: Option<&Path>) -> Result<String, String> {
    let as_root = running_as_root();
    // Private bundled phar under /var/lib/cpn (mode 700) is not readable by web users.
    // Global probes (no path) also skip site-user: version/info need --allow-root as root.
    let try_site_user = as_root && path.is_some() && !bin_is_private_phar(bin_spec);
    let site_user = if try_site_user {
        preferred_site_run_user()
    } else {
        None
    };

    // Prefer dropping to the web user so files are not root-owned and WP-CLI stays quiet.
    if let Some(ref user) = site_user {
        if let Some(docroot) = path {
            let _ = chown_docroot_to_web_user(docroot);
        }
        match spawn_wp_command(bin_spec, args, path, Some(user), false) {
            Ok(cmd) => {
                let timeout = if path.is_none() {
                    WP_CLI_VERSION_TIMEOUT
                } else {
                    WP_CLI_RUN_TIMEOUT
                };
                let out = command_output_with_timeout(cmd, timeout, "wp-cli")
                    .map_err(|e| format!("Failed to run WP-CLI as `{user}`: {e}"))?;
                if out.status.success() {
                    return Ok(String::from_utf8_lossy(&out.stdout).to_string());
                }
                // Fall through when sudo/user cannot launch WP-CLI; keep real WP errors.
                let err = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                let combined = format!("{} {}", err.trim(), stdout.trim());
                if !should_fall_through_to_allow_root(&combined) {
                    return Err(sanitize_wp_cli_error(&combined));
                }
            }
            Err(e) => {
                // Continue to allow-root fallback.
                let _ = e;
            }
        }
    }

    let allow_root = as_root;
    let cmd = spawn_wp_command(bin_spec, args, path, None, allow_root)?;
    let timeout = if path.is_none() {
        WP_CLI_VERSION_TIMEOUT
    } else {
        WP_CLI_RUN_TIMEOUT
    };
    let out = command_output_with_timeout(cmd, timeout, "wp-cli")
        .map_err(|e| format!("Failed to run WP-CLI: {e}"))?;
    if out.status.success() {
        // Private phar runs as root with --allow-root; restore web ownership on docroots.
        if as_root
            && bin_is_private_phar(bin_spec)
            && let Some(docroot) = path
        {
            let _ = chown_docroot_to_web_user(docroot);
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        Err(sanitize_wp_cli_error(&format!(
            "{} {}",
            err.trim(),
            stdout.trim()
        )))
    }
}

pub fn detect_wp_cli() -> WpCliStatus {
    match which_wp() {
        Some(bin) => {
            // Prefer the cached phar version so GET /wordpress never blocks on PHP.
            let version = if bin_is_private_phar(&bin) {
                read_version_cache().or_else(|| probe_wp_cli_version(&bin))
            } else {
                probe_wp_cli_version(&bin)
            };
            let detail = if version.is_some() {
                "WP-CLI is available.".into()
            } else {
                "WP-CLI binary found, but version could not be read. Use Ensure WP-CLI to refresh.".into()
            };
            WpCliStatus {
                available: true,
                binary: Some(bin),
                version,
                detail,
            }
        }
        None => WpCliStatus {
            available: false,
            binary: None,
            version: None,
            detail: "WP-CLI not found. Use Ensure WP-CLI, or place `wp` on PATH. Fallback downloads wp-cli.phar under the CPN data bin directory.".into(),
        },
    }
}

fn phar_looks_valid(phar: &Path) -> bool {
    match fs::metadata(phar) {
        Ok(meta) => meta.is_file() && meta.len() > 100_000,
        Err(_) => false,
    }
}

pub fn ensure_wp_cli() -> Result<WpCliStatus, String> {
    let php = php_bin().ok_or_else(|| {
        "PHP CLI not found. Install PHP before using WP-CLI for WordPress installs.".to_string()
    })?;
    let _ = php;

    // Prefer an existing system `wp` when it already reports a real version.
    if let Some(bin) = which_wp() {
        if !bin_is_private_phar(&bin)
            && let Some(version) = probe_wp_cli_version(&bin)
        {
            return Ok(WpCliStatus {
                available: true,
                binary: Some(bin),
                version: Some(version),
                detail: "WP-CLI is available.".into(),
            });
        }
    }

    let bin_dir = paths::join_data("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Could not create {}: {e}", bin_dir.display()))?;
    let phar = bundled_phar_path();
    let mut downloaded = false;
    if !phar_looks_valid(&phar) {
        download_phar(&phar)?;
        downloaded = true;
    }

    let bin = phar_bin_spec(&phar);
    let mut version = probe_wp_cli_version(&bin);
    if version.is_none() {
        // Corrupt or incomplete phar: re-download once and probe again.
        download_phar(&phar)?;
        downloaded = true;
        version = probe_wp_cli_version(&bin);
    }
    let version = version.ok_or_else(|| {
        "WP-CLI was installed, but version could not be read. Check PHP CLI and try Ensure WP-CLI again."
            .to_string()
    })?;
    write_version_cache(&version);

    let detail = if downloaded {
        format!(
            "WP-CLI ensured ({version}) at {} (memory_limit={WP_CLI_PHP_MEMORY_LIMIT}).",
            phar.display()
        )
    } else {
        format!("WP-CLI ensured ({version}).")
    };
    Ok(WpCliStatus {
        available: true,
        binary: Some(bin),
        version: Some(version),
        detail,
    })
}

fn download_phar(dest: &Path) -> Result<(), String> {
    let tmp = dest.with_extension("phar.tmp");
    let tmp_s = tmp.to_str().unwrap_or("/tmp/wp-cli.phar.tmp");
    let curl_ok = Command::new("curl")
        .args([
            "-fsSL",
            "--connect-timeout",
            "20",
            "-o",
            tmp_s,
            WP_CLI_PHAR_URL,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !curl_ok {
        let wget = Command::new("wget")
            .args(["-q", "-O", tmp_s, WP_CLI_PHAR_URL])
            .status()
            .map_err(|e| format!("curl/wget missing for WP-CLI download: {e}"))?;
        if !wget.success() {
            return Err("Failed to download wp-cli.phar (curl and wget both failed)".into());
        }
    }
    fs::rename(&tmp, dest).map_err(|e| format!("Could not place wp-cli.phar: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dest, fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

pub fn wp_run(path: &Path, args: &[&str]) -> Result<String, String> {
    let status = ensure_wp_cli()?;
    let bin = status
        .binary
        .ok_or_else(|| "WP-CLI binary missing after ensure".to_string())?;
    run_wp_raw(&bin, args, Some(path))
}

pub fn wp_option_get(path: &Path, key: &str) -> Result<String, String> {
    wp_run(path, &["option", "get", key]).map(|s| s.trim().to_string())
}

pub fn wp_option_update(path: &Path, key: &str, value: &str) -> Result<(), String> {
    wp_run(path, &["option", "update", key, value]).map(|_| ())
}

pub fn is_wordpress_docroot(path: &Path) -> bool {
    path.join("wp-config.php").is_file() || path.join("wp-includes").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[test]
    fn skip_heavy_detects_uploads_tree() {
        let dir = std::env::temp_dir().join(format!("cpn-wp-heavy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("wp-content").join("uploads")).unwrap();
        assert!(skip_heavy_docroot(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_does_not_panic() {
        let status = detect_wp_cli();
        assert!(!status.detail.is_empty());
    }

    #[test]
    fn sanitize_hides_yikes_root_warning() {
        let raw = "Error: YIKES! It looks like you're running this as root. If you'd like to continue as root, please run this again, adding this flag: --allow-root";
        let msg = sanitize_wp_cli_error(raw);
        assert!(!msg.to_ascii_lowercase().contains("yikes"));
        assert!(!msg.contains("--allow-root"));
        assert!(msg.to_ascii_lowercase().contains("wordpress"));
    }

    #[test]
    fn build_wp_argv_for_system_wp() {
        let argv = build_wp_argv(
            "/usr/local/bin/wp",
            &["core", "download"],
            Some(Path::new("/home/a/public_html")),
        )
        .expect("argv");
        assert_eq!(argv[0], "/usr/local/bin/wp");
        assert!(argv.iter().any(|a| a.starts_with("--path=")));
        assert_eq!(argv[argv.len() - 2], "core");
        assert_eq!(argv[argv.len() - 1], "download");
    }

    #[test]
    fn parse_version_ignores_yikes() {
        let raw = "Error: YIKES! It looks like you're running this as root.\nWP-CLI 2.12.0\n";
        assert_eq!(parse_wp_cli_version(raw).as_deref(), Some("WP-CLI 2.12.0"));
    }

    #[test]
    fn format_binary_for_display() {
        assert_eq!(
            format_wp_cli_binary("php:/var/lib/cpn/bin/wp-cli.phar"),
            "php /var/lib/cpn/bin/wp-cli.phar"
        );
        assert_eq!(
            format_wp_cli_binary("/usr/local/bin/wp"),
            "/usr/local/bin/wp"
        );
    }

    #[test]
    fn fallthrough_detects_private_phar_errors() {
        assert!(should_fall_through_to_allow_root(
            "Could not open input file: /var/lib/cpn/bin/wp-cli.phar"
        ));
        assert!(!should_fall_through_to_allow_root(
            "Error establishing a database connection"
        ));
    }
}
