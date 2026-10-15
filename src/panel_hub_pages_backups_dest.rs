//! LIVE backup destination forms (local, Google Drive, remote SFTP, S3-compatible).

use crate::panel_hubs::feature_shell;
use crate::panel_ops_backup_extra::{
    BackupDestinations, GoogleDriveDest, RemoteDest, S3Dest, load_destinations, save_destinations,
    set_gdrive_secrets, set_remote_password, set_s3_secrets, test_google_drive, test_remote,
    test_s3,
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

pub fn backups_destinations_page(notice: Option<&str>, error: Option<&str>) -> String {
    let d = load_destinations();
    let body = format!(
        r#"<p class="muted" style="margin:0 0 14px;">Secrets are stored under the CPN data directory (mode 600) and never written into destination JSON or logs. <a href="/backups/google-drive">Google Drive</a> · <a href="/backups/remote">Remote SFTP</a> · <a href="/backups/s3">S3-compatible</a></p>
    <form method="post" action="/backups/destinations/save" class="stack-form" style="max-width:640px;display:grid;gap:12px;">
      <label><input type="checkbox" name="local_enabled" value="1"{local}> Local archives enabled</label>
      <label><input type="checkbox" name="gdrive_enabled" value="1"{gdrive}> Google Drive enabled</label>
      <label><input type="checkbox" name="remote_enabled" value="1"{remote}> Remote SFTP/SCP enabled</label>
      <label><input type="checkbox" name="s3_enabled" value="1"{s3}> S3-compatible enabled</label>
      <button type="submit" class="btn-primary">Save destination toggles</button>
    </form>"#,
        local = checked(d.local_enabled),
        gdrive = checked(d.google_drive.enabled),
        remote = checked(d.remote.enabled),
        s3 = checked(d.s3.enabled),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Destinations", None),
        ],
        "Destinations",
        "Local, Google Drive, remote SFTP, and S3-compatible backup targets.",
        &body,
        notice,
        error,
    )
}

pub fn backups_gdrive_page(notice: Option<&str>, error: Option<&str>) -> String {
    let d = load_destinations();
    let g = &d.google_drive;
    let secret_hint = if g.client_secret_set {
        "saved"
    } else {
        "not set"
    };
    let token_hint = if g.refresh_token_set {
        "saved"
    } else {
        "not set"
    };
    let body = format!(
        r#"<form method="post" action="/backups/google-drive/save" class="stack-form" style="max-width:640px;display:grid;gap:10px;">
      <label><input type="checkbox" name="enabled" value="1"{en}> Enable Google Drive destination</label>
      <label>OAuth client ID
        <input type="text" name="client_id" value="{cid}" maxlength="200" autocomplete="off" style="width:100%;">
      </label>
      <label>OAuth client secret <span class="muted">({secret_hint}; leave blank to keep)</span>
        <input type="password" name="client_secret" maxlength="400" autocomplete="new-password" style="width:100%;">
      </label>
      <label>Refresh token <span class="muted">({token_hint}; leave blank to keep)</span>
        <input type="password" name="refresh_token" maxlength="800" autocomplete="new-password" style="width:100%;">
      </label>
      <label>Drive folder ID <span class="muted">(optional)</span>
        <input type="text" name="folder_id" value="{folder}" maxlength="120" autocomplete="off" style="width:100%;">
      </label>
      <label>Operator note
        <input type="text" name="note" value="{note}" maxlength="200" style="width:100%;">
      </label>
      <div style="display:flex;flex-wrap:wrap;gap:8px;">
        <button type="submit" class="btn-primary">Save Google Drive</button>
      </div>
    </form>
    <form method="post" action="/backups/google-drive/test" style="margin-top:12px;">
      <button type="submit" class="btn-secondary">Test connection</button>
    </form>
    <p class="muted" style="margin-top:14px;"><a href="/backups/destinations">← Destinations</a></p>"#,
        en = checked(g.enabled),
        cid = html_escape(&g.client_id),
        secret_hint = secret_hint,
        token_hint = token_hint,
        folder = html_escape(&g.folder_id),
        note = html_escape(&g.note),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Google Drive", None),
        ],
        "Google Drive",
        "OAuth credentials for scheduled backup upload.",
        &body,
        notice,
        error,
    )
}

