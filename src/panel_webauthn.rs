//! WebAuthn relying-party helpers and short-lived ceremony state.
//! RP ID follows the request Host (works for `127.0.0.1` lab and production FQDNs).
//!
//! Registration uses one presence-only security-key ceremony (no CredProtect
//! UV-required extension) so Chrome/Edge accept create options for both Windows
//! Hello and FIDO2 security keys. The browser OS picker chooses the authenticator;
//! the UI exposes a single **Register passkey** button. Optional `kind` on
//! register/start is accepted for compatibility: `security-key` forces a
//! cross-platform attachment hint (silent client retry); everything else stays
//! flexible (no attachment).
//!
//! Credentials are stored as Passkeys either way.

use crate::account::{data_dir, now_unix};
use crate::account_passkeys::{
    add_passkey, all_passkeys_for_auth, exclude_credential_ids, passkey_owner, passkeys_for_auth,
    update_passkey_after_auth,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::{
    AuthenticatorAttachment, CreationChallengeResponse, Credential, DiscoverableAuthentication,
    DiscoverableKey, Passkey, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse, SecurityKey, SecurityKeyAuthentication, SecurityKeyRegistration,
    Webauthn, WebauthnBuilder,
};

pub use crate::panel_webauthn_client::passkey_client_script;

const CEREMONY_TTL_SECS: u64 = 300;
const MAX_HOST_LEN: usize = 253;

/// Optional attachment hint for register/start (UI stays one button).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAuthenticatorKind {
    /// No attachment: platform (Windows Hello) and roaming keys both OK.
    Flexible,
    /// Prefer a roaming FIDO2 / YubiKey (silent client retry path).
    SecurityKey,
}

impl RegisterAuthenticatorKind {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "security-key" | "security_key" | "cross-platform" | "yubikey" | "roaming" => {
                Self::SecurityKey
            }
            // platform / hello / empty / unknown: one flexible ceremony.
            _ => Self::Flexible,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flexible => "flexible",
            Self::SecurityKey => "security-key",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CeremonyKind {
    RegisterSecurityKey(SecurityKeyRegistration),
    Authenticate { state: SecurityKeyAuthentication },
    Discoverable(DiscoverableAuthentication),
}

fn passkey_to_security_key(passkey: &Passkey) -> SecurityKey {
    SecurityKey::from(Credential::from(passkey.clone()))
}

fn security_key_to_passkey(key: SecurityKey) -> Passkey {
    Passkey::from(Credential::from(key))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CeremonyRecord {
    id: String,
    username: String,
    created_at_unix: u64,
    kind: CeremonyKind,
}

fn ceremonies_dir() -> PathBuf {
    data_dir().join("passkeys").join("ceremonies")
}

fn ceremony_path(id: &str) -> PathBuf {
    ceremonies_dir().join(format!("{id}.json"))
}

/// Remove pending WebAuthn ceremony files (login/register challenges). Returns count removed.
pub fn clear_all_ceremonies() -> Result<usize, String> {
    let dir = ceremonies_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(0);
    };
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => removed = removed.saturating_add(1),
            Err(err) => {
                return Err(format!(
                    "Could not remove ceremony {}: {err}",
                    path.display()
                ));
            }
        }
    }
    Ok(removed)
}

fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create {}: {err}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|err| format!("Could not write {}: {err}", path.display()))?;
    file.write_all(bytes)
        .map_err(|err| format!("Could not save {}: {err}", path.display()))?;
    Ok(())
}

fn new_ceremony_id() -> String {
    let bytes: [u8; 16] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn save_ceremony(record: &CeremonyRecord) -> Result<(), String> {
    let json = serde_json::to_string(record)
        .map_err(|err| format!("Could not serialize ceremony: {err}"))?;
    write_secret_file(&ceremony_path(&record.id), json.as_bytes())
}

fn take_ceremony(id: &str) -> Result<CeremonyRecord, String> {
    let path = ceremony_path(id);
    let raw =
        fs::read_to_string(&path).map_err(|_| "Passkey ceremony expired or missing".to_string())?;
    let _ = fs::remove_file(&path);
    let record: CeremonyRecord =
        serde_json::from_str(&raw).map_err(|_| "Invalid passkey ceremony".to_string())?;
    if now_unix().saturating_sub(record.created_at_unix) > CEREMONY_TTL_SECS {
        return Err("Passkey ceremony expired; try again".into());
    }
    Ok(record)
}

/// Stable UUID for a panel username (v5-like from SHA-256).
pub fn user_uuid(username: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"cpn-passkey-user|");
    hasher.update(username.to_ascii_lowercase().as_bytes());
    let dig = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&dig[..16]);
    // Set UUID version/variant bits for a valid UUID shape.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn sanitize_host(raw: &str) -> Result<String, String> {
    let host = raw.trim();
    if host.is_empty() || host.len() > MAX_HOST_LEN {
        return Err("Invalid Host header".into());
    }
    if host
        .chars()
        .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
    {
        return Err("Invalid Host header".into());
    }
    Ok(host.to_string())
}

