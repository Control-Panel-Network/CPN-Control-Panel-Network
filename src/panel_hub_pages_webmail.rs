//! Email > Webmail HTML (open client, internal embed, path regen).

use crate::mail_accounts::list_accounts_public;
use crate::panel_hubs::{feature_shell, not_configured_body};
use crate::panel_webmail::{
    WebmailPanelConfig, load_webmail_config, regenerate_webmail_path, save_webmail_config,
    webmail_admin_path, webmail_health_hint, webmail_label, webmail_open_path, webmail_ready,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn mailbox_addresses_json(selected: &str) -> String {
    let accounts = list_accounts_public();
    let mut addrs: Vec<String> = accounts.into_iter().map(|a| a.address).collect();
    let selected = selected.trim();
    if !selected.is_empty() && !addrs.iter().any(|a| a.eq_ignore_ascii_case(selected)) {
        addrs.insert(0, selected.to_string());
    }
    addrs.sort();
    addrs.dedup();
    let json = serde_json::to_string(&addrs).unwrap_or_else(|_| "[]".into());
    // Keep JSON intact inside <script type="application/json">; only break out of the tag.
    json.replace("</", "<\\/")
}

fn mailbox_picker_script() -> &'static str {
    r#"<script>
(function () {
  var input = document.getElementById('auto_login_search');
  var hidden = document.getElementById('auto_login_account');
  var list = document.getElementById('auto_login_results');
  var dataEl = document.getElementById('cpn-mbox-data');
  var clearBtn = document.getElementById('auto_login_clear');
  if (!input || !hidden || !dataEl) return;
  var accounts = [];
  try { accounts = JSON.parse(dataEl.textContent || '[]'); } catch (e) { accounts = []; }
  function render(filter) {
    if (!list) return;
    var q = (filter || '').toLowerCase();
    var matches = accounts.filter(function (a) {
      return !q || String(a).toLowerCase().indexOf(q) !== -1;
    }).slice(0, 50);
    list.innerHTML = '';
    matches.forEach(function (addr) {
      var li = document.createElement('li');
      li.setAttribute('role', 'option');
      li.textContent = addr;
      li.tabIndex = 0;
      li.addEventListener('click', function () { pick(addr); });
      li.addEventListener('keydown', function (ev) {
        if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); pick(addr); }
      });
      list.appendChild(li);
    });
    list.hidden = matches.length === 0;
  }
  function pick(addr) {
    hidden.value = addr;
    input.value = addr;
    if (list) list.hidden = true;
  }
  function clearSel() {
    hidden.value = '';
    input.value = '';
    if (list) list.hidden = true;
  }
  input.addEventListener('focus', function () { render(input.value); });
  input.addEventListener('input', function () {
    hidden.value = '';
    render(input.value);
  });
  input.addEventListener('keydown', function (ev) {
    if (ev.key === 'Escape' && list) list.hidden = true;
  });
  if (clearBtn) clearBtn.addEventListener('click', function (ev) {
    ev.preventDefault();
    clearSel();
  });
  var form = input.closest('form');
  if (form) {
    form.addEventListener('submit', function () {
      if (hidden.value) return;
      var typed = (input.value || '').trim().toLowerCase();
      var hit = accounts.find(function (a) { return String(a).toLowerCase() === typed; });
      if (hit) hidden.value = hit;
    });
  }
  document.addEventListener('click', function (ev) {
    if (!list || list.hidden) return;
    if (ev.target === input || list.contains(ev.target)) return;
    list.hidden = true;
  });
})();
</script>
<style>
.cpn-mbox-combo { position: relative; }
.cpn-mbox-combo .cpn-mbox-row { display: flex; gap: 8px; align-items: center; }
.cpn-mbox-combo .cpn-mbox-row input { flex: 1; }
.cpn-mbox-results {
  list-style: none; margin: 4px 0 0; padding: 0; max-height: 220px; overflow: auto;
  border: 1px solid var(--hairline, #d0d5dd); border-radius: 10px; background: #fff;
  position: absolute; left: 0; right: 0; z-index: 20;
}
.cpn-mbox-results li { padding: 8px 12px; cursor: pointer; font-weight: 500; }
.cpn-mbox-results li:hover, .cpn-mbox-results li:focus { background: #f2f4f7; outline: none; }
[data-color-mode="dark"] .cpn-mbox-results { background: #1f242e; border-color: #3a4150; }
[data-color-mode="dark"] .cpn-mbox-results li:hover,
[data-color-mode="dark"] .cpn-mbox-results li:focus { background: #2a3140; }
</style>"#
}

pub fn email_webmail_page(notice: Option<&str>, error: Option<&str>) -> String {
    let _ = crate::install_webmail_runtime::heal_webmail_loopback_config();
    let notice_html = flash("ok", notice);
    let error_html = flash("error", error);
    let body = if webmail_ready() {
        let cfg = load_webmail_config();
        let label = webmail_label();
        let open = webmail_open_path().unwrap_or_else(|| format!("{}/", cfg.public_path));
        let admin = webmail_admin_path().unwrap_or_else(|| format!("{}/?admin", cfg.public_path));
        let embed_checked = if cfg.internal_embed { " checked" } else { "" };
        let mailbox_json = mailbox_addresses_json(&cfg.auto_login_account);
        let empty_hint = if mailbox_json == "[]" {
            r#"<p class="muted">No mailboxes yet. Create one under Email &gt; Accounts, then return here.</p>"#
        } else {
            ""
        };
        let iframe = if cfg.internal_embed {
            format!(
                r#"<div class="webmail-embed" style="margin-top:16px;">
          <p class="muted">Internal webmail (same origin). <a href="{open}" target="_blank" rel="noopener noreferrer">Open in new tab</a></p>
          <iframe title="{label}" src="{open}" style="width:100%;min-height:70vh;border:1px solid var(--border, #334);border-radius:8px;background:#111;"></iframe>
        </div>"#,
                open = html_escape(&open),
                label = html_escape(label),
            )
        } else {
            String::new()
        };
        format!(
            r#"{notice}{error}
        <p><strong>{label}</strong> is installed. Open the real webmail UI (not only the plugin dashboard).</p>
        <p style="display:flex;flex-wrap:wrap;gap:10px;margin:16px 0;">
          <a class="btn-primary" href="{open}" target="_blank" rel="noopener noreferrer">Open {label}</a>
          <a class="btn-secondary" href="{admin}" target="_blank" rel="noopener noreferrer">{label} Admin</a>
          <a class="btn-secondary" href="/email/webmail/app">Internal view</a>
        </p>
        <p class="muted">Public path: <code>{path}</code> (proxied through this panel to PHP-FPM on {health}). Mailbox data under <code>/var/lib/cpn-webmail</code> is preserved when you regenerate the path.</p>
        <form method="post" action="/email/webmail/settings" class="stack-form" style="max-width:520px;margin-top:16px;">
          <label for="auto_login_search">Preferred open account</label>
          <div class="cpn-mbox-combo">
            <div class="cpn-mbox-row">
              <input id="auto_login_search" type="search" value="{auto}" autocomplete="off" placeholder="Search mailboxes" aria-autocomplete="list" aria-controls="auto_login_results">
              <button type="button" class="btn-secondary" id="auto_login_clear">Clear</button>
            </div>
            <ul id="auto_login_results" class="cpn-mbox-results" role="listbox" hidden></ul>
            <script type="application/json" id="cpn-mbox-data">{mbox_json}</script>
            {empty_hint}
          </div>
          <input type="hidden" id="auto_login_account" name="auto_login_account" value="{auto}">
          <p class="muted">Select a mailbox from Email Accounts. Open {label} prefills that address on the login form. True SSO would need a stored mailbox password (not kept in settings.json); password is still entered in webmail for now.</p>
          <label for="public_path">Webmail URL path</label>
          <input id="public_path" name="public_path" type="text" value="{path}" autocomplete="off">
          <label style="display:flex;align-items:center;gap:10px;font-weight:600;">
            <input type="checkbox" name="internal_embed" value="1"{embed}>
            Enable internal webmail embed on this page
          </label>
          <button type="submit" class="btn-primary">Save webmail settings</button>
        </form>
        {script}
        <form method="post" action="/email/webmail/regenerate-path" style="margin-top:12px;" onsubmit="return confirm('Regenerate the public webmail path? Bookmarks to the old URL will stop working. Mail data is kept.');">
          <button type="submit" class="btn-secondary">Regenerate webmail URL path</button>
        </form>
        {iframe}"#,
            notice = notice_html,
            error = error_html,
            label = html_escape(label),
            open = html_escape(&open),
            admin = html_escape(&admin),
            path = html_escape(&cfg.public_path),
            health = html_escape(webmail_health_hint()),
            auto = html_escape(&cfg.auto_login_account),
            embed = embed_checked,
            mbox_json = mailbox_json,
            empty_hint = empty_hint,
            script = mailbox_picker_script(),
            iframe = iframe,
        )
    } else {
        format!(
            r#"{notice}{error}{}"#,
            not_configured_body(
                "Webmail client is not installed yet.",
                "Install SnappyMail or Roundcube from the installer mail stage, or add the SnappyMail/Roundcube plugin for a site. When installed, Open Webmail appears here."
            ),
            notice = notice_html,
            error = error_html,
        )
    };
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Webmail", None),
        ],
        "Webmail",
        "Open SnappyMail or Roundcube.",
        &body,
        None,
        None,
    )
}

pub fn email_webmail_app_page() -> String {
    if !webmail_ready() {
        return email_webmail_page(
            None,
            Some("Install SnappyMail or Roundcube before using internal webmail."),
        );
    }
    let cfg = load_webmail_config();
    if !cfg.internal_embed {
        return email_webmail_page(
            None,
            Some("Enable internal webmail embed under Email > Webmail settings first."),
        );
    }
    let open = webmail_open_path().unwrap_or_else(|| format!("{}/", cfg.public_path));
    let label = webmail_label();
    let body = format!(
        r#"<p style="display:flex;flex-wrap:wrap;gap:10px;">
        <a class="btn-primary" href="{open}" target="_blank" rel="noopener noreferrer">Open in new tab</a>
        <a class="btn-secondary" href="/email/webmail">Webmail settings</a>
      </p>
      <iframe title="{label}" src="{open}" style="width:100%;min-height:75vh;border:1px solid var(--border, #334);border-radius:8px;background:#111;"></iframe>"#,
        open = html_escape(&open),
        label = html_escape(label),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Email", Some("/email")),
            ("Webmail", Some("/email/webmail")),
            ("Internal", None),
        ],
        &format!("Internal {label}"),
        "Embedded webmail inside CPN.",
        &body,
        None,
        None,
    )
}

