//! Site-scoped web terminal: authenticated WebSocket bridged to a login shell under site home.

use crate::http_helpers::websocket_origin_ok;
use crate::installer::AppState;
use crate::panel_site_tools_security::{
    check_terminal_rate_limit, terminal_csrf_token, verify_terminal_csrf,
};
use crate::sites::{SiteRecord, site_home_from_record};
use crate::site_acl::{SitePerm, require_manage_site};
use actix_web::{HttpRequest, HttpResponse, web};
use futures_util::StreamExt as _;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

pub fn terminal_page_script(domain: &str, csrf: &str) -> String {
    let domain_js = serde_json::to_string(domain).unwrap_or_else(|_| "\"\"".into());
    let csrf_js = serde_json::to_string(csrf).unwrap_or_else(|_| "\"\"".into());
    format!(
        r##"
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/xterm@5.3.0/css/xterm.min.css">
<script src="https://cdn.jsdelivr.net/npm/xterm@5.3.0/lib/xterm.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/xterm-addon-fit@0.8.0/lib/xterm-addon-fit.min.js"></script>
<div id="cpn-term" style="height:min(62vh,520px);border:1px solid var(--m-line);border-radius:12px;padding:8px;background:#0b0d12;"></div>
<p class="manage-muted" id="cpn-term-status">Connecting…</p>
<script>
(function(){{
  const domain = {domain_js};
  const csrf = {csrf_js};
  const status = document.getElementById("cpn-term-status");
  const term = new Terminal({{
    cursorBlink: true,
    fontSize: 13,
    theme: {{ background: "#0b0d12", foreground: "#e5e7eb" }}
  }});
  const fit = new FitAddon.FitAddon();
  term.loadAddon(fit);
  term.open(document.getElementById("cpn-term"));
  fit.fit();
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  const url = proto + "//" + location.host + "/api/websites/terminal/ws?domain=" +
    encodeURIComponent(domain) + "&csrf=" + encodeURIComponent(csrf);
  const ws = new WebSocket(url);
  ws.binaryType = "arraybuffer";
  ws.onopen = function() {{
    status.textContent = "Connected. Shell starts in the site home directory.";
    try {{
      ws.send(JSON.stringify({{ type: "resize", cols: term.cols, rows: term.rows }}));
    }} catch (e) {{}}
  }};
  ws.onmessage = function(ev) {{
    if (typeof ev.data === "string") {{
      term.write(ev.data);
    }} else {{
      term.write(new Uint8Array(ev.data));
    }}
  }};
  ws.onclose = function() {{ status.textContent = "Disconnected."; }};
  ws.onerror = function() {{ status.textContent = "Terminal connection error."; }};
  term.onData(function(data) {{
    if (ws.readyState === 1) {{ ws.send(data); }}
  }});
  window.addEventListener("resize", function() {{
    fit.fit();
    if (ws.readyState === 1) {{
      ws.send(JSON.stringify({{ type: "resize", cols: term.cols, rows: term.rows }}));
    }}
  }});
}})();
</script>
"##,
        domain_js = domain_js,
        csrf_js = csrf_js,
    )
}

pub fn tab_terminal(site: &SiteRecord, username: &str) -> String {
    let csrf = terminal_csrf_token(username, &site.domain);
    let home = site_home_from_record(site);
    format!(
        r#"<div class="manage-log-panel">
  <h3>Web terminal</h3>
  <p class="manage-muted">Interactive shell scoped to <code>{home}</code>. Session cookie and CSRF required. Rate-limited.</p>
  {script}
</div>"#,
        home = crate::panel_website_manage_ui::html_escape(&home.display().to_string()),
        script = terminal_page_script(&site.domain, &csrf),
    )
}

fn shell_command(home: &PathBuf) -> Command {
    let mut cmd = Command::new("script");
    cmd.arg("-qfc")
        .arg("exec /bin/bash --login")
        .arg("/dev/null")
        .current_dir(home)
        .env("HOME", home)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cmd
}

#[derive(Debug, serde::Deserialize)]
pub struct TerminalWsQuery {
    domain: String,
    csrf: String,
}

