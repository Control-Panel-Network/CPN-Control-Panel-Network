//! Temporary HTTP redirect helper on the previous listen port during a migration window.
//! Dual-listen: the main installer binds the new port; this task binds the old port only.

use crate::panel_network::{PortMigration, load_panel_hostname, public_base_url};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

const READ_BUDGET: Duration = Duration::from_secs(3);
const MAX_REQUEST_BYTES: usize = 8192;

fn request_path(buffer: &[u8]) -> String {
    let text = String::from_utf8_lossy(buffer);
    let first_line = text.lines().next().unwrap_or("GET / HTTP/1.1");
    let mut parts = first_line.split_whitespace();
    let _method = parts.next();
    let path = parts.next().unwrap_or("/");
    if path.is_empty() {
        "/".into()
    } else {
        path.to_string()
    }
}

fn redirect_location(migration: &PortMigration, request_host: Option<&str>, path: &str) -> String {
    let base = if let Some(hostname) = load_panel_hostname() {
        format!("https://{hostname}")
    } else {
        let host = request_host
            .and_then(|value| value.split(':').next())
            .filter(|value| !value.is_empty())
            .unwrap_or("127.0.0.1");
        public_base_url(migration.new_port, Some(host))
    };
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    format!("{base}{path}")
}

fn host_header(buffer: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(buffer);
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line
            .strip_prefix("Host:")
            .or_else(|| line.strip_prefix("host:"))
        {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

async fn serve_one(mut stream: tokio::net::TcpStream, migration: PortMigration) {
    let mut buf = vec![0u8; MAX_REQUEST_BYTES];
    let read = match timeout(READ_BUDGET, stream.read(&mut buf)).await {
        Ok(Ok(n)) => n,
        _ => 0,
    };
    let path = if read > 0 {
        request_path(&buf[..read])
    } else {
        "/".into()
    };
    let host = if read > 0 {
        host_header(&buf[..read])
    } else {
        None
    };
    let location = redirect_location(&migration, host.as_deref(), &path);
    let body = format!(
        "<!DOCTYPE html><html><head><meta http-equiv=\"refresh\" content=\"0;url={loc}\"></head>\
         <body><p>Moved to <a href=\"{loc}\">{loc}</a></p></body></html>",
        loc = html_escape(&location)
    );
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {len}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        location = location,
        len = body.len(),
        body = body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Bind `migration.old_port` on each host and redirect until the task is cancelled or bind fails.
pub async fn run_redirect_listeners(hosts: Vec<String>, migration: PortMigration) {
    let mut join_set = tokio::task::JoinSet::new();
    for host in hosts {
        let migration = migration.clone();
        join_set.spawn(async move {
            let addr = format!("{}:{}", host, migration.old_port);
            let listener = match TcpListener::bind(&addr).await {
                Ok(listener) => listener,
                Err(error) => {
                    eprintln!(
                        "cpn-installer: could not bind old-port redirect helper on {addr}: {error}"
                    );
                    return;
                }
            };
            println!(
                "  Old-port redirect: {addr} -> new port {} until unix {}",
                migration.new_port, migration.expires_at
            );
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let migration = migration.clone();
                        tokio::spawn(async move {
                            serve_one(stream, migration).await;
                        });
                    }
                    Err(error) => {
                        eprintln!("cpn-installer: old-port accept error: {error}");
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        });
    }
    while join_set.join_next().await.is_some() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;
    use crate::panel_network::OldPortPolicy;

    #[test]
    fn builds_redirect_with_port() {
        with_test_data_dir(|| {
            let migration = PortMigration {
                old_port: 2087,
                new_port: 9443,
                mode: OldPortPolicy::Redirect1m,
                expires_at: 0,
            };
            let location = redirect_location(&migration, Some("10.0.0.8:2087"), "/login");
            assert_eq!(location, "http://10.0.0.8:9443/login");
        });
    }

    #[test]
    fn parses_path_from_request_line() {
        let req = b"GET /status?x=1 HTTP/1.1\r\nHost: 127.0.0.1:2087\r\n\r\n";
        assert_eq!(request_path(req), "/status?x=1");
        assert_eq!(host_header(req).as_deref(), Some("127.0.0.1:2087"));
    }
}
