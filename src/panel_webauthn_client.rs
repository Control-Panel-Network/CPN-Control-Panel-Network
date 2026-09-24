//! Browser WebAuthn client script (register / login / MFA) and message helpers.

/// Map a browser WebAuthn error into a short, high-contrast operator message.
/// No raw W3C URLs. Used by unit tests and embedded in `passkey_client_script`.
pub fn passkey_user_message_logic_js() -> &'static str {
    r#"
function cpnStripUrls(text){
  return String(text||'').replace(/https?:\/\/\S+/gi,'').replace(/\s{2,}/g,' ').replace(/\s+([.,;:!?])/g,'$1').trim();
}
function cpnPasskeyUserMessage(err,kind){
  const name=String((err&&err.name)||'');
  const raw=String((err&&err.message)||err||'');
  const lower=raw.toLowerCase();
  if(/does not support passkeys/i.test(raw)){
    return 'This browser does not support passkeys.';
  }
  if(/open this page as http:\/\/localhost/i.test(raw) || /not 127\.0\.0\.1/i.test(raw)){
    return cpnStripUrls(raw) || 'Open this page as http://localhost with the same port for passkeys.';
  }
  if(/no passkey is registered/i.test(raw) || /no passkeys registered/i.test(raw)){
    return 'No passkey is registered for this account.';
  }
  if(/services are still starting|sign-in is disabled/i.test(raw)){
    return cpnStripUrls(raw) || 'Panel services are still starting. Sign-in is temporarily disabled.';
  }
  if(name==='SecurityError' || /relying party|rp id|securityerror/i.test(lower)){
    return 'Passkey could not run for this site address. Use http://localhost with the same port (127.0.0.1 is redirected automatically), or your panel hostname.';
  }
  if(name==='InvalidStateError' || /already registered|invalid state/i.test(lower)){
    return kind==='login'
      ? 'This passkey cannot be used right now. Try another device or authenticator.'
      : 'This passkey is already registered.';
  }
  if(name==='NotSupportedError' || /not supported|incongruent|inconsistent/i.test(lower)){
    return kind==='login'
      ? 'This authenticator cannot sign in here. Try Windows Hello or a FIDO2 security key.'
      : 'This authenticator cannot register here. Try Windows Hello or a FIDO2 security key.';
  }
  if(name==='NetworkError' || /network|failed to fetch/i.test(lower)){
    return 'Could not reach the panel to finish the passkey step. Check the connection and try again.';
  }
  if(name==='AbortError'){
    return kind==='login'
      ? 'Passkey sign-in was cancelled. Try again when ready.'
      : 'Passkey registration was cancelled. Try again when ready.';
  }
  if(name==='NotAllowedError'){
    // Chrome uses the same NotAllowedError for cancel, timeout, missing UV/PIN, and no authenticator.
    return kind==='login'
      ? 'Passkey sign-in did not complete. Touch your security key or approve Windows Hello when prompted. If you cancelled, try again.'
      : 'Passkey registration did not complete. Touch your security key or approve Windows Hello when prompted. A FIDO2 PIN helps on security keys. If you cancelled, try again.';
  }
  // Do not treat a W3C URL in the message as cancel; many WebAuthn errors include one.
  if(/incorrect passkey|not registered|different account|unknown credential/i.test(lower)){
    return cpnStripUrls(raw) || 'Incorrect passkey. Try again or use another sign-in method.';
  }
  if(/timed out|timeout/i.test(lower) && !/not allowed/i.test(lower)){
    return kind==='login'
      ? 'Passkey sign-in timed out. Try again and complete the prompt sooner.'
      : 'Passkey registration timed out. Try again and complete the prompt sooner.';
  }
  const cleaned=cpnStripUrls(raw);
  if(cleaned) return cleaned;
  return kind==='login' ? 'Passkey sign-in failed. Try again.' : 'Passkey registration failed. Try again.';
}
"#
}

