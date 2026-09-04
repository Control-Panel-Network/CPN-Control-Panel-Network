//! WebAuthn relying-party helpers and short-lived ceremony state.
//! RP ID follows the request Host (works for `127.0.0.1` lab and production FQDNs).

use crate::account::{data_dir, now_unix};
use crate::account_passkeys::{
    add_passkey, exclude_credential_ids, passkeys_for_auth, update_passkey_after_auth,
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
    CreationChallengeResponse, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential,
    RegisterPublicKeyCredential, RequestChallengeResponse, Webauthn, WebauthnBuilder,
};

const CEREMONY_TTL_SECS: u64 = 300;
const MAX_HOST_LEN: usize = 253;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CeremonyKind {
    Register(PasskeyRegistration),
    Authenticate {
        username: String,
        state: PasskeyAuthentication,
    },
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

/// Build a Webauthn RP for this HTTP request (origin + RP ID without port).
pub fn webauthn_for_request(
    host_header: Option<&str>,
    https: bool,
) -> Result<(Webauthn, String), String> {
    let host = sanitize_host(host_header.unwrap_or("127.0.0.1"))?;
    let rp_id = host
        .split(':')
        .next()
        .unwrap_or("127.0.0.1")
        .trim()
        .to_string();
    if rp_id.is_empty() {
        return Err("Could not derive WebAuthn RP ID".into());
    }
    let scheme = if https { "https" } else { "http" };
    let origin = format!("{scheme}://{host}");
    let origin_url = Url::parse(&origin).map_err(|err| format!("Invalid origin URL: {err}"))?;
    let webauthn = WebauthnBuilder::new(&rp_id, &origin_url)
        .map_err(|err| format!("WebAuthn builder error: {err}"))?
        .rp_name("CPN Panel")
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
    username: &str,
) -> Result<(String, RequestChallengeResponse), String> {
    let creds = passkeys_for_auth(username);
    if creds.is_empty() {
        return Err("No passkeys registered for this account".into());
    }
    let (rcr, state) = webauthn
        .start_passkey_authentication(&creds)
        .map_err(|err| format!("Could not start passkey authentication: {err}"))?;
    let id = new_ceremony_id();
    save_ceremony(&CeremonyRecord {
        id: id.clone(),
        username: username.to_string(),
        created_at_unix: now_unix(),
        kind: CeremonyKind::Authenticate {
            username: username.to_string(),
            state,
        },
    })?;
    Ok((id, rcr))
}

pub fn finish_authentication(
    webauthn: &Webauthn,
    ceremony_id: &str,
    credential: &PublicKeyCredential,
) -> Result<String, String> {
    let record = take_ceremony(ceremony_id)?;
    let CeremonyKind::Authenticate { username, state } = record.kind else {
        return Err("Passkey ceremony type mismatch".into());
    };
    let result = webauthn
        .finish_passkey_authentication(credential, &state)
        .map_err(|err| format!("Passkey authentication failed: {err}"))?;
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
  if(!res.ok) throw new Error(data.error||('Request failed ('+res.status+')'));
  return data;
}
async function cpnRegisterPasskey(){
  const status=document.getElementById('cpn-passkey-status');
  try{
    if(!window.PublicKeyCredential) throw new Error('This browser does not support passkeys');
    if(status) status.textContent='Starting registration…';
    const label=(document.getElementById('cpn-passkey-label')||{}).value||'';
    const start=await cpnJson('/account/users/profile/passkey/register/start',{});
    const pk=await cpnDecodeCreateOptions(start.publicKey);
    const cred=await navigator.credentials.create({publicKey:pk});
    if(!cred) throw new Error('Passkey creation cancelled');
    await cpnJson('/account/users/profile/passkey/register/finish',{
      ceremony_id:start.ceremony_id,
      label:label,
      credential:cpnCredToJson(cred)
    });
    if(status) status.textContent='Passkey registered.';
    location.reload();
  }catch(err){
    if(status) status.textContent=String(err.message||err);
  }
}
async function cpnLoginPasskey(){
  const status=document.getElementById('cpn-passkey-login-status');
  const userEl=document.getElementById('username');
  try{
    if(!window.PublicKeyCredential) throw new Error('This browser does not support passkeys');
    const username=(userEl&&userEl.value||'').trim();
    if(!username) throw new Error('Enter your username first');
    if(status) status.textContent='Waiting for authenticator…';
    const start=await cpnJson('/login/passkey/start',{username:username});
    const pk=await cpnDecodeGetOptions(start.publicKey);
    const cred=await navigator.credentials.get({publicKey:pk});
    if(!cred) throw new Error('Passkey sign-in cancelled');
    const finish=await cpnJson('/login/passkey/finish',{
      ceremony_id:start.ceremony_id,
      credential:cpnCredToJson(cred)
    });
    location.href=finish.redirect||'/dashboard';
  }catch(err){
    if(status) status.textContent=String(err.message||err);
  }
}
"#
}
