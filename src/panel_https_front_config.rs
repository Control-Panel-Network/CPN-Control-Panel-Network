//! Pure config snippets and OLS httpd helpers for panel HTTPS front proxy.

pub const OLS_VH_NAME: &str = "CPNPanel";
pub const ACME_WEBROOT: &str = "/var/lib/cpn/acme-webroot";
pub const HTTPD_CONF: &str = "/usr/local/lsws/conf/httpd_config.conf";
pub const NGINX_CONF: &str = "/etc/nginx/conf.d/cpn-panel-https.conf";
pub const CADDY_SNIPPET: &str = "/etc/caddy/Caddyfile.d/cpn-panel-https.caddy";

/// Pure OLS vhconf body for panel reverse proxy (+ ACME webroot).
pub fn ols_panel_proxy_vhconf(listen_port: u16, acme_webroot: &str) -> String {
    format!(
        "docRoot                   {acme_webroot}/\n\
         enableGzip                1\n\
         extprocessor cpnpanel {{\n\
           type                    proxy\n\
           address                 http://127.0.0.1:{listen_port}\n\
           maxConns                200\n\
           pcKeepAliveTimeout      60\n\
           initTimeout             60\n\
           retryTimeout            0\n\
           respBuffer              0\n\
         }}\n\
         context /.well-known/ {{\n\
           location                {acme_webroot}/.well-known/\n\
           allowBrowse             1\n\
           addDefaultCharset       off\n\
         }}\n\
         context / {{\n\
           type                    proxy\n\
           handler                 cpnpanel\n\
           addDefaultCharset       off\n\
         }}\n"
    )
}

/// Pure nginx server blocks for panel hostname (HTTP ACME + optional HTTPS proxy).
pub fn nginx_panel_proxy_conf(
    hostname: &str,
    listen_port: u16,
    acme_webroot: &str,
    cert: Option<(&str, &str)>,
) -> String {
    let mut out = String::new();
    out.push_str("# Managed by CPN panel HTTPS front (do not edit by hand)\n");
    out.push_str(&format!(
        "server {{\n\
           listen 80;\n\
           listen [::]:80;\n\
           server_name {hostname};\n\
           location /.well-known/acme-challenge/ {{\n\
             root {acme_webroot};\n\
           }}\n"
    ));
    if let Some((fullchain, privkey)) = cert {
        out.push_str(
            "  location / {\n\
               return 301 https://$host$request_uri;\n\
             }\n\
           }\n",
        );
        out.push_str(&format!(
            "server {{\n\
               listen 443 ssl http2;\n\
               listen [::]:443 ssl http2;\n\
               server_name {hostname};\n\
               ssl_certificate {fullchain};\n\
               ssl_certificate_key {privkey};\n\
               location / {{\n\
                 proxy_http_version 1.1;\n\
                 proxy_set_header Host $host;\n\
                 proxy_set_header X-Real-IP $remote_addr;\n\
                 proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n\
                 proxy_set_header X-Forwarded-Proto $scheme;\n\
                 proxy_set_header Upgrade $http_upgrade;\n\
                 proxy_set_header Connection \"upgrade\";\n\
                 proxy_pass http://127.0.0.1:{listen_port};\n\
               }}\n\
             }}\n"
        ));
    } else {
        out.push_str(&format!(
            "  location / {{\n\
               proxy_http_version 1.1;\n\
               proxy_set_header Host $host;\n\
               proxy_set_header X-Real-IP $remote_addr;\n\
               proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n\
               proxy_set_header X-Forwarded-Proto $scheme;\n\
               proxy_set_header Upgrade $http_upgrade;\n\
               proxy_set_header Connection \"upgrade\";\n\
               proxy_pass http://127.0.0.1:{listen_port};\n\
             }}\n\
           }}\n"
        ));
    }
    out
}

/// Pure Caddyfile snippet for panel hostname (Caddy manages TLS when public DNS works).
pub fn caddy_panel_proxy_snippet(hostname: &str, listen_port: u16) -> String {
    format!(
        "# Managed by CPN panel HTTPS front\n\
         {hostname} {{\n\
           reverse_proxy 127.0.0.1:{listen_port}\n\
         }}\n"
    )
}

pub fn insert_map_before_catchall(conf: &str, listener: &str, map_line: &str) -> Option<String> {
    if conf.contains(map_line.trim()) {
        return None;
    }
    let marker = format!("listener {listener}");
    let start = conf.find(&marker)?;
    let rest = &conf[start..];
    let end_rel = rest.find("\n}")?;
    let block_end = start + end_rel;
    let before = &conf[..block_end];
    let after = &conf[block_end..];
    if let Some(catch) = before.rfind("map ") {
        let catch_slice = &before[catch..];
        if catch_slice.contains('*') {
            let mut out = String::with_capacity(conf.len() + map_line.len() + 2);
            out.push_str(&before[..catch]);
            out.push_str(map_line);
            if !map_line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&before[catch..]);
            out.push_str(after);
            return Some(out);
        }
    }
    let mut out = String::with_capacity(conf.len() + map_line.len() + 2);
    out.push_str(before);
    out.push('\n');
    out.push_str(map_line.trim_end());
    out.push('\n');
    out.push_str(after);
    Some(out)
}