fn split_host_port(host: &str) -> (String, Option<String>) {
    // Bracketed IPv6: [::1]:2087
    if let Some(rest) = host.strip_prefix('[') {
        if let Some((addr, port)) = rest.split_once("]:") {
            return (addr.to_string(), Some(port.to_string()));
        }
        if let Some(addr) = rest.strip_suffix(']') {
            return (addr.to_string(), None);
        }
    }
    // IPv4 or hostname with optional port (last ':' for host:port).
    if let Some((name, port)) = host.rsplit_once(':')
        && !name.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
    {
        return (name.to_string(), Some(port.to_string()));
    }
    (host.to_string(), None)
}

fn is_loopback_host(host_no_port: &str) -> bool {
    matches!(
        host_no_port.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1" | "0:0:0:0:0:0:0:1"
    )
}

fn origin_url(scheme: &str, host_no_port: &str, port: Option<&str>) -> Result<Url, String> {
    let host = match port {
        Some(p) if !p.is_empty() => format!("{host_no_port}:{p}"),
        _ => host_no_port.to_string(),
    };
    Url::parse(&format!("{scheme}://{host}")).map_err(|err| format!("Invalid origin URL: {err}"))
}

/// Build a Webauthn RP for this HTTP request (origin + RP ID without port).
///
/// `webauthn-rs` rejects raw IP RP IDs because `url::Url::domain()` is `None`
/// for IP literals (builder error: "The configuration was invalid"). Loopback
/// Hosts (`127.0.0.1`, `::1`, `localhost`) therefore use RP ID `localhost`, and
/// both `http://localhost:PORT` and `http://127.0.0.1:PORT` are allowed origins
/// so lab access via either URL can verify ceremonies. Browsers still require the
/// page host to match the RP ID for `credentials.get/create`, so the client JS
/// redirects `127.0.0.1` to `localhost` before the ceremony. Prefer `localhost` in
/// the browser; API begin-register works for either Host header. When
/// `panel_public_url` is set (NAT labs), that origin is also allowed.
pub fn webauthn_for_request(
    host_header: Option<&str>,
    https: bool,
) -> Result<(Webauthn, String), String> {
    let host = sanitize_host(host_header.unwrap_or("127.0.0.1"))?;
    let (host_no_port, port) = split_host_port(&host);
    if host_no_port.is_empty() {
        return Err("Could not derive WebAuthn RP ID".into());
    }
    if host_no_port.parse::<std::net::IpAddr>().is_ok() && !is_loopback_host(&host_no_port) {
        return Err(
            "Passkeys require a hostname. Use a domain name or localhost (not a public IP).".into(),
        );
    }
    let scheme = if https { "https" } else { "http" };
    let port_ref = port.as_deref();
    let loopback = is_loopback_host(&host_no_port);
    let rp_id = if loopback {
        "localhost".to_string()
    } else {
        host_no_port.clone()
    };
    // Primary origin must have a domain() for WebauthnBuilder::new.
    let primary = origin_url(scheme, &rp_id, port_ref)?;
    let mut builder = WebauthnBuilder::new(&rp_id, &primary)
        .map_err(|err| format!("WebAuthn builder error: {err}"))?
        .rp_name("CPN Panel")
        .allow_any_port(true)
        // Avoid CredProtect UV-required + UV-preferred incongruence that Chrome/Edge
        // reject with NotSupportedError during security-key registration.
        .danger_set_user_presence_only_security_keys(true);
    if loopback {
        // Accept either loopback spelling used by the lab browser/API client.
        for alt_host in ["localhost", "127.0.0.1", "[::1]"] {
            if let Ok(alt) = origin_url(scheme, alt_host, port_ref)
                && alt != primary
            {
                builder = builder.append_allowed_origin(&alt);
            }
        }
    }
    // NAT / reverse-proxy labs: also allow the configured public panel URL origin.
    if let Some(public) = crate::panel_public_url::load_panel_public_url()
        && let Ok(public_url) = Url::parse(&public)
    {
        builder = builder.append_allowed_origin(&public_url);
        if let Some(host_str) = public_url.host_str()
            && is_loopback_host(host_str)
        {
            let pub_port = public_url.port().map(|p| p.to_string());
            let pub_scheme = public_url.scheme();
            for alt_host in ["localhost", "127.0.0.1", "[::1]"] {
                if let Ok(alt) = origin_url(pub_scheme, alt_host, pub_port.as_deref()) {
                    builder = builder.append_allowed_origin(&alt);
                }
            }
        }
    }
    let webauthn = builder
        .build()
        .map_err(|err| format!("WebAuthn build error: {err}"))?;
    Ok((webauthn, rp_id))
}

