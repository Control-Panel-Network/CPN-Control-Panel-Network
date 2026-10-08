//! Mr Agent site publish helpers: folder mode (default) and confirmed vhost takeover.
//! Panel Install / Activate / settings buttons call these so operators need no SSH.

use crate::plugin_activation::{host_plugin_installed, host_plugin_path};
use crate::plugins::plugins_dir_for_domain;
use crate::plugins_settings::load_plugin_settings;
use crate::sites::load_site;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MR_AGENT_ID: &str = "mrAgent";
pub const HOST_DOMAIN_SENTINEL: &str = "_host";

pub fn is_mr_agent(id: &str) -> bool {
    id.trim().eq_ignore_ascii_case(MR_AGENT_ID)
}

/// Dual-scoped: Host Store + Site Store (site install still allowed without Host).
pub fn is_dual_scoped_plugin(id: &str) -> bool {
    is_mr_agent(id)
}

pub fn normalize_install_mode(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "vhost" | "docroot" | "takeover" => "vhost".into(),
        _ => "folder".into(),
    }
}

pub fn vhost_confirm_accepted(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "confirm"
    )
}

pub fn vhost_impact_lines(domain: &str, plugin_public: &Path) -> Vec<String> {
    vec![
        format!(
            "Document root for `{domain}` will point at {}",
            plugin_public.display()
        ),
        "Visitors to the site apex see Mr Agent instead of the previous site".into(),
        "Existing files under the old docroot are kept on disk but are no longer served".into(),
        "Safer alternative: folder mode publishes only /mr-agent/ under the current docroot".into(),
    ]
}

/// Resolve plugin tree: prefer a full site install, else host-plugins copy.
pub fn resolve_mr_agent_root(domain: &str) -> Result<PathBuf, String> {
    let domain = domain.trim();
    if domain.is_empty() || domain.eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL) {
        let host = host_plugin_path(MR_AGENT_ID);
        if host.join("modules").join("panel_bridge.php").is_file()
            || host.join("cpn-plugin.json").is_file()
        {
            return Ok(host);
        }
        return Err("Mr Agent is not installed on the Host".into());
    }
    if let Ok(site_root) = plugins_dir_for_domain(domain) {
        let site = site_root.join(MR_AGENT_ID);
        if site.join("modules").join("panel_bridge.php").is_file() {
            return Ok(site);
        }
        // Host activation stub: use host tree.
        if site.join("cpn-plugin.json").is_file() && host_plugin_installed(MR_AGENT_ID) {
            return Ok(host_plugin_path(MR_AGENT_ID));
        }
    }
    if host_plugin_installed(MR_AGENT_ID) {
        return Ok(host_plugin_path(MR_AGENT_ID));
    }
    Err(format!(
        "Mr Agent is not installed for `{domain}` and not on the Host"
    ))
}

fn write_mode_marker(plugin_root: &Path, domain: &str, mode: &str) -> Result<(), String> {
    let data = plugin_root.join("data");
    fs::create_dir_all(&data).map_err(|e| format!("Could not create plugin data dir: {e}"))?;
    fs::write(data.join("domain.txt"), format!("{domain}\n"))
        .map_err(|e| format!("Could not write domain marker: {e}"))?;
    fs::write(data.join("install_mode.txt"), format!("{mode}\n"))
        .map_err(|e| format!("Could not write install_mode marker: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&data, fs::Permissions::from_mode(0o700));
        let _ = fs::set_permissions(data.join("domain.txt"), fs::Permissions::from_mode(0o600));
        let _ = fs::set_permissions(
            data.join("install_mode.txt"),
            fs::Permissions::from_mode(0o600),
        );
    }
    Ok(())
}

fn secret_dir_for(domain: &str) -> PathBuf {
    let d = domain.trim();
    if d.is_empty() || d.eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL) {
        PathBuf::from("/var/lib/cpn/mr-agent/_host")
    } else {
        PathBuf::from("/var/lib/cpn/mr-agent").join(d)
    }
}

fn chmod_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
    let _ = mode;
    let _ = path;
}

fn random_access_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("mra{nanos:x}")
        .chars()
        .take(24)
        .collect()
}