/// Client JS for register (profile) and login ceremonies.
pub fn passkey_client_script() -> String {
    let messages = passkey_user_message_logic_js();
    format!(
        r#"
function cpnB64urlToBuf(b64url){{
  const s=String(b64url).replace(/-/g,'+').replace(/_/g,'/');
  const pad='='.repeat((4-(s.length%4))%4);
  const bin=atob(s+pad);
  const out=new Uint8Array(bin.length);
  for(let i=0;i<bin.length;i++) out[i]=bin.charCodeAt(i);
  return out.buffer;
}}
function cpnBufToB64url(buf){{
  const bytes=new Uint8Array(buf);
  let s='';
  for(const b of bytes) s+=String.fromCharCode(b);
  return btoa(s).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');
}}
async function cpnDecodeCreateOptions(pk){{
  pk.challenge=cpnB64urlToBuf(pk.challenge);
  pk.user.id=cpnB64urlToBuf(pk.user.id);
  if(pk.excludeCredentials){{
    for(const c of pk.excludeCredentials){{ c.id=cpnB64urlToBuf(c.id); }}
  }}
  return pk;
}}
async function cpnDecodeGetOptions(pk){{
  pk.challenge=cpnB64urlToBuf(pk.challenge);
  if(pk.allowCredentials){{
    for(const c of pk.allowCredentials){{ c.id=cpnB64urlToBuf(c.id); }}
  }}
  return pk;
}}
function cpnCredToJson(cred){{
  const r={{
    id:cred.id,
    rawId:cpnBufToB64url(cred.rawId),
    type:cred.type,
    response:{{}}
  }};
  const resp=cred.response;
  if(resp.clientDataJSON) r.response.clientDataJSON=cpnBufToB64url(resp.clientDataJSON);
  if(resp.attestationObject) r.response.attestationObject=cpnBufToB64url(resp.attestationObject);
  if(resp.authenticatorData) r.response.authenticatorData=cpnBufToB64url(resp.authenticatorData);
  if(resp.signature) r.response.signature=cpnBufToB64url(resp.signature);
  if(resp.userHandle) r.response.userHandle=cpnBufToB64url(resp.userHandle);
  return r;
}}
async function cpnJson(url,body){{
  const res=await fetch(url,{{
    method:'POST',
    headers:{{'Content-Type':'application/json','Accept':'application/json'}},
    credentials:'same-origin',
    body:JSON.stringify(body||{{}})
  }});
  const data=await res.json().catch(()=>({{}}));
  if(!res.ok) throw new Error(cpnStripUrls(data.error||('Request failed ('+res.status+')')));
  return data;
}}
function cpnShowPasskeyError(message){{
  const msg=cpnStripUrls(message)||'Passkey sign-in failed.';
  const banner=document.getElementById('i18n-login-error');
  if(banner){{
    banner.hidden=false;
    banner.removeAttribute('hidden');
    banner.setAttribute('role','alert');
    banner.textContent=msg;
    try{{ banner.scrollIntoView({{behavior:'smooth',block:'nearest'}}); }}catch(e){{}}
  }}
  const status=document.getElementById('cpn-passkey-login-status')
    ||document.getElementById('cpn-passkey-status');
  if(status){{
    status.textContent=msg;
    status.style.color='#fecaca';
    status.style.fontWeight='650';
  }}
  if(!banner && !status){{
    try{{ window.alert(msg); }}catch(e){{}}
  }}
}}
function cpnClearPasskeyError(){{
  const banner=document.getElementById('i18n-login-error');
  if(banner && !document.body.getAttribute('data-login-error')){{
    banner.hidden=true;
    banner.textContent='';
  }}
  const status=document.getElementById('cpn-passkey-login-status')
    ||document.getElementById('cpn-passkey-status');
  if(status){{
    status.textContent='';
    status.style.color='';
    status.style.fontWeight='';
  }}
}}
function cpnPreferLocalhostForPasskeys(){{
  const h=String((location&&location.hostname)||'');
  if(h==='127.0.0.1'||h==='[::1]'||h==='::1'){{
    const port=location.port?(':'+location.port):'';
    const dest='http://localhost'+port+location.pathname+location.search+location.hash;
    try{{ location.replace(dest); }}catch(e){{ location.href=dest; }}
    return true;
  }}
  return false;
}}
{messages}
async function cpnRegisterPasskey(){{
  const status=document.getElementById('cpn-passkey-status');
  try{{
    cpnClearPasskeyError();
    if(!window.PublicKeyCredential) throw new Error('This browser does not support passkeys');
    // Browsers bind RP ID to the page host; use localhost (not 127.0.0.1) for create().
    if(cpnPreferLocalhostForPasskeys()) return;
    if(status) status.textContent='Starting registration...';
    const label=(document.getElementById('cpn-passkey-label')||{{}}).value||'';
    const next=cpnPasskeyRegisterNext();
    const start=await cpnJson('/account/users/profile/passkey/register/start',{{}});
    const pk=await cpnDecodeCreateOptions(start.publicKey);
    const cred=await navigator.credentials.create({{publicKey:pk}});
    if(!cred) throw new Error('Passkey registration did not complete.');
    const finish=await cpnJson('/account/users/profile/passkey/register/finish',{{
      ceremony_id:start.ceremony_id,
      label:label,
      next:next,
      credential:cpnCredToJson(cred)
    }});
    if(status) status.textContent='Passkey registered.';
    const go=finish.redirect||next||'';
    if(go){{ location.href=go; return; }}
    location.reload();
  }}catch(err){{
    const msg=cpnPasskeyUserMessage(err,'register');
    cpnShowPasskeyError(msg);
    if(status) status.textContent=msg;
  }}
}}
function cpnPasskeyRegisterNext(){{
  const path=String((location&&location.pathname)||'');
  const enroll=document.getElementById('cpn-passkey-enroll');
  if(enroll){{
    // MFA gate may overlay Edit/View profile; return there so another passkey can be added.
    if(path==='/account/users/modify'||path.indexOf('/account/users/modify/')===0
      ||path==='/account/users/profile'||path.indexOf('/account/users/profile/')===0){{
      return '/account/users/modify?notice=Passkey+registered';
    }}
    const fromEnroll=enroll.getAttribute('data-redirect');
    if(fromEnroll) return fromEnroll;
    return '/account/users/modify?notice=Passkey+registered';
  }}
  const box=document.getElementById('cpn-passkey-register');
  if(box){{
    const fromBox=box.getAttribute('data-redirect');
    if(fromBox) return fromBox;
  }}
  if(path.indexOf('/account/users/')===0){{
    return '/account/users/modify?notice=Passkey+registered';
  }}
  return '';
}}
async function cpnLoginPasskey(){{
  const status=document.getElementById('cpn-passkey-login-status');
  try{{
    cpnClearPasskeyError();
    if(document.body && document.body.getAttribute('data-services-ready')==='0'){{
      throw new Error('Panel services are still starting. Sign-in is disabled until Web server and MariaDB are running.');
    }}
    if(!window.PublicKeyCredential) throw new Error('This browser does not support passkeys');
    if(cpnPreferLocalhostForPasskeys()) return;
    if(status){{ status.textContent='Waiting for authenticator...'; status.style.color=''; status.style.fontWeight=''; }}
    const start=await cpnJson('/login/passkey/start',{{}});
    const pk=await cpnDecodeGetOptions(start.publicKey);
    const cred=await navigator.credentials.get({{publicKey:pk}});
    if(!cred) throw new Error('Passkey sign-in did not complete.');
    const finish=await cpnJson('/login/passkey/finish',{{
      ceremony_id:start.ceremony_id,
      credential:cpnCredToJson(cred),
      next:(document.body&&document.body.getAttribute('data-login-next'))||new URLSearchParams(location.search).get('next')||''
    }});
    location.href=finish.redirect||'/dashboard';
  }}catch(err){{
    cpnShowPasskeyError(cpnPasskeyUserMessage(err,'login'));
  }}
}}
async function cpnMfaPasskey(){{
  const status=document.getElementById('cpn-passkey-login-status');
  try{{
    cpnClearPasskeyError();
    if(!window.PublicKeyCredential) throw new Error('This browser does not support passkeys');
    if(cpnPreferLocalhostForPasskeys()) return;
    if(status){{ status.textContent='Waiting for authenticator...'; status.style.color=''; status.style.fontWeight=''; }}
    const start=await cpnJson('/login/2fa/passkey/start',{{}});
    const pk=await cpnDecodeGetOptions(start.publicKey);
    const cred=await navigator.credentials.get({{publicKey:pk}});
    if(!cred) throw new Error('Passkey sign-in did not complete.');
    const finish=await cpnJson('/login/2fa/passkey/finish',{{
      ceremony_id:start.ceremony_id,
      credential:cpnCredToJson(cred),
      next:(document.body&&document.body.getAttribute('data-login-next'))||new URLSearchParams(location.search).get('next')||''
    }});
    location.href=finish.redirect||'/dashboard';
  }}catch(err){{
    cpnShowPasskeyError(cpnPasskeyUserMessage(err,'login'));
  }}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::{passkey_client_script, passkey_user_message_logic_js};

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
        assert!(
            script.contains("function cpnPreferLocalhostForPasskeys"),
            "passkey ceremonies must redirect 127.0.0.1 to localhost"
        );
        assert!(
            script.contains("location.replace(dest)"),
            "localhost redirect must replace the current page"
        );
    }

    #[test]
    fn error_mapper_does_not_treat_w3c_url_as_cancel() {
        let logic = passkey_user_message_logic_js();
        assert!(
            !logic.contains("/w3\\.org\\/TR\\/webauthn/i.test"),
            "W3C URL alone must not map to cancelled/timed out"
        );
        assert!(
            logic.contains("name==='NotAllowedError'"),
            "NotAllowedError must have its own friendly branch"
        );
        assert!(
            logic.contains("name==='AbortError'"),
            "AbortError must map to cancelled, not a generic timeout"
        );
        assert!(
            logic.contains("name==='SecurityError'"),
            "SecurityError/RP ID must stay distinct"
        );
        assert!(
            logic.contains("name==='NotSupportedError'"),
            "NotSupportedError must stay distinct from cancel"
        );
        assert!(
            !logic.contains("cancelled or timed out"),
            "avoid the old ambiguous cancelled-or-timed-out copy"
        );
        assert!(
            logic.contains("Touch your security key")
                || logic.contains("approve Windows Hello"),
            "NotAllowedError copy should guide YubiKey / Hello"
        );
    }

    #[test]
    fn status_color_is_high_contrast_on_dark() {
        let script = passkey_client_script();
        assert!(
            script.contains("#fecaca"),
            "passkey status errors need a light red on dark theme"
        );
    }
}