pub fn backups_remote_page(notice: Option<&str>, error: Option<&str>) -> String {
    let d = load_destinations();
    let r = &d.remote;
    let pass_hint = if r.password_set { "saved" } else { "not set" };
    let body = format!(
        r#"<form method="post" action="/backups/remote/save" class="stack-form" style="max-width:640px;display:grid;gap:10px;">
      <label><input type="checkbox" name="enabled" value="1"{en}> Enable remote SFTP/SCP</label>
      <label>Protocol
        <select name="protocol" style="width:100%;max-width:240px;">
          <option value="sftp"{sftp}>SFTP</option>
          <option value="scp"{scp}>SCP</option>
        </select>
      </label>
      <label>Host
        <input type="text" name="host" value="{host}" maxlength="200" required style="width:100%;">
      </label>
      <label>Port
        <input type="number" name="port" value="{port}" min="1" max="65535" style="width:120px;">
      </label>
      <label>Username
        <input type="text" name="username" value="{user}" maxlength="120" required style="width:100%;">
      </label>
      <label>Auth mode
        <select name="auth_mode" style="width:100%;max-width:240px;">
          <option value="key"{key}>SSH key</option>
          <option value="password"{pw}>Password</option>
        </select>
      </label>
      <label>SSH private key path <span class="muted">(when using key auth)</span>
        <input type="text" name="key_path" value="{keypath}" maxlength="300" style="width:100%;">
      </label>
      <label>Password <span class="muted">({pass_hint}; leave blank to keep)</span>
        <input type="password" name="password" maxlength="400" autocomplete="new-password" style="width:100%;">
      </label>
      <label>Remote path
        <input type="text" name="remote_path" value="{rpath}" maxlength="300" required style="width:100%;">
      </label>
      <label>Operator note
        <input type="text" name="note" value="{note}" maxlength="200" style="width:100%;">
      </label>
      <button type="submit" class="btn-primary">Save remote destination</button>
    </form>
    <form method="post" action="/backups/remote/test" style="margin-top:12px;">
      <button type="submit" class="btn-secondary">Test connection</button>
    </form>
    <p class="muted" style="margin-top:14px;"><a href="/backups/destinations">← Destinations</a></p>"#,
        en = checked(r.enabled),
        sftp = if r.protocol.eq_ignore_ascii_case("scp") {
            ""
        } else {
            " selected"
        },
        scp = if r.protocol.eq_ignore_ascii_case("scp") {
            " selected"
        } else {
            ""
        },
        host = html_escape(&r.host),
        port = if r.port == 0 { 22 } else { r.port },
        user = html_escape(&r.username),
        key = if r.auth_mode.eq_ignore_ascii_case("password") {
            ""
        } else {
            " selected"
        },
        pw = if r.auth_mode.eq_ignore_ascii_case("password") {
            " selected"
        } else {
            ""
        },
        keypath = html_escape(&r.key_path),
        pass_hint = pass_hint,
        rpath = html_escape(&r.remote_path),
        note = html_escape(&r.note),
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("Remote Backups", None),
        ],
        "Remote Backups",
        "SFTP/SCP transfer targets for scheduled backups.",
        &body,
        notice,
        error,
    )
}

pub fn backups_s3_page(notice: Option<&str>, error: Option<&str>) -> String {
    let d = load_destinations();
    let s = &d.s3;
    let ak = if s.access_key_set { "saved" } else { "not set" };
    let sk = if s.secret_key_set { "saved" } else { "not set" };
    let body = format!(
        r#"<form method="post" action="/backups/s3/save" class="stack-form" style="max-width:640px;display:grid;gap:10px;">
      <label><input type="checkbox" name="enabled" value="1"{en}> Enable S3-compatible destination</label>
      <label>Endpoint URL <span class="muted">(Wasabi, Cloudflare R2, MinIO, AWS)</span>
        <input type="text" name="endpoint" value="{endpoint}" maxlength="300" required style="width:100%;" placeholder="https://s3.eu-central-1.wasabisys.com">
      </label>
      <label>Region
        <input type="text" name="region" value="{region}" maxlength="64" style="width:100%;" placeholder="auto or eu-central-1">
      </label>
      <label>Bucket
        <input type="text" name="bucket" value="{bucket}" maxlength="120" required style="width:100%;">
      </label>
      <label>Prefix <span class="muted">(optional folder inside the bucket)</span>
        <input type="text" name="prefix" value="{prefix}" maxlength="200" style="width:100%;">
      </label>
      <label>Access key <span class="muted">({ak}; leave blank to keep)</span>
        <input type="password" name="access_key" maxlength="200" autocomplete="new-password" style="width:100%;">
      </label>
      <label>Secret key <span class="muted">({sk}; leave blank to keep)</span>
        <input type="password" name="secret_key" maxlength="400" autocomplete="new-password" style="width:100%;">
      </label>
      <button type="submit" class="btn-primary">Save S3 destination</button>
    </form>
    <form method="post" action="/backups/s3/test" style="margin-top:12px;">
      <button type="submit" class="btn-secondary">Test connection</button>
    </form>
    <p class="muted" style="margin-top:14px;"><a href="/backups/destinations">← Destinations</a></p>"#,
        en = checked(s.enabled),
        endpoint = html_escape(&s.endpoint),
        region = html_escape(&s.region),
        bucket = html_escape(&s.bucket),
        prefix = html_escape(&s.prefix),
        ak = ak,
        sk = sk,
    );
    feature_shell(
        &[
            ("Dashboard", Some("/dashboard")),
            ("Backups", Some("/backups")),
            ("S3", None),
        ],
        "S3-compatible",
        "Wasabi, R2, and other S3-compatible backup destinations.",
        &body,
        notice,
        error,
    )
}

