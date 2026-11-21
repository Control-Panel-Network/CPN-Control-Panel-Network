//! Write Nginx / Caddy / OpenLiteSpeed loopback listeners for CPN webmail.

use crate::install_journal;
use crate::install_webmail_proxy::{
    caddy_webmail_snippet, nginx_webmail_conf, ols_webmail_listener_block, ols_webmail_vhconf,
};
use std::path::{Path, PathBuf};

const STAGE: &str = "webmail_runtime";
const NGINX_CONF: &str = "/etc/nginx/conf.d/cpn-webmail.conf";
const CADDY_SNIPPET: &str = "/etc/caddy/Caddyfile.d/cpn-webmail.caddy";

pub fn configure_nginx_proxy(docroot: &str, port: u16) -> Result<(), String> {
    install_journal::write_file_tracked(
        STAGE,
        Path::new(NGINX_CONF),
        &nginx_webmail_conf(docroot, port),
    )?;
    Ok(())
}

pub fn configure_caddy_proxy(docroot: &str, port: u16) -> Result<(), String> {
    std::fs::create_dir_all("/etc/caddy/Caddyfile.d").map_err(|error| error.to_string())?;
    install_journal::write_file_tracked(
        STAGE,
        Path::new(CADDY_SNIPPET),
        &caddy_webmail_snippet(docroot, port),
    )?;

    let main = Path::new("/etc/caddy/Caddyfile");
    let import_line = "import /etc/caddy/Caddyfile.d/*.caddy\n";
    let mut body = if main.exists() {
        std::fs::read_to_string(main).unwrap_or_default()
    } else {
        String::new()
    };
    if !body.contains("Caddyfile.d/*.caddy") {
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(import_line);
        install_journal::write_file_tracked(STAGE, main, &body)?;
    }
    Ok(())
}

pub fn configure_ols_proxy(docroot: &str, port: u16) -> Result<(), String> {
    let vh_dir = "/usr/local/lsws/conf/vhosts/CPNWebmail";
    std::fs::create_dir_all(vh_dir).map_err(|error| error.to_string())?;
    let vhconf_path_buf = PathBuf::from(format!("{vh_dir}/vhconf.conf"));
    install_journal::write_file_tracked(STAGE, &vhconf_path_buf, &ols_webmail_vhconf(docroot))?;
    if !vhconf_path_buf.exists() {
        return Err(format!(
            "OpenLiteSpeed rejected {vh_dir}/vhconf.conf (file missing after write)"
        ));
    }

    let httpd = "/usr/local/lsws/conf/httpd_config.conf";
    let mut conf = std::fs::read_to_string(httpd).unwrap_or_default();
    if !conf.contains("virtualHost CPNWebmail") {
        conf.push_str(&ols_webmail_listener_block(port));
        install_journal::write_file_tracked(STAGE, Path::new(httpd), &conf)?;
        return Ok(());
    }
    let updated = rewrite_ols_webmail_listener_address(&conf, port);
    if updated != conf {
        install_journal::write_file_tracked(STAGE, Path::new(httpd), &updated)?;
    }
    Ok(())
}

pub fn rewrite_ols_webmail_listener_address(conf: &str, port: u16) -> String {
    let mut out = String::with_capacity(conf.len() + 16);
    let mut in_listener = false;
    for line in conf.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("listener CPNWebmailHttp") {
            in_listener = true;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_listener && trimmed.starts_with("address") {
            out.push_str(&format!("  address                 127.0.0.1:{port}\n"));
            continue;
        }
        if in_listener && trimmed == "}" {
            in_listener = false;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !conf.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

pub fn nginx_conf_path() -> &'static str {
    NGINX_CONF
}

pub fn caddy_snippet_path() -> &'static str {
    CADDY_SNIPPET
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ols_listener_address_rewrites_port() {
        let conf = "listener CPNWebmailHttp {\n  address                 127.0.0.1:8080\n  secure                  0\n}\n";
        let updated = rewrite_ols_webmail_listener_address(conf, 18081);
        assert!(updated.contains("127.0.0.1:18081"));
        assert!(!updated.contains("127.0.0.1:8080"));
    }
}
