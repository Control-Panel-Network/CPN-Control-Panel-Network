//! FTP and Dropbox backup destination forms (unlocked with backupDestinations).

use crate::panel_hubs::feature_shell;
use crate::panel_ops_backup_extra::{
    DropboxDest, FtpDest, load_destinations, save_destinations, set_dropbox_secrets,
    set_ftp_password, test_dropbox, test_ftp,
};

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn checked(on: bool) -> &'static str {
    if on { " checked" } else { "" }
}

pub fn backups_ftp_page(notice: Option<&str>, error: Option<&str>) -> String {
    let d = load_destinations();
    let f = &d.ftp;
    let pw = if f.password_set { "saved" } else { "not set" };
    let body = format!(
        r#"<form method="post" action="/backups/ftp/save" class="stack-form" style="max-width:640px;display:grid;gap:10px;">
      <label><input type="checkbox" class="cpn-switch" role="switch" name="enabled" value="1"{en}> Enable FTP destination</label>
      <label>Host <input type="text" name="host" value="{host}" maxlength="200" autocomplete="off" style="width:100%;"></label>
      <label>Port <input type="number" name="port" value="{port}" min="1" max="65535" style="width:100%;"></label>
      <label>Username <input type="text" name="username" value="{user}" maxlength="120" autocomplete="off" style="width:100%;"></label>
      <label>Password <span class="muted">({pw}; leave blank to keep)</span>
        <input type="password" name="password" maxlength="400" autocomplete="new-password" style="width:100%;">
      </label>
      <label>Remote path <input type="text" name="remote_path" value="{path}" maxlength="400" style="width:100%;"></label>
      <label><input type="checkbox" name="passive" value="1"{passive}> Passive mode</label>
      <label>Operator note <input type="text" name="note" value="{note}" maxlength="200" style="width:100%;"></label>
      <button type="submit" class="btn-primary">Save FTP</button>
    </form>
    <form method="post" action="/backups/ftp/test" style="margin-top:12px;">
      <button type="submit" class="btn-secondary">Test connection</button>
    </form>
    <p class="muted" style="margin-top:14px;"><a href="/backups/destinations">← Destinations</a></p>"#,
        en = checked(f.enabled),
        host = html_escape(&f.host),
        port = if f.port == 0 { 21 } else { f.port },
        user = html_escape(&f.username),
        pw = pw,
        path = html_escape(&f.remote_path),
        passive = checked(f.passive),
        note = html_escape(&f.note),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("FTP", None),
        ],
        "FTP",
        "FTP backup destination for scheduled archive upload.",
        &body,
        notice,
        error,
    )
}

pub fn backups_dropbox_page(notice: Option<&str>, error: Option<&str>) -> String {
    let d = load_destinations();
    let x = &d.dropbox;
    let secret = if x.app_secret_set { "saved" } else { "not set" };
    let token = if x.refresh_token_set {
        "saved"
    } else {
        "not set"
    };
    let body = format!(
        r#"<form method="post" action="/backups/dropbox/save" class="stack-form" style="max-width:640px;display:grid;gap:10px;">
      <label><input type="checkbox" class="cpn-switch" role="switch" name="enabled" value="1"{en}> Enable Dropbox destination</label>
      <label>App key <input type="text" name="app_key" value="{key}" maxlength="200" autocomplete="off" style="width:100%;"></label>
      <label>App secret <span class="muted">({secret}; leave blank to keep)</span>
        <input type="password" name="app_secret" maxlength="400" autocomplete="new-password" style="width:100%;">
      </label>
      <label>Refresh token <span class="muted">({token}; leave blank to keep)</span>
        <input type="password" name="refresh_token" maxlength="800" autocomplete="new-password" style="width:100%;">
      </label>
      <label>Folder path <span class="muted">(optional, for example /CPN-Backups)</span>
        <input type="text" name="folder_path" value="{folder}" maxlength="300" autocomplete="off" style="width:100%;">
      </label>
      <label>Operator note <input type="text" name="note" value="{note}" maxlength="200" style="width:100%;"></label>
      <button type="submit" class="btn-primary">Save Dropbox</button>
    </form>
    <form method="post" action="/backups/dropbox/test" style="margin-top:12px;">
      <button type="submit" class="btn-secondary">Test connection</button>
    </form>
    <p class="muted" style="margin-top:14px;"><a href="/backups/destinations">← Destinations</a></p>"#,
        en = checked(x.enabled),
        key = html_escape(&x.app_key),
        secret = secret,
        token = token,
        folder = html_escape(&x.folder_path),
        note = html_escape(&x.note),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Dropbox", None),
        ],
        "Dropbox",
        "Dropbox (Disk) backup destination via OAuth refresh token.",
        &body,
        notice,
        error,
    )
}

/// Form fields for saving an FTP backup destination.
pub struct FtpSaveInput<'a> {
    pub enabled: bool,
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub password: &'a str,
    pub remote_path: &'a str,
    pub passive: bool,
    pub note: &'a str,
}

pub fn save_ftp_form(input: FtpSaveInput<'_>) -> Result<String, String> {
    set_ftp_password(input.password)?;
    let mut d = load_destinations();
    d.ftp = FtpDest {
        enabled: input.enabled,
        host: input.host.trim().to_string(),
        port: if input.port == 0 { 21 } else { input.port },
        username: input.username.trim().to_string(),
        password_set: false,
        remote_path: input.remote_path.trim().to_string(),
        passive: input.passive,
        note: input.note.trim().to_string(),
    };
    save_destinations(&d)?;
    Ok("FTP destination saved.".into())
}

pub fn save_dropbox_form(
    enabled: bool,
    app_key: &str,
    app_secret: &str,
    refresh_token: &str,
    folder_path: &str,
    note: &str,
) -> Result<String, String> {
    set_dropbox_secrets(app_secret, refresh_token)?;
    let mut d = load_destinations();
    d.dropbox = DropboxDest {
        enabled,
        app_key: app_key.trim().to_string(),
        app_secret_set: false,
        refresh_token_set: false,
        folder_path: folder_path.trim().to_string(),
        note: note.trim().to_string(),
    };
    save_destinations(&d)?;
    Ok("Dropbox destination saved.".into())
}

pub fn run_ftp_test() -> Result<String, String> {
    test_ftp()
}

pub fn run_dropbox_test() -> Result<String, String> {
    test_dropbox()
}