fn flag(v: &str) -> bool {
    matches!(v.trim(), "1" | "true" | "on" | "yes")
}

pub fn save_backup_destinations_toggles(
    local_enabled: bool,
    gdrive_enabled: bool,
    remote_enabled: bool,
    s3_enabled: bool,
) -> Result<String, String> {
    let mut d = load_destinations();
    d.local_enabled = local_enabled;
    d.google_drive.enabled = gdrive_enabled;
    d.remote.enabled = remote_enabled;
    d.s3.enabled = s3_enabled;
    save_destinations(&d)?;
    Ok("Destination toggles saved.".into())
}

pub fn save_gdrive_form(
    enabled: bool,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    folder_id: &str,
    note: &str,
) -> Result<String, String> {
    set_gdrive_secrets(client_secret, refresh_token)?;
    let mut d = load_destinations();
    d.google_drive = GoogleDriveDest {
        enabled,
        client_id: client_id.trim().to_string(),
        client_secret_set: false,
        refresh_token_set: false,
        folder_id: folder_id.trim().to_string(),
        note: note.trim().to_string(),
    };
    save_destinations(&d)?;
    Ok("Google Drive destination saved.".into())
}

/// Form fields for saving a remote SFTP/SCP backup destination.
pub struct RemoteSaveInput<'a> {
    pub enabled: bool,
    pub protocol: &'a str,
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub auth_mode: &'a str,
    pub key_path: &'a str,
    pub password: &'a str,
    pub remote_path: &'a str,
    pub note: &'a str,
}

pub fn save_remote_form(input: RemoteSaveInput<'_>) -> Result<String, String> {
    set_remote_password(input.password)?;
    let mut d = load_destinations();
    d.remote = RemoteDest {
        enabled: input.enabled,
        protocol: input.protocol.trim().to_ascii_lowercase(),
        host: input.host.trim().to_string(),
        port: if input.port == 0 { 22 } else { input.port },
        username: input.username.trim().to_string(),
        auth_mode: input.auth_mode.trim().to_ascii_lowercase(),
        password_set: false,
        key_path: input.key_path.trim().to_string(),
        remote_path: input.remote_path.trim().to_string(),
        note: input.note.trim().to_string(),
    };
    save_destinations(&d)?;
    Ok("Remote destination saved.".into())
}

pub fn save_s3_form(
    enabled: bool,
    endpoint: &str,
    region: &str,
    bucket: &str,
    prefix: &str,
    access_key: &str,
    secret_key: &str,
) -> Result<String, String> {
    set_s3_secrets(access_key, secret_key)?;
    let mut d = load_destinations();
    d.s3 = S3Dest {
        enabled,
        endpoint: endpoint.trim().to_string(),
        region: region.trim().to_string(),
        bucket: bucket.trim().to_string(),
        prefix: prefix.trim().to_string(),
        access_key_set: false,
        secret_key_set: false,
    };
    save_destinations(&d)?;
    Ok("S3-compatible destination saved.".into())
}

pub fn run_gdrive_test() -> Result<String, String> {
    test_google_drive()
}

pub fn run_remote_test() -> Result<String, String> {
    test_remote()
}

pub fn run_s3_test() -> Result<String, String> {
    test_s3()
}

pub fn parse_flag(v: &str) -> bool {
    flag(v)
}

/// Keep type name available for callers that still import BackupDestinations via pages.
pub type DestinationsSnapshot = BackupDestinations;