pub async fn websites_terminal_ws(
    http: HttpRequest,
    body: web::Payload,
    state: web::Data<Arc<AppState>>,
    query: web::Query<TerminalWsQuery>,
) -> actix_web::Result<HttpResponse> {
    use crate::auth_api::panel_user_from_request;

    let Some(user) = panel_user_from_request(&state, &http) else {
        return Ok(HttpResponse::Unauthorized().finish());
    };
    if !websocket_origin_ok(&http, state.allow_remote, &state.allowed_hosts) {
        return Ok(HttpResponse::Forbidden().finish());
    }
    if !verify_terminal_csrf(&user, &query.domain, &query.csrf) {
        return Ok(HttpResponse::Forbidden().body("Invalid terminal CSRF"));
    }
    if let Err(err) = check_terminal_rate_limit(&user) {
        return Ok(HttpResponse::TooManyRequests().body(err));
    }
    let site = match require_manage_site(&user, &query.domain, SitePerm::Enable) {
        Ok(s) => s,
        Err(_) => return Ok(HttpResponse::Forbidden().finish()),
    };
    let home = site_home_from_record(&site);
    if !home.is_dir() {
        return Ok(HttpResponse::BadRequest().body("Site home directory missing"));
    }

    let (response, mut session, mut msg_stream) = actix_ws::handle(&http, body)?;

    actix_web::rt::spawn(async move {
        let mut child = match shell_command(&home).spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = session
                    .text(format!("Could not start shell: {e}\n"))
                    .await;
                let _ = session.close(None).await;
                return;
            }
        };
        let mut stdin = match child.stdin.take() {
            Some(s) => s,
            None => {
                let _ = session.text("Shell stdin unavailable\n").await;
                let _ = session.close(None).await;
                return;
            }
        };
        let mut stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let _ = session.text("Shell stdout unavailable\n").await;
                let _ = session.close(None).await;
                return;
            }
        };
        // Drain stderr into the same stream so errors are visible.
        let mut stderr = child.stderr.take();

        let _ = session
            .text(format!("CPN site shell · {}\n", home.display()))
            .await;

        let mut out_buf = [0u8; 4096];
        let mut err_buf = [0u8; 1024];
        loop {
            tokio::select! {
                read = stdout.read(&mut out_buf) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            let chunk = String::from_utf8_lossy(&out_buf[..n]).into_owned();
                            if session.text(chunk).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                read = async {
                    if let Some(err) = stderr.as_mut() {
                        err.read(&mut err_buf).await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    match read {
                        Ok(0) => {
                            stderr = None;
                        }
                        Ok(n) => {
                            let chunk = String::from_utf8_lossy(&err_buf[..n]).into_owned();
                            if session.text(chunk).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => {
                            stderr = None;
                        }
                    }
                }
                message = msg_stream.next() => {
                    match message {
                        Some(Ok(actix_ws::Message::Text(text))) => {
                            let t = text.to_string();
                            if t.starts_with('{') {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                                    if v.get("type").and_then(|x| x.as_str()) == Some("resize") {
                                        continue;
                                    }
                                }
                            }
                            if stdin.write_all(t.as_bytes()).await.is_err() {
                                break;
                            }
                            let _ = stdin.flush().await;
                        }
                        Some(Ok(actix_ws::Message::Binary(bin))) => {
                            if stdin.write_all(&bin).await.is_err() {
                                break;
                            }
                            let _ = stdin.flush().await;
                        }
                        Some(Ok(actix_ws::Message::Ping(p))) => {
                            let _ = session.pong(&p).await;
                        }
                        Some(Ok(actix_ws::Message::Close(reason))) => {
                            let _ = session.close(reason).await;
                            break;
                        }
                        None | Some(Err(_)) => break,
                        _ => {}
                    }
                }
            }
        }
        let _ = child.kill().await;
        let _ = session.close(None).await;
    });

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::with_test_data_dir;

    #[test]
    fn page_script_has_ws_path() {
        with_test_data_dir(|| {
            let html = terminal_page_script("example.com", "1.abc");
            assert!(html.contains("/api/websites/terminal/ws"));
            assert!(html.contains("xterm"));
            assert!(!html.to_lowercase().contains("cyberpanel"));
        });
    }
}