pub fn start_registration(
    webauthn: &Webauthn,
    username: &str,
    kind: RegisterAuthenticatorKind,
) -> Result<(String, CreationChallengeResponse), String> {
    let exclude = exclude_credential_ids(username);
    let exclude = if exclude.is_empty() {
        None
    } else {
        Some(exclude)
    };
    // Presence-only: no CredProtect UV-required, UV discouraged. Do not use
    // start_passkey_registration (UV required + CredProtect) which pushes Edge
    // toward Microsoft Password Manager hybrid save on localhost labs.
    let attachment = match kind {
        RegisterAuthenticatorKind::Flexible => None,
        RegisterAuthenticatorKind::SecurityKey => Some(AuthenticatorAttachment::CrossPlatform),
    };
    let (ccr, state) = webauthn
        .start_securitykey_registration(
            user_uuid(username),
            username,
            username,
            exclude,
            None,
            attachment,
        )
        .map_err(|err| format!("Could not start passkey registration: {err}"))?;
    let id = new_ceremony_id();
    save_ceremony(&CeremonyRecord {
        id: id.clone(),
        username: username.to_string(),
        created_at_unix: now_unix(),
        kind: CeremonyKind::RegisterSecurityKey(state),
    })?;
    Ok((id, ccr))
}

pub fn finish_registration(
    webauthn: &Webauthn,
    username: &str,
    ceremony_id: &str,
    label: &str,
    credential: &RegisterPublicKeyCredential,
) -> Result<(), String> {
    let record = take_ceremony(ceremony_id)?;
    if !record.username.eq_ignore_ascii_case(username) {
        return Err("Passkey ceremony user mismatch".into());
    }
    match record.kind {
        CeremonyKind::RegisterSecurityKey(state) => {
            let security_key = webauthn
                .finish_securitykey_registration(credential, &state)
                .map_err(|err| format!("Passkey registration failed: {err}"))?;
            add_passkey(username, label, security_key_to_passkey(security_key))?;
        }
        _ => return Err("Passkey ceremony type mismatch".into()),
    }
    Ok(())
}

pub fn start_authentication(
    webauthn: &Webauthn,
) -> Result<(String, RequestChallengeResponse), String> {
    let creds = all_passkeys_for_auth();
    let (rcr, kind) = if creds.is_empty() {
        let (rcr, state) = webauthn
            .start_discoverable_authentication()
            .map_err(|err| format!("Could not start passkey authentication: {err}"))?;
        (rcr, CeremonyKind::Discoverable(state))
    } else {
        let keys = creds
            .iter()
            .map(|(_, passkey)| passkey_to_security_key(passkey))
            .collect::<Vec<_>>();
        let (rcr, state) = webauthn
            .start_securitykey_authentication(&keys)
            .map_err(|err| format!("Could not start passkey authentication: {err}"))?;
        (rcr, CeremonyKind::Authenticate { state })
    };
    let id = new_ceremony_id();
    save_ceremony(&CeremonyRecord {
        id: id.clone(),
        username: String::new(),
        created_at_unix: now_unix(),
        kind,
    })?;
    Ok((id, rcr))
}

/// Start a passkey assertion limited to one account (2FA step after password).
pub fn start_authentication_for_user(
    webauthn: &Webauthn,
    username: &str,
) -> Result<(String, RequestChallengeResponse), String> {
    let keys = passkeys_for_auth(username);
    if keys.is_empty() {
        return Err("No passkey is registered for this account.".into());
    }
    let sk = keys.iter().map(passkey_to_security_key).collect::<Vec<_>>();
    let (rcr, state) = webauthn
        .start_securitykey_authentication(&sk)
        .map_err(|_| "Could not start passkey authentication.".to_string())?;
    let id = new_ceremony_id();
    save_ceremony(&CeremonyRecord {
        id: id.clone(),
        username: username.to_string(),
        created_at_unix: now_unix(),
        kind: CeremonyKind::Authenticate { state },
    })?;
    Ok((id, rcr))
}