pub fn ensure_ols_https_listener(
    conf: &str,
    hostname: &str,
    fullchain: &str,
    privkey: &str,
) -> String {
    let map_line = format!("  map                      {OLS_VH_NAME} {hostname}\n");
    if conf.contains("listener CPNHttps") {
        let mut updated = conf.to_string();
        if let Some(next) = insert_map_before_catchall(&updated, "CPNHttps", &map_line) {
            updated = next;
        }
        if !updated.contains(fullchain) || !updated.contains(privkey) {
            updated = rewrite_ols_listener_certs(&updated, fullchain, privkey);
        }
        return updated;
    }
    let mut out = conf.to_string();
    out.push_str(&format!(
        "\nlistener CPNHttps {{\n\
           address                 *:443\n\
           secure                  1\n\
           keyFile                 {privkey}\n\
           certFile                {fullchain}\n\
           map                     {OLS_VH_NAME} {hostname}\n\
         }}\n"
    ));
    out
}

fn rewrite_ols_listener_certs(conf: &str, fullchain: &str, privkey: &str) -> String {
    let marker = "listener CPNHttps";
    let Some(start) = conf.find(marker) else {
        return conf.to_string();
    };
    let rest = &conf[start..];
    let Some(end_rel) = rest.find("\n}") else {
        return conf.to_string();
    };
    let block_end = start + end_rel + 2;
    let block = &conf[start..block_end];
    let mut rewritten = String::new();
    for line in block.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("keyFile") {
            rewritten.push_str(&format!("  keyFile                 {privkey}\n"));
        } else if trimmed.starts_with("certFile") {
            rewritten.push_str(&format!("  certFile                {fullchain}\n"));
        } else {
            rewritten.push_str(line);
            rewritten.push('\n');
        }
    }
    let mut out = String::new();
    out.push_str(&conf[..start]);
    out.push_str(rewritten.trim_end());
    out.push('\n');
    out.push_str(&conf[block_end..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ols_vhconf_proxies_listen_port_and_acme() {
        let body = ols_panel_proxy_vhconf(2087, "/var/lib/cpn/acme-webroot");
        assert!(body.contains("http://127.0.0.1:2087"));
        assert!(body.contains("type                    proxy"));
        assert!(body.contains("context /.well-known/"));
        assert!(!body.contains("https://127.0.0.1:2087"));
    }

    #[test]
    fn nginx_conf_http_only_without_cert() {
        let body =
            nginx_panel_proxy_conf("panel.example.com", 2087, "/var/lib/cpn/acme-webroot", None);
        assert!(body.contains("server_name panel.example.com"));
        assert!(body.contains("proxy_pass http://127.0.0.1:2087"));
        assert!(body.contains("acme-challenge"));
        assert!(!body.contains("listen 443"));
    }

    #[test]
    fn nginx_conf_https_when_cert_paths_given() {
        let body = nginx_panel_proxy_conf(
            "panel.example.com",
            2087,
            "/var/lib/cpn/acme-webroot",
            Some((
                "/etc/letsencrypt/live/panel.example.com/fullchain.pem",
                "/etc/letsencrypt/live/panel.example.com/privkey.pem",
            )),
        );
        assert!(body.contains("listen 443 ssl"));
        assert!(body.contains("ssl_certificate "));
        assert!(body.contains("return 301 https://$host$request_uri"));
    }

    #[test]
    fn caddy_snippet_uses_hostname_without_port() {
        let body = caddy_panel_proxy_snippet("panel.example.com", 2087);
        assert!(body.contains("panel.example.com {"));
        assert!(body.contains("reverse_proxy 127.0.0.1:2087"));
        assert!(!body.contains(":2087 {"));
    }

    #[test]
    fn never_builds_https_on_listen_port_in_snippets() {
        let ols = ols_panel_proxy_vhconf(2087, "/tmp/acme");
        let nginx = nginx_panel_proxy_conf("a.example.com", 2087, "/tmp/acme", None);
        let caddy = caddy_panel_proxy_snippet("a.example.com", 2087);
        for body in [ols, nginx, caddy] {
            assert!(!body.contains("https://127.0.0.1:2087"));
            assert!(!body.contains("https://0.0.0.0:2087"));
        }
    }

    #[test]
    fn ols_https_listener_appends_once() {
        let base = "listener CPNHttp {\n  address *:80\n  secure 0\n  map CPN *\n}\n";
        let once = ensure_ols_https_listener(
            base,
            "panel.example.com",
            "/c/fullchain.pem",
            "/c/privkey.pem",
        );
        assert!(once.contains("listener CPNHttps"));
        assert!(once.contains("map                     CPNPanel panel.example.com"));
        let twice = ensure_ols_https_listener(
            &once,
            "panel.example.com",
            "/c/fullchain.pem",
            "/c/privkey.pem",
        );
        assert_eq!(twice.matches("listener CPNHttps").count(), 1);
    }
}
