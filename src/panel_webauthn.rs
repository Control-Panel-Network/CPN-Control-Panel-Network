//! WebAuthn relying-party helpers and short-lived ceremony state.
//! RP ID follows the request Host (works for `127.0.0.1` lab and production FQDNs).

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
    CreationChallengeResponse, DiscoverableAuthentication, DiscoverableKey, PasskeyAuthentication,
    PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse, Webauthn, WebauthnBuilder,
};

const CEREMONY_TTL_SECS: u64 = 300;
const MAX_HOST_LEN: usize = 253;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CeremonyKind {
    Register(PasskeyRegistration),
    Authenticate { state: PasskeyAuthentication },
    Discoverable(DiscoverableAuthentication),
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
/// so lab access via either URL can verify ceremonies. Prefer `localhost` in the
/// browser for create/get; API begin-register works for either Host header.
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
        .allow_any_port(true);
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
    let webauthn = builder
        .build()
        .map_err(|err| format!("WebAuthn build error: {err}"))?;
    Ok((webauthn, rp_id))
}

pub fn start_registration(
    webauthn: &Webauthn,
    username: &str,
) -> Result<(String, CreationChallengeResponse), String> {
    let exclude = exclude_credential_ids(username);
    let exclude = if exclude.is_empty() {
        None
    } else {
        Some(exclude)
    };
    let (ccr, state) = webauthn
        .start_passkey_registration(user_uuid(username), username, username, exclude)
        .map_err(|err| format!("Could not start passkey registration: {err}"))?;
    let id = new_ceremony_id();
    save_ceremony(&CeremonyRecord {
        id: id.clone(),
        username: username.to_string(),
        created_at_unix: now_unix(),
        kind: CeremonyKind::Register(state),
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
    let CeremonyKind::Register(state) = record.kind else {
        return Err("Passkey ceremony type mismatch".into());
    };
    let passkey = webauthn
        .finish_passkey_registration(credential, &state)
        .map_err(|err| format!("Passkey registration failed: {err}"))?;
    add_passkey(username, label, passkey)?;
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
            .into_iter()
            .map(|(_, passkey)| passkey)
            .collect::<Vec<_>>();
        let (rcr, state) = webauthn
            .start_passkey_authentication(&keys)
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
    let (rcr, state) = webauthn
        .start_passkey_authentication(&keys)
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
                .finish_passkey_authentication(credential, &state)
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
        CeremonyKind::Register(_) => return Err("Passkey ceremony type mismatch".into()),
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

/// Client JS for register (profile) and login ceremonies.
pub fn passkey_client_script() -> &'static str {
    r#"
function cpnB64urlToBuf(b64url){
  const s=String(b64url).replace(/-/g,'+').replace(/_/g,'/');
  const pad='='.repeat((4-(s.length%4))%4);
  const bin=atob(s+pad);
  const out=new Uint8Array(bin.length);
  for(let i=0;i<bin.length;i++) out[i]=bin.charCodeAt(i);
  return out.buffer;
}
function cpnBufToB64url(buf){
  const bytes=new Uint8Array(buf);
  let s='';
  for(const b of bytes) s+=String.fromCharCode(b);
  return btoa(s).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');
}
async function cpnDecodeCreateOptions(pk){
  pk.challenge=cpnB64urlToBuf(pk.challenge);
  pk.user.id=cpnB64urlToBuf(pk.user.id);
  if(pk.excludeCredentials){
    for(const c of pk.excludeCredentials){ c.id=cpnB64urlToBuf(c.id); }
  }
  return pk;
}
async function cpnDecodeGetOptions(pk){
  pk.challenge=cpnB64urlToBuf(pk.challenge);
  if(pk.allowCredentials){
    for(const c of pk.allowCredentials){ c.id=cpnB64urlToBuf(c.id); }
  }
  return pk;
}
function cpnCredToJson(cred){
  const r={
    id:cred.id,
    rawId:cpnBufToB64url(cred.rawId),
    type:cred.type,
    response:{}
  };
  const resp=cred.response;
  if(resp.clientDataJSON) r.response.clientDataJSON=cpnBufToB64url(resp.clientDataJSON);
  if(resp.attestationObject) r.response.attestationObject=cpnBufToB64url(resp.attestationObject);
  if(resp.authenticatorData) r.response.authenticatorData=cpnBufToB64url(resp.authenticatorData);
  if(resp.signature) r.response.signature=cpnBufToB64url(resp.signature);
  if(resp.userHandle) r.response.userHandle=cpnBufToB64url(resp.userHandle);
  return r;
}
async function cpnJson(url,body){
  const res=await fetch(url,{
    method:'POST',
    headers:{'Content-Type':'application/json','Accept':'application/json'},
    credentials:'same-origin',
    body:JSON.stringify(body||{})
  });
  const data=await res.json().catch(()=>({}));
  if(!res.ok) throw new Error(cpnStripUrls(data.error||('Request failed ('+res.status+')')));
  return data;
}
function cpnStripUrls(text){
  return String(text||'').replace(/https?:\/\/\S+/gi,'').replace(/\s{2,}/g,' ').replace(/\s+([.,;:!?])/g,'$1').trim();
}
function cpnShowPasskeyError(message){
  const msg=cpnStripUrls(message)||'Passkey sign-in failed.';
  const banner=document.getElementById('i18n-login-error');
  if(banner){
    banner.hidden=false;
    banner.removeAttribute('hidden');
    banner.setAttribute('role','alert');
    banner.textContent=msg;
    try{ banner.scrollIntoView({behavior:'smooth',block:'nearest'}); }catch(e){}
  }
  const status=document.getElementById('cpn-passkey-login-status')
    ||document.getElementById('cpn-passkey-status');
  if(status){
    status.textContent=msg;
    status.style.color='#991b1b';
    status.style.fontWeight='650';
  }
  if(!banner && !status){
    try{ window.alert(msg); }catch(e){}
  }
}
function cpnClearPasskeyError(){
  const banner=document.getElementById('i18n-login-error');
  if(banner && !document.body.getAttribute('data-login-error')){
    banner.hidden=true;
    banner.textContent='';
  }
  const status=document.getElementById('cpn-passkey-login-status')
    ||document.getElementById('cpn-passkey-status');
  if(status){
    status.textContent='';
    status.style.color='';
    status.style.fontWeight='';
  }
}
function cpnPasskeyUserMessage(err,kind){
  const name=String((err&&err.name)||'');
  const raw=String((err&&err.message)||err||'');
  const lower=raw.toLowerCase();
  if(/does not support passkeys/i.test(raw)){
    return 'This browser does not support passkeys.';
  }
  if(/open this page as http:\/\/localhost/i.test(raw) || /not 127\.0\.0\.1/i.test(raw)){
    return cpnStripUrls(raw) || 'Open this page as http://localhost (not 127.0.0.1) for passkeys.';
  }
  if(/no passkey is registered/i.test(raw) || /no passkeys registered/i.test(raw)){
    return 'No passkey is registered for this account.';
  }
  if(/services are still starting|sign-in is disabled/i.test(raw)){
    return cpnStripUrls(raw) || 'Panel services are still starting. Sign-in is temporarily disabled.';
  }
  if(name==='NotAllowedError'||name==='AbortError'
    ||/not allowed|timed out|timeout|abort|cancel+ed/i.test(raw)
    ||/w3\.org\/TR\/webauthn/i.test(raw)){
    return kind==='login'
      ? 'Passkey sign-in was cancelled or timed out. Try again.'
      : 'Passkey registration was cancelled or timed out. Try again.';
  }
  if(name==='InvalidStateError' || /already registered|invalid state/i.test(lower)){
    return kind==='login'
      ? 'This passkey cannot be used right now. Try another device or authenticator.'
      : 'This passkey is already registered.';
  }
  if(name==='SecurityError' || /relying party|rp id|securityerror/i.test(lower)){
    return 'Passkey could not run for this site address. Prefer http://localhost with the same port, or your panel hostname.';
  }
  if(/incorrect passkey|not registered|different account|unknown credential/i.test(lower)){
    return cpnStripUrls(raw) || 'Incorrect passkey. Try again or use another sign-in method.';
  }
  const cleaned=cpnStripUrls(raw);
  if(cleaned) return cleaned;
  return kind==='login' ? 'Passkey sign-in failed. Try again.' : 'Passkey registration failed. Try again.';
}
async function cpnRegisterPasskey(){
  const status=document.getElementById('cpn-passkey-status');
  try{
    cpnClearPasskeyError();
    if(!window.PublicKeyCredential) throw new Error('This browser does not support passkeys');
    // Browsers bind RP ID to the page host; use localhost (not 127.0.0.1) for create().
    if(location.hostname==='127.0.0.1'||location.hostname==='[::1]'||location.hostname==='::1'){
      const port=location.port?(':'+location.port):'';
      throw new Error('Open this page as http://localhost'+port+' (not 127.0.0.1) to register a passkey.');
    }
    if(status) status.textContent='Starting registration...';
    const label=(document.getElementById('cpn-passkey-label')||{}).value||'';
    const next=cpnPasskeyRegisterNext();
    const start=await cpnJson('/account/users/profile/passkey/register/start',{});
    const pk=await cpnDecodeCreateOptions(start.publicKey);
    const cred=await navigator.credentials.create({publicKey:pk});
    if(!cred) throw new Error('Passkey registration was cancelled or timed out.');
    const finish=await cpnJson('/account/users/profile/passkey/register/finish',{
      ceremony_id:start.ceremony_id,
      label:label,
      next:next,
      credential:cpnCredToJson(cred)
    });
    if(status) status.textContent='Passkey registered.';
    const go=finish.redirect||next||'';
    if(go){ location.href=go; return; }
    location.reload();
  }catch(err){
    const msg=cpnPasskeyUserMessage(err,'register');
    cpnShowPasskeyError(msg);
    if(status) status.textContent=msg;
  }
}
function cpnPasskeyRegisterNext(){
  const path=String((location&&location.pathname)||'');
  const enroll=document.getElementById('cpn-passkey-enroll');
  if(enroll){
    // MFA gate may overlay Edit/View profile; return there so another passkey can be added.
    if(path==='/account/users/modify'||path.indexOf('/account/users/modify/')===0
      ||path==='/account/users/profile'||path.indexOf('/account/users/profile/')===0){
      return '/account/users/modify?notice=Passkey+registered';
    }
    const fromEnroll=enroll.getAttribute('data-redirect');
    if(fromEnroll) return fromEnroll;
    return '/account/users/modify?notice=Passkey+registered';
  }
  const box=document.getElementById('cpn-passkey-register');
  if(box){
    const fromBox=box.getAttribute('data-redirect');
    if(fromBox) return fromBox;
  }
  if(path.indexOf('/account/users/')===0){
    return '/account/users/modify?notice=Passkey+registered';
  }
  return '';
}
async function cpnLoginPasskey(){
  const status=document.getElementById('cpn-passkey-login-status');
  try{
    cpnClearPasskeyError();
    if(document.body && document.body.getAttribute('data-services-ready')==='0'){
      throw new Error('Panel services are still starting. Sign-in is disabled until Web server and MariaDB are running.');
    }
    if(!window.PublicKeyCredential) throw new Error('This browser does not support passkeys');
    if(status){ status.textContent='Waiting for authenticator...'; status.style.color=''; status.style.fontWeight=''; }
    const start=await cpnJson('/login/passkey/start',{});
    const pk=await cpnDecodeGetOptions(start.publicKey);
    const cred=await navigator.credentials.get({publicKey:pk});
    if(!cred) throw new Error('Passkey sign-in was cancelled or timed out.');
    const finish=await cpnJson('/login/passkey/finish',{
      ceremony_id:start.ceremony_id,
      credential:cpnCredToJson(cred),
      next:(document.body&&document.body.getAttribute('data-login-next'))||new URLSearchParams(location.search).get('next')||''
    });
    location.href=finish.redirect||'/dashboard';
  }catch(err){
    cpnShowPasskeyError(cpnPasskeyUserMessage(err,'login'));
  }
}
async function cpnMfaPasskey(){
  const status=document.getElementById('cpn-passkey-login-status');
  try{
    cpnClearPasskeyError();
    if(!window.PublicKeyCredential) throw new Error('This browser does not support passkeys');
    if(status){ status.textContent='Waiting for authenticator...'; status.style.color=''; status.style.fontWeight=''; }
    const start=await cpnJson('/login/2fa/passkey/start',{});
    const pk=await cpnDecodeGetOptions(start.publicKey);
    const cred=await navigator.credentials.get({publicKey:pk});
    if(!cred) throw new Error('Passkey sign-in was cancelled or timed out.');
    const finish=await cpnJson('/login/2fa/passkey/finish',{
      ceremony_id:start.ceremony_id,
      credential:cpnCredToJson(cred),
      next:(document.body&&document.body.getAttribute('data-login-next'))||new URLSearchParams(location.search).get('next')||''
    });
    location.href=finish.redirect||'/dashboard';
  }catch(err){
    cpnShowPasskeyError(cpnPasskeyUserMessage(err,'login'));
  }
}
"#
}

#[cfg(test)]
mod tests {
    use super::{passkey_client_script, start_authentication, webauthn_for_request};
    use crate::account::with_test_data_dir;

    #[test]
    fn register_script_sends_next_and_prefers_profile_return() {
        let script = passkey_client_script();
        assert!(
            script.contains("function cpnPasskeyRegisterNext"),
            "client must compute context-aware return path"
        );
        assert!(
            script.contains("next:next"),
            "register finish must send next for server allowlist"
        );
        assert!(
            script.contains("/account/users/modify?notice=Passkey+registered"),
            "profile/edit overlay must return to Modify User"
        );
        assert!(
            script.contains("finish.redirect||next"),
            "client must honor server redirect"
        );
        assert!(
            script.contains("function cpnShowPasskeyError"),
            "login/2FA must surface high-contrast passkey errors"
        );
        assert!(
            script.contains("function cpnMfaPasskey"),
            "2FA page must offer passkey assertion"
        );
    }

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
    fn localhost_host_builds_webauthn() {
        let (wan, rp_id) = webauthn_for_request(Some("localhost:2087"), false)
            .expect("builder should accept localhost");
        assert_eq!(rp_id, "localhost");
        assert!(!wan.get_allowed_origins().is_empty());
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
}