pub fn finish_authentication(
    webauthn: &Webauthn,
    ceremony_id: &str,
    credential: &PublicKeyCredential,
) -> Result<String, String> {
    let record = take_ceremony(ceremony_id)?;
    let expected_user = record.username.clone();
    let (username, result) = match record.kind {
        CeremonyKind::Authenticate { state } => {
            let result = webauthn
                .finish_securitykey_authentication(credential, &state)
                .map_err(|_| {
                    "Incorrect passkey. Try again or use another sign-in method.".to_string()
                })?;
            let username = passkey_owner(result.cred_id())
                .ok_or_else(|| "That passkey is not registered for this panel.".to_string())?;
            (username, result)
        }
        CeremonyKind::Discoverable(state) => {
            let (_, credential_id) = webauthn
                .identify_discoverable_authentication(credential)
                .map_err(|_| {
                    "Incorrect passkey. Try again or use another sign-in method.".to_string()
                })?;
            let username = passkey_owner(credential_id)
                .ok_or_else(|| "That passkey is not registered for this panel.".to_string())?;
            let keys = passkeys_for_auth(&username)
                .iter()
                .map(DiscoverableKey::from)
                .collect::<Vec<_>>();
            let result = webauthn
                .finish_discoverable_authentication(credential, state, &keys)
                .map_err(|_| {
                    "Incorrect passkey. Try again or use another sign-in method.".to_string()
                })?;
            (username, result)
        }
        CeremonyKind::RegisterSecurityKey(_) => {
            return Err("Passkey ceremony type mismatch".into());
        }
    };
    if !expected_user.is_empty() && !expected_user.eq_ignore_ascii_case(&username) {
        return Err("That passkey belongs to a different account.".into());
    }
    // Persist counter / credential updates when the library reports a change.
    let mut creds = passkeys_for_auth(&username);
    if let Some(pk) = creds.iter_mut().find(|c| c.cred_id() == result.cred_id()) {
        let _ = pk.update_credential(&result);
        update_passkey_after_auth(&username, result.cred_id(), pk.clone())?;
    }
    Ok(username)
}

#[cfg(test)]
mod tests {
    use super::{
        RegisterAuthenticatorKind, start_authentication, start_registration, webauthn_for_request,
    };
    use crate::account::with_test_data_dir;

    #[test]
    fn loopback_ip_host_builds_webauthn() {
        let (wan, rp_id) = webauthn_for_request(Some("127.0.0.1:2087"), false)
            .expect("builder should accept loopback");
        assert_eq!(rp_id, "localhost");
        let origins = wan.get_allowed_origins();
        assert!(
            origins.iter().any(|u| u.as_str().contains("127.0.0.1")),
            "expected 127.0.0.1 origin among {origins:?}"
        );
        assert!(
            origins.iter().any(|u| u.as_str().contains("localhost")),
            "expected localhost origin among {origins:?}"
        );
    }

    #[test]
    fn nat_public_port_host_builds_webauthn() {
        let (wan, rp_id) = webauthn_for_request(Some("127.0.0.1:2091"), false)
            .expect("builder should accept NAT host port");
        assert_eq!(rp_id, "localhost");
        let origins = wan.get_allowed_origins();
        assert!(
            origins
                .iter()
                .any(|u| u.as_str().contains("127.0.0.1:2091") || u.as_str().contains("localhost")),
            "expected loopback origin for public port among {origins:?}"
        );
    }

    #[test]
    fn public_url_origin_appended_when_configured() {
        use crate::panel_public_url::save_panel_public_url;
        with_test_data_dir(|| {
            save_panel_public_url("http://127.0.0.1:2091").expect("save public url");
            let (wan, rp_id) =
                webauthn_for_request(Some("127.0.0.1:2087"), false).expect("webauthn");
            assert_eq!(rp_id, "localhost");
            let origins = wan.get_allowed_origins();
            assert!(
                origins.iter().any(|u| u.as_str().contains(":2091")),
                "panel_public_url origin should be allowed among {origins:?}"
            );
        });
    }

    #[test]
    fn localhost_host_builds_webauthn() {
        let (wan, rp_id) = webauthn_for_request(Some("localhost:2087"), false)
            .expect("builder should accept localhost");
        assert_eq!(rp_id, "localhost");
        assert!(!wan.get_allowed_origins().is_empty());
    }