/// Ensure `/var/lib/cpn/mr-agent/<domain>/` secrets, chats, locks, and optional config.php.
pub fn ensure_site_secrets(domain: &str) -> Result<String, String> {
    let plugin_root = resolve_mr_agent_root(domain)?;
    let secret = secret_dir_for(domain);
    let parent = PathBuf::from("/var/lib/cpn/mr-agent");
    fs::create_dir_all(&parent).map_err(|e| format!("Could not create mr-agent secret root: {e}"))?;
    chmod_mode(&parent, 0o700);
    fs::create_dir_all(&secret).map_err(|e| format!("Could not create secret dir: {e}"))?;
    chmod_mode(&secret, 0o700);
    for sub in ["chats", "locks"] {
        let p = secret.join(sub);
        fs::create_dir_all(&p).map_err(|e| format!("Could not create {}: {e}", p.display()))?;
        chmod_mode(&p, 0o700);
    }
    let keys = secret.join("keys.json");
    if !keys.is_file() {
        fs::write(&keys, "{\"host\":{},\"users\":{}}\n")
            .map_err(|e| format!("Could not write keys.json: {e}"))?;
        chmod_mode(&keys, 0o600);
    }
    let config = plugin_root.join("config.php");
    let example = plugin_root.join("config.php.example");
    if !config.is_file() && example.is_file() {
        let pass = random_access_password();
        let domain_s = if domain.trim().is_empty()
            || domain.trim().eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL)
        {
            HOST_DOMAIN_SENTINEL
        } else {
            domain.trim()
        };
        let body = format!(
            "<?php\nreturn [\n    'access_password' => '{pass}',\n    'domain' => '{domain_s}',\n    'openai_api_key' => '',\n    'anthropic_api_key' => '',\n    'custom_api_key' => '',\n    'custom_base_url' => '',\n    'local_base_url' => 'http://127.0.0.1:11434/v1',\n    'local_api_key' => '',\n    'local_model' => 'llama3.2:1b',\n];\n"
        );
        fs::write(&config, body).map_err(|e| format!("Could not write config.php: {e}"))?;
        chmod_mode(&config, 0o640);
        let pass_file = secret.join("access.password");
        fs::write(&pass_file, format!("{pass}\n"))
            .map_err(|e| format!("Could not write access.password: {e}"))?;
        chmod_mode(&pass_file, 0o600);
    } else if config.is_file() {
        chmod_mode(&config, 0o640);
    }
    Ok(format!(
        "Secrets ready under {}",
        secret.display()
    ))
}

/// Install-mode from saved plugin settings (defaults to folder).
pub fn install_mode_from_settings(domain: &str) -> String {
    let settings = load_plugin_settings(domain, MR_AGENT_ID).unwrap_or_default();
    normalize_install_mode(
        settings
            .fields
            .get("install_mode")
            .map(String::as_str)
            .unwrap_or("folder"),
    )
}

/// Full panel-mediated setup: secrets + folder (or confirmed vhost) publish.
/// Prefer this over telling operators to run `install.sh` over SSH.
pub fn run_setup(domain: &str, mode: &str, confirm_vhost: &str) -> Result<String, String> {
    let d = domain.trim();
    if d.is_empty() || d.eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL) {
        let host = finalize_host_install()?;
        let secrets = ensure_site_secrets(HOST_DOMAIN_SENTINEL)?;
        return Ok(format!("{host}. {secrets}"));
    }
    let secrets = ensure_site_secrets(d)?;
    // Prefer native publish; fall back to install.sh when present (Linux guests).
    match publish_for_mode(d, mode, confirm_vhost) {
        Ok(pub_notice) => Ok(format!("{secrets}. {pub_notice}")),
        Err(pub_err) => match run_install_sh(d, mode, confirm_vhost) {
            Ok(sh_notice) => Ok(format!("{secrets}. Native publish failed ({pub_err}); {sh_notice}")),
            Err(sh_err) => Err(format!(
                "Setup incomplete: {pub_err}. install.sh fallback: {sh_err}. Use Plugin settings Run setup after fixing permissions."
            )),
        },
    }
}

