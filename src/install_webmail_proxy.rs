//! Nginx / Caddy / OpenLiteSpeed reverse-proxy snippets for webmail.

pub fn nginx_webmail_conf(docroot: &str) -> String {
    format!(
        "# Managed by CPN (issue #6)\n\
         server {{\n\
           listen 127.0.0.1:8080;\n\
           server_name localhost;\n\
           root {docroot};\n\
           index index.php index.html;\n\
           location / {{\n\
             try_files $uri $uri/ /index.php?$query_string;\n\
           }}\n\
           location ~ \\.php$ {{\n\
             include fastcgi_params;\n\
             fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n\
             fastcgi_pass unix:/run/php-fpm/cpn-webmail.sock;\n\
           }}\n\
           location ~ /(\\.|config|temp|logs|data) {{\n\
             deny all;\n\
           }}\n\
         }}\n"
    )
}

pub fn caddy_webmail_snippet(docroot: &str) -> String {
    format!(
        "# Managed by CPN (issue #6)\n\
         http://127.0.0.1:8080 {{\n\
           @denied path /data/* /temp/* /logs/* /config/* /./*\n\
           handle @denied {{\n\
             respond 403\n\
           }}\n\
           root * {docroot}\n\
           php_fastcgi unix//run/php-fpm/cpn-webmail.sock\n\
           file_server\n\
         }}\n"
    )
}

pub fn ols_webmail_vhconf(docroot: &str) -> String {
    format!(
        "docRoot                   {docroot}/\n\
         enableGzip                1\n\
         index  {{\n\
           useServer               0\n\
           indexFiles              index.php, index.html\n\
         }}\n\
         extprocessor cpnphp {{\n\
           type                    fcgi\n\
           address                 uds:///run/php-fpm/cpn-webmail.sock\n\
           maxConns                8\n\
           initTimeout             60\n\
           retryTimeout            0\n\
           persistConn             1\n\
           respBuffer              0\n\
           autoStart               0\n\
         }}\n\
         scriptHandler  {{\n\
           add                     fcgi:cpnphp php\n\
         }}\n\
         context /data/ {{\n\
           type                    NULL\n\
           location                {docroot}/data/\n\
           allowBrowse             0\n\
           accessControl  {{\n\
             deny                  *\n\
           }}\n\
         }}\n\
         context /temp/ {{\n\
           type                    NULL\n\
           location                {docroot}/temp/\n\
           allowBrowse             0\n\
           accessControl  {{\n\
             deny                  *\n\
           }}\n\
         }}\n\
         context /logs/ {{\n\
           type                    NULL\n\
           location                {docroot}/logs/\n\
           allowBrowse             0\n\
           accessControl  {{\n\
             deny                  *\n\
           }}\n\
         }}\n\
         context / {{\n\
           type                    NULL\n\
           location                {docroot}/\n\
           allowBrowse             1\n\
           rewrite  {{\n\
             enable                1\n\
           }}\n\
           addDefaultCharset       off\n\
         }}\n\
         rewrite  {{\n\
           enable                  1\n\
           rules                   <<<END_rules\n\
RewriteRule ^(.*)$ - [E=HTTP_AUTHORIZATION:%{{HTTP:Authorization}}]\n\
END_rules\n\
         }}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_webmail_runtime::SNAPPYMAIL_DATA_DIR;

    #[test]
    fn frontend_configs_deny_sensitive_paths() {
        let nginx = nginx_webmail_conf("/opt/cpn-webmail/snappymail");
        assert!(nginx.contains("deny all") && nginx.contains("temp|logs|data"));
        let caddy = caddy_webmail_snippet("/opt/cpn-webmail/snappymail");
        assert!(caddy.contains("respond 403") && caddy.contains("/data/*"));
        let vh = ols_webmail_vhconf("/opt/cpn-webmail/snappymail");
        assert!(vh.contains("context /data/") && vh.contains("deny                  *"));
        assert!(SNAPPYMAIL_DATA_DIR.starts_with("/var/lib/"));
    }
}