    #[test]
    fn localhost_nat_2091_allows_browser_origin() {
        let (wan, rp_id) = webauthn_for_request(Some("localhost:2091"), false)
            .expect("builder should accept localhost NAT port");
        assert_eq!(rp_id, "localhost");
        let origins = wan.get_allowed_origins();
        assert!(
            origins
                .iter()
                .any(|u| u.as_str() == "http://localhost:2091/"
                    || u.as_str().contains("localhost:2091")),
            "expected http://localhost:2091 among {origins:?}"
        );
        assert!(
            origins
                .iter()
                .any(|u| u.as_str().contains("127.0.0.1:2091")),
            "expected 127.0.0.1:2091 sibling origin among {origins:?}"
        );
    }

    #[test]
    fn login_challenge_starts_without_registered_passkeys() {
        with_test_data_dir(|| {
            let (webauthn, _) =
                webauthn_for_request(Some("localhost:2087"), false).expect("webauthn");
            let (_, challenge) =
                start_authentication(&webauthn).expect("discoverable challenge should start");
            let json = serde_json::to_value(challenge).expect("challenge JSON");
            assert!(json.get("publicKey").is_some());
        });
    }

    fn assert_flexible_create_options(json: &serde_json::Value, expect_cross_platform: bool) {
        let pk = json
            .get("publicKey")
            .expect("publicKey")
            .as_object()
            .expect("object");
        let rp = pk.get("rp").expect("rp");
        assert_eq!(rp.get("id").and_then(|v| v.as_str()), Some("localhost"));
        let selection = pk
            .get("authenticatorSelection")
            .expect("authenticatorSelection");
        let uv = selection
            .get("userVerification")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_ne!(
            uv, "required",
            "labs must not require UV (YubiKey without PIN); got {selection}"
        );
        let attachment = selection
            .get("authenticatorAttachment")
            .and_then(|v| v.as_str());
        if expect_cross_platform {
            assert_eq!(
                attachment,
                Some("cross-platform"),
                "security-key retry should hint cross-platform; got {selection}"
            );
        } else {
            assert!(
                attachment.is_none(),
                "flexible ceremony must omit authenticatorAttachment; got {selection}"
            );
        }
        let extensions = pk.get("extensions").and_then(|v| v.as_object());
        let cred_protect = extensions
            .and_then(|e| e.get("credentialProtectionPolicy"))
            .and_then(|v| v.as_str());
        assert!(
            cred_protect.is_none() || cred_protect != Some("userVerificationRequired"),
            "CredProtect UV-required + non-required UV is rejected by Chrome as incongruent; got {extensions:?}"
        );
        let resident = selection
            .get("residentKey")
            .or_else(|| selection.get("requireResidentKey"));
        if let Some(rk) = resident {
            let required = rk.as_str() == Some("required") || rk.as_bool() == Some(true);
            assert!(
                !required,
                "discoverable/required residentKey often pushes Edge to Microsoft Password Manager; got {selection}"
            );
        }
    }

    #[test]
    fn registration_flexible_avoids_credprotect_and_attachment() {
        with_test_data_dir(|| {
            let (webauthn, rp_id) =
                webauthn_for_request(Some("localhost:2091"), false).expect("webauthn");
            assert_eq!(rp_id, "localhost");
            let (_, challenge) =
                start_registration(&webauthn, "cpnowner", RegisterAuthenticatorKind::Flexible)
                    .expect("flexible registration should start");
            let json = serde_json::to_value(challenge).expect("challenge JSON");
            assert_flexible_create_options(&json, false);
        });
    }

    #[test]
    fn registration_security_key_retry_hints_cross_platform() {
        with_test_data_dir(|| {
            let (webauthn, _) =
                webauthn_for_request(Some("localhost:2091"), false).expect("webauthn");
            let (_, challenge) = start_registration(
                &webauthn,
                "cpnowner",
                RegisterAuthenticatorKind::SecurityKey,
            )
            .expect("security-key registration should start");
            let json = serde_json::to_value(challenge).expect("challenge JSON");
            assert_flexible_create_options(&json, true);
        });
    }

    #[test]
    fn platform_kind_maps_to_flexible_ceremony() {
        assert_eq!(
            RegisterAuthenticatorKind::parse("platform"),
            RegisterAuthenticatorKind::Flexible
        );
        assert_eq!(
            RegisterAuthenticatorKind::parse(""),
            RegisterAuthenticatorKind::Flexible
        );
        assert_eq!(
            RegisterAuthenticatorKind::parse("security-key"),
            RegisterAuthenticatorKind::SecurityKey
        );
    }
}
