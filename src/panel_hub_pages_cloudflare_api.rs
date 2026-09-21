//! Cloudflare DNS API Settings tab (OAuth + optional manual token fallback).

use crate::panel_ops_cloudflare::{cloudflare_public, format_verify_time};
use crate::panel_ops_cloudflare_oauth::oauth_public;

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// API Settings body. `tabs` is the shared Manage/API tab bar HTML from the parent page.
pub fn api_settings_body(listen_port: u16, tabs: &str) -> String {
    let pubv = cloudflare_public();
    let oauth = oauth_public(listen_port);
    let sync_en = if pubv.sync_local { " selected" } else { "" };
    let sync_dis = if pubv.sync_local { "" } else { " selected" };
    let tok_sel = if pubv.auth_type == "global_key" {
        ""
    } else {
        " selected"
    };
    let key_sel = if pubv.auth_type == "global_key" {
        " selected"
    } else {
        ""
    };
    let can_test = pubv.configured || oauth.linked;
    let configured = if pubv.configured {
        format!(
            r#"<p class="muted">Token on disk: <code>{}</code> (masked). Leave the token field blank to keep the current secret.</p>"#,
            html_escape(&pubv.token_masked)
        )
    } else {
        r#"<p class="muted">No Cloudflare API token stored yet. Create a token with Zone DNS Edit permissions.</p>"#.into()
    };
    let status_banner = match (
        can_test,
        pubv.last_verify_ok,
        pubv.last_verify_at_unix,
        pubv.last_verify_message.as_deref(),
        pubv.last_zone_count,
    ) {
        (false, _, _, _, _) => {
            r#"<p class="panel-notice" role="status">Not linked yet. Connect with OAuth or add a manual token, then use <strong>Test connection</strong>.</p>"#.to_string()
        }
        (true, Some(true), Some(ts), msg, zones) => {
            let when = format_verify_time(ts);
            let zones_txt = zones
                .map(|n| format!("{n} zone(s)"))
                .unwrap_or_else(|| "zones checked".into());
            let detail = msg.unwrap_or("Connection valid");
            format!(
                r#"<p class="panel-notice success" role="status"><strong>Connection valid</strong> · {zones} · last verified {when}<br><span class="muted">{detail}</span></p>"#,
                zones = html_escape(&zones_txt),
                when = html_escape(&when),
                detail = html_escape(detail),
            )
        }
        (true, Some(false), Some(ts), msg, _) => {
            let when = format_verify_time(ts);
            let detail = msg.unwrap_or("Connection check failed");
            format!(
                r#"<p class="panel-notice error" role="status"><strong>Connection invalid</strong> · last checked {when}<br><span class="muted">{detail}</span></p>"#,
                when = html_escape(&when),
                detail = html_escape(detail),
            )
        }
        (true, _, _, _, _) => {
            if oauth.linked {
                r#"<p class="panel-notice" role="status">OAuth is linked. Click <strong>Test connection</strong> to verify access and list zones.</p>"#.to_string()
            } else {
                r#"<p class="panel-notice" role="status">Credentials are saved. Click <strong>Test connection</strong> to verify access and list zones.</p>"#.to_string()
            }
        }
    };
    let test_disabled = if can_test { "" } else { " disabled" };
    let oauth_status = if oauth.linked {
        format!(
            r#"<p class="panel-notice success" role="status"><span class="plugin-badge installed" style="margin-right:8px;">Connected</span><strong>OAuth linked</strong> · scopes: <code>{}</code></p>"#,
            html_escape(&oauth.scopes)
        )
    } else if oauth.client_configured {
        r#"<p class="panel-notice" role="status">OAuth client saved. Click <strong>Connect with Cloudflare</strong> to authorize DNS access.</p>"#.into()
    } else {
        r#"<p class="muted">Optional: register a Cloudflare OAuth app and connect instead of pasting an API token (DNS link only; not panel login).</p>"#.into()
    };
    let oauth_secret_field = if oauth.client_configured {
        format!(
            r#"<p class="muted">Client secret on disk: <code>{}</code> (masked). Leave blank to keep the current secret.</p>"#,
            html_escape(&oauth.client_secret_masked)
        )
    } else {
        String::new()
    };
    // Connected: Disconnect primary + Re-authorize secondary. Not linked: Connect primary only.
    let oauth_actions = if oauth.linked {
        r#"<div class="cf-actions" style="margin-top:4px;">
<form method="post" action="/dns/cloudflare/oauth/disconnect" style="display:inline;" onsubmit="return confirm('Disconnect Cloudflare OAuth?');">
  <button type="submit" class="btn-primary">Disconnect OAuth</button>
</form>
<form method="post" action="/dns/cloudflare/oauth/connect" style="display:inline;">
  <button type="submit" class="btn-secondary">Re-authorize</button>
</form>
</div>"#
            .to_string()
    } else if oauth.client_configured {
        r#"<div class="cf-actions" style="margin-top:4px;">
<form method="post" action="/dns/cloudflare/oauth/connect" style="display:inline;">
  <button type="submit" class="btn-primary">Connect with Cloudflare</button>
</form>
</div>"#
            .to_string()
    } else {
        r#"<div class="cf-actions" style="margin-top:4px;">
<form method="post" action="/dns/cloudflare/oauth/connect" style="display:inline;">
  <button type="submit" class="btn-primary" disabled>Connect with Cloudflare</button>
</form>
</div>"#
            .to_string()
    };
    // Manual token fallback only when OAuth is disconnected; collapsed by default.
    let manual_fallback = if oauth.linked {
        String::new()
    } else {
        format!(
            r#"<details class="cf-manual-fallback" style="margin-top:28px;max-width:520px;">
  <summary style="cursor:pointer;font-weight:600;font-size:1.05em;padding:8px 0;">Manual API token (fallback)</summary>
  <div style="padding-top:10px;">
  <p class="muted">Use an API Token or Global API Key when you are not linking with OAuth.</p>
  {configured}
  <form method="post" action="/dns/cloudflare/settings" class="stack-form">
    <label for="auth_type">Authentication type</label>
    <select id="auth_type" name="auth_type">
      <option value="api_token"{tok_sel}>API Token (recommended)</option>
      <option value="global_key"{key_sel}>Global API Key (email + key)</option>
    </select>
    <label for="email">Cloudflare Email</label>
    <input id="email" name="email" type="email" placeholder="your@email.com" value="{email}">
    <p class="muted">Optional when using an API Token. Required for Global API Key.</p>
    <label for="api_token">API Token</label>
    <input id="api_token" name="api_token" type="password" autocomplete="new-password" placeholder="Enter your Cloudflare API token">
    <label for="sync_local">Sync Local Records to Cloudflare</label>
    <select id="sync_local" name="sync_local">
      <option value="1"{sync_en}>Enable</option>
      <option value="0"{sync_dis}>Disable</option>
    </select>
    <div style="display:flex;flex-wrap:wrap;gap:8px;margin-top:12px;">
      <button type="submit" class="btn-primary">Save Configuration</button>
    </div>
  </form>
  </div>
</details>"#,
            configured = configured,
            email = html_escape(&pubv.email),
            tok_sel = tok_sel,
            key_sel = key_sel,
            sync_en = sync_en,
            sync_dis = sync_dis,
        )
    };
    format!(
        r#"{tabs}
<div class="cf-api-test" style="margin-bottom:20px;max-width:520px;">
  <form method="post" action="/dns/cloudflare/test">
    <button type="submit" class="btn-secondary"{test_disabled}>Test connection</button>
  </form>
  <p class="muted" style="margin-top:8px;">Checks your Cloudflare link and lists zones you can manage. Secrets stay masked.</p>
  {status_banner}
</div>
<h3>Cloudflare OAuth (DNS link)</h3>
<p class="muted">Redirect URI for your Cloudflare OAuth app: <code>{redirect}</code></p>
{oauth_status}
<form method="post" action="/dns/cloudflare/oauth/client" class="stack-form" style="max-width:520px;margin-bottom:16px;">
  <label for="oauth_client_id">OAuth Client ID</label>
  <input id="oauth_client_id" name="client_id" type="text" value="{oauth_client_id}" autocomplete="off">
  <label for="oauth_client_secret">OAuth Client Secret</label>
  <input id="oauth_client_secret" name="client_secret" type="password" autocomplete="new-password" placeholder="Paste client secret">
  {oauth_secret_field}
  <button type="submit" class="btn-secondary">Save OAuth client</button>
</form>
{oauth_actions}
{manual_fallback}"#,
        tabs = tabs,
        test_disabled = test_disabled,
        status_banner = status_banner,
        redirect = html_escape(&oauth.redirect_uri),
        oauth_status = oauth_status,
        oauth_client_id = html_escape(&oauth.client_id),
        oauth_secret_field = oauth_secret_field,
        oauth_actions = oauth_actions,
        manual_fallback = manual_fallback,
    )
}