/// Run plugin `install.sh` when present (same effect as SSH for operators).
pub fn run_install_sh(domain: &str, mode: &str, confirm_vhost: &str) -> Result<String, String> {
    let plugin_root = resolve_mr_agent_root(domain)?;
    let script = plugin_root.join("install.sh");
    if !script.is_file() {
        return Err("install.sh not found in plugin tree".into());
    }
    let mode = normalize_install_mode(mode);
    let mut cmd = Command::new("bash");
    cmd.arg(&script)
        .arg(domain.trim())
        .current_dir(&plugin_root)
        .env("INSTALL_MODE", &mode);
    if mode == "vhost" {
        if !vhost_confirm_accepted(confirm_vhost) {
            return Err(
                "Vhost install cancelled: confirmation required. Use folder mode, or confirm vhost takeover."
                    .into(),
            );
        }
        cmd.env("CONFIRM", "yes");
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Could not run install.sh: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        return Err(format!(
            "install.sh exited {}: {detail}",
            output.status.code().unwrap_or(-1)
        ));
    }
    Ok(format!(
        "Ran install.sh for {} ({mode})",
        domain.trim()
    ))
}

/// Prune chat logs via `modules/cli_prune.php` (panel button; no SSH).
pub fn prune_chat_logs(domain: &str) -> Result<String, String> {
    let plugin_root = resolve_mr_agent_root(domain)?;
    let script = plugin_root.join("modules").join("cli_prune.php");
    if !script.is_file() {
        return Err("cli_prune.php not found in plugin tree".into());
    }
    let arg = if domain.trim().is_empty()
        || domain.trim().eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL)
    {
        HOST_DOMAIN_SENTINEL.to_string()
    } else {
        domain.trim().to_string()
    };
    let output = Command::new("php")
        .arg(&script)
        .arg(&arg)
        .current_dir(&plugin_root)
        .output()
        .map_err(|e| format!("Could not run cli_prune.php: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Prune failed ({}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary = stdout.lines().next().unwrap_or("Prune finished").trim();
    Ok(if summary.is_empty() {
        format!("Pruned chat logs for {arg}")
    } else {
        format!("Pruned chat logs for {arg}: {summary}")
    })
}

/// Publish folder mode: symlink `public/` -> `{docroot}/mr-agent` without wiping site index.
pub fn publish_folder(domain: &str) -> Result<String, String> {
    let site = load_site(domain)?;
    let plugin_root = resolve_mr_agent_root(&site.domain)?;
    let public = plugin_root.join("public");
    if !public.is_dir() {
        return Err("Mr Agent public/ folder is missing".into());
    }
    let docroot = PathBuf::from(&site.docroot);
    if !docroot.is_dir() {
        return Err(format!("Site docroot missing: {}", docroot.display()));
    }
    let target = docroot.join("mr-agent");
    if target.exists() || target.is_symlink() {
        if target.is_symlink() {
            fs::remove_file(&target)
                .map_err(|e| format!("Could not replace old /mr-agent link: {e}"))?;
        } else if target.is_dir() && target.join("index.php").is_file() {
            fs::remove_dir_all(&target)
                .map_err(|e| format!("Could not replace old /mr-agent folder: {e}"))?;
        } else {
            return Err(format!(
                "Refusing to replace unexpected path {}",
                target.display()
            ));
        }
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&public, &target)
            .map_err(|e| format!("Could not create /mr-agent symlink: {e}"))?;
    }
    #[cfg(not(unix))]
    {
        // Windows labs: copy a minimal junction via directory symlink when possible.
        std::os::windows::fs::symlink_dir(&public, &target).map_err(|e| {
            format!(
                "Could not create /mr-agent symlink (Windows): {e}. Use Plugin settings Run setup on the Linux guest."
            )
        })?;
    }
    write_mode_marker(&plugin_root, &site.domain, "folder")?;
    let marker = plugin_root.join("data").join("folder_docroot.txt");
    let _ = fs::write(&marker, format!("{}\n", site.docroot));
    Ok(format!(
        "Published folder mode for {} at /mr-agent (site index unchanged)",
        site.domain
    ))
}