pub fn apply_webmail_settings_form(
    auto_login_account: &str,
    public_path: &str,
    internal_embed: bool,
) -> Result<String, String> {
    let mut cfg = load_webmail_config();
    let selected = auto_login_account.trim().to_ascii_lowercase();
    if !selected.is_empty() {
        let known = list_accounts_public();
        let ok = known
            .iter()
            .any(|a| a.address.eq_ignore_ascii_case(&selected));
        if !ok {
            return Err(
                "Preferred open account must be an existing Email Accounts mailbox, or clear the selection."
                    .into(),
            );
        }
    }
    cfg.auto_login_account = selected;
    cfg.public_path = public_path.trim().to_string();
    cfg.internal_embed = internal_embed;
    save_webmail_config(&cfg)?;
    let _ = crate::install_webmail_runtime::heal_webmail_loopback_config();
    Ok("Webmail settings saved".into())
}

pub fn apply_regenerate_path() -> Result<String, String> {
    let cfg = regenerate_webmail_path()?;
    Ok(format!(
        "Webmail path moved to {}. Mail data was not deleted.",
        cfg.public_path
    ))
}

fn flash(kind: &str, message: Option<&str>) -> String {
    let Some(message) = message.filter(|v| !v.is_empty()) else {
        return String::new();
    };
    let class = if kind == "error" {
        "panel-notice error"
    } else {
        "panel-notice ok"
    };
    format!(
        r#"<p class="{class}" role="status">{}</p>"#,
        html_escape(message)
    )
}

#[allow(dead_code)]
pub fn default_config_for_tests() -> WebmailPanelConfig {
    WebmailPanelConfig::default()
}