/// Vhost takeover: rewrite site.json docroot to plugin public/. Requires confirm.
pub fn publish_vhost(domain: &str, confirm: &str) -> Result<String, String> {
    if !vhost_confirm_accepted(confirm) {
        return Err(
            "Vhost install cancelled: confirmation required. This takes over the site document root. Use folder mode, or confirm vhost takeover explicitly."
                .into(),
        );
    }
    let site = load_site(domain)?;
    let plugin_root = resolve_mr_agent_root(&site.domain)?;
    let public = plugin_root.join("public");
    if !public.is_dir() {
        return Err("Mr Agent public/ folder is missing".into());
    }
    let public_s = public
        .canonicalize()
        .unwrap_or(public)
        .display()
        .to_string();
    let data = plugin_root.join("data");
    fs::create_dir_all(&data).map_err(|e| format!("Could not create plugin data dir: {e}"))?;
    let prev_marker = data.join("vhost_previous_docroot.txt");
    if !prev_marker.is_file() {
        let _ = fs::write(&prev_marker, format!("{}\n", site.docroot));
    }
    crate::sites::modify_site(
        &site.domain,
        crate::sites::SiteModify {
            docroot: Some(public_s.clone()),
            ..Default::default()
        },
    )?;
    write_mode_marker(&plugin_root, &site.domain, "vhost")?;
    Ok(format!(
        "Published vhost mode for {}: docroot is now {}",
        site.domain, public_s
    ))
}

pub fn publish_for_mode(domain: &str, mode: &str, confirm_vhost: &str) -> Result<String, String> {
    match normalize_install_mode(mode).as_str() {
        "vhost" => publish_vhost(domain, confirm_vhost),
        _ => publish_folder(domain),
    }
}

/// After catalog host install, finalize secrets via install-host.sh when present.
pub fn finalize_host_install() -> Result<String, String> {
    let root = host_plugin_path(MR_AGENT_ID);
    let script = root.join("install-host.sh");
    if !script.is_file() {
        // Catalog may not have landed 1.3.0 yet; still mark host-ready.
        let data = root.join("data");
        let _ = fs::create_dir_all(&data);
        let _ = fs::write(data.join("domain.txt"), "_host\n");
        let _ = fs::write(data.join("install_mode.txt"), "host\n");
        return Ok("Host Mr Agent installed (finalize script not in package yet)".into());
    }
    let status = Command::new("bash")
        .arg(&script)
        .current_dir(&root)
        .env("HOST_PLUGIN_ROOT", root.display().to_string())
        .status()
        .map_err(|e| format!("Could not run install-host.sh: {e}"))?;
    if !status.success() {
        return Err(format!(
            "install-host.sh exited {}",
            status.code().unwrap_or(-1)
        ));
    }
    Ok("Host Mr Agent finalized (panel chat, no site takeover)".into())
}

/// Preferred Expand URL: panel-hosted chat (does not depend on site docroot).
pub fn panel_expand_url(domain: &str) -> String {
    let d = domain.trim();
    if d.is_empty() || d.eq_ignore_ascii_case(HOST_DOMAIN_SENTINEL) {
        "/plugins/mr-agent".into()
    } else {
        let mut enc = String::with_capacity(d.len() * 3);
        for byte in d.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    enc.push(byte as char);
                }
                b' ' => enc.push('+'),
                _ => enc.push_str(&format!("%{byte:02X}")),
            }
        }
        format!("/plugins/mr-agent?domain={enc}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_is_default_mode() {
        assert_eq!(normalize_install_mode(""), "folder");
        assert_eq!(normalize_install_mode("FOLDER"), "folder");
        assert_eq!(normalize_install_mode("vhost"), "vhost");
    }

    #[test]
    fn vhost_needs_truthy_confirm() {
        assert!(!vhost_confirm_accepted(""));
        assert!(vhost_confirm_accepted("yes"));
        assert!(vhost_confirm_accepted("1"));
    }

    #[test]
    fn dual_scoped_is_mr_agent() {
        assert!(is_dual_scoped_plugin("mrAgent"));
        assert!(!is_dual_scoped_plugin("clamav"));
    }

    #[test]
    fn expand_urls() {
        assert_eq!(panel_expand_url(""), "/plugins/mr-agent");
        assert!(panel_expand_url("ai.example.com").contains("ai.example.com"));
    }

    #[test]
    fn secret_dir_host_sentinel() {
        assert!(secret_dir_for("_host")
            .display()
            .to_string()
            .ends_with("_host"));
        assert!(secret_dir_for("ai.example.com")
            .display()
            .to_string()
            .contains("ai.example.com"));
    }
}
