//! Client-side i18n script for panel auth HTML pages (login, forgot, token gate).

pub const PANEL_I18N_SCRIPT: &str = r#"
<script>
(function () {
  var STORAGE_KEY = 'cpn-panel-locale';
  var COOKIE_KEY = 'cpn_locale';
  var LABELS = { en: 'English', es: 'Español', nb: 'Norsk' };
  var M = {
    en: {
      languageLabel: 'Language',
      title: 'Sign in',
      documentTitle: 'Sign in · CPN Panel',
      brand: 'CPN PANEL',
      noteReady: 'Initial account is ready for {username}. Full Next.js panel auth will use these credentials when connected.',
      noteMissing: 'No initial account yet. Finish the installer first.',
      username: 'Username',
      password: 'Password',
      forgot: 'Forgot password?',
      submit: 'Sign in',
      postTitle: 'Login received via POST',
      postBody: 'Full panel authentication will connect in a later version.',
      postBack: 'Back to sign in',
      forgotTitle: 'Forgot password',
      forgotDocumentTitle: 'Forgot password · CPN Panel',
      forgotIntro: 'Enter the username or recovery email for your panel account. If a matching account exists, a reset message will be sent when mail delivery is configured.',
      forgotUsername: 'Username',
      forgotEmailLabel: 'Recovery email',
      forgotSubmit: 'Request reset',
      forgotSmtp: 'Password reset email is not connected yet. When SMTP is configured, matching requests will receive a reset link. Until then, ask a server operator to reset the account.',
      forgotBack: 'Back to sign in',
      forgotAckTitle: 'Check your inbox',
      forgotAckBody: 'If an account matches the details you entered, a reset message will be sent when mail delivery is available. For security, this page does not confirm whether an account exists.',
      authBlockedTitle: 'Open the installer URL with its token',
      authBlockedBody: 'Installation is not finished yet. Use the full URL printed in the installer console, including the ?token=... query parameter.',
      authBlockedLogin: 'If installation already finished, open panel login.'
    },
    es: {
      languageLabel: 'Idioma',
      title: 'Iniciar sesión',
      documentTitle: 'Inicio de sesión · CPN Panel',
      brand: 'CPN PANEL',
      noteReady: 'Cuenta inicial lista para {username}. El panel Next.js completo usará estos datos cuando la autenticación esté conectada.',
      noteMissing: 'Todavía no hay una cuenta inicial. Completa el instalador primero.',
      username: 'Usuario',
      password: 'Contraseña',
      forgot: '¿Olvidaste la contraseña?',
      submit: 'Entrar',
      postTitle: 'Login recibido por POST',
      postBody: 'La autenticación completa del panel se conectará en una versión posterior.',
      postBack: 'Volver al inicio de sesión',
      forgotTitle: 'Contraseña olvidada',
      forgotDocumentTitle: 'Contraseña olvidada · CPN Panel',
      forgotIntro: 'Introduce el usuario o el correo de recuperación de tu cuenta del panel. Si existe una cuenta coincidente, se enviará un mensaje de restablecimiento cuando el correo esté configurado.',
      forgotUsername: 'Usuario',
      forgotEmailLabel: 'Correo de recuperación',
      forgotSubmit: 'Solicitar restablecimiento',
      forgotSmtp: 'El correo de restablecimiento aún no está conectado. Cuando SMTP esté configurado, las solicitudes coincidentes recibirán un enlace. Mientras tanto, pide a un operador del servidor que restablezca la cuenta.',
      forgotBack: 'Volver al inicio de sesión',
      forgotAckTitle: 'Revisa tu bandeja de entrada',
      forgotAckBody: 'Si una cuenta coincide con los datos introducidos, se enviará un mensaje de restablecimiento cuando el correo esté disponible. Por seguridad, esta página no confirma si la cuenta existe.',
      authBlockedTitle: 'Abre la URL del instalador con su token',
      authBlockedBody: 'La instalación aún no ha terminado. Usa la URL completa impresa en la consola del instalador, incluyendo el parámetro ?token=...',
      authBlockedLogin: 'Si la instalación ya terminó, abre el inicio de sesión del panel.'
    },
    nb: {
      languageLabel: 'Språk',
      title: 'Logg inn',
      documentTitle: 'Logg inn · CPN Panel',
      brand: 'CPN PANEL',
      noteReady: 'Startkonto er klar for {username}. Full Next.js-panelautentisering vil bruke disse dataene når den er koblet til.',
      noteMissing: 'Ingen startkonto ennå. Fullfør installasjonsveiviseren først.',
      username: 'Brukernavn',
      password: 'Passord',
      forgot: 'Glemt passordet?',
      submit: 'Logg inn',
      postTitle: 'Innlogging mottatt via POST',
      postBody: 'Full panelautentisering kobles til i en senere versjon.',
      postBack: 'Tilbake til innlogging',
      forgotTitle: 'Glemt passord',
      forgotDocumentTitle: 'Glemt passord · CPN Panel',
      forgotIntro: 'Skriv inn brukernavn eller gjenopprettings-e-post for panelkontoen. Hvis en konto matcher, sendes en tilbakestillingsmelding når e-post er konfigurert.',
      forgotUsername: 'Brukernavn',
      forgotEmailLabel: 'Gjenopprettings-e-post',
      forgotSubmit: 'Be om tilbakestilling',
      forgotSmtp: 'E-post for tilbakestilling av passord er ikke koblet til ennå. Når SMTP er konfigurert, får matchende forespørsler en lenke. Inntil da, be en serveroperatør om å tilbakestille kontoen.',
      forgotBack: 'Tilbake til innlogging',
      forgotAckTitle: 'Sjekk innboksen',
      forgotAckBody: 'Hvis en konto matcher opplysningene du oppga, sendes en tilbakestillingsmelding når e-post er tilgjengelig. Av sikkerhetshensyn bekrefter ikke denne siden om kontoen finnes.',
      authBlockedTitle: 'Åpne installasjons-URL-en med token',
      authBlockedBody: 'Installasjonen er ikke ferdig ennå. Bruk hele URL-en som ble skrevet ut i installasjonskonsollen, inkludert ?token=...-parameteren.',
      authBlockedLogin: 'Hvis installasjonen allerede er ferdig, åpne panelinnlogging.'
    }
  };

  function normalize(raw) {
    var value = String(raw || '').trim().toLowerCase();
    if (value.indexOf('es') === 0) return 'es';
    if (value.indexOf('nb') === 0 || value === 'no' || value.indexOf('nn') === 0) return 'nb';
    return 'en';
  }

  function readCookie() {
    var match = document.cookie.match(/(?:^|; )cpn_locale=([^;]*)/);
    return match ? decodeURIComponent(match[1]) : '';
  }

  function writeCookie(locale) {
    document.cookie = COOKIE_KEY + '=' + encodeURIComponent(locale) + '; path=/; max-age=31536000; SameSite=Lax';
  }

  function readStored() {
    try {
      var fromLocal = window.localStorage.getItem(STORAGE_KEY);
      if (fromLocal) return normalize(fromLocal);
    } catch (e) {}
    var fromCookie = readCookie();
    if (fromCookie) return normalize(fromCookie);
    return normalize(document.documentElement.getAttribute('data-initial-locale') || 'en');
  }

  function persist(locale) {
    try { window.localStorage.setItem(STORAGE_KEY, locale); } catch (e) {}
    writeCookie(locale);
  }

  function fill(template, vars) {
    return String(template || '').replace(/\{(\w+)\}/g, function (_, key) {
      return vars[key] != null ? vars[key] : '';
    });
  }

  function apply(locale) {
    var t = M[locale] || M.en;
    var username = document.body.getAttribute('data-username') || 'admin';
    var configured = document.body.getAttribute('data-configured') === '1';
    var page = document.body.getAttribute('data-page') || 'login';
    document.documentElement.lang = locale;
    var select = document.getElementById('cpn-lang');
    if (select) {
      select.value = locale;
      select.setAttribute('aria-label', t.languageLabel);
    }
    var langLabel = document.getElementById('cpn-lang-label');
    if (langLabel) langLabel.textContent = t.languageLabel;

    if (page === 'token') {
      document.title = 'CPN Installer';
      setText('i18n-auth-title', t.authBlockedTitle);
      setText('i18n-auth-body', t.authBlockedBody);
      setText('i18n-auth-login', t.authBlockedLogin);
      return;
    }

    if (page === 'forgot' || page === 'forgot-ack') {
      document.title = t.forgotDocumentTitle;
      setText('i18n-brand', t.brand);
      setText('i18n-title', page === 'forgot-ack' ? t.forgotAckTitle : t.forgotTitle);
      if (page === 'forgot-ack') {
        setText('i18n-forgot-ack', t.forgotAckBody);
        setText('i18n-forgot-smtp', t.forgotSmtp);
        setText('i18n-forgot-back', t.forgotBack);
        return;
      }
      setText('i18n-forgot-intro', t.forgotIntro);
      setText('i18n-forgot-username', t.forgotUsername);
      setText('i18n-forgot-email-label', t.forgotEmailLabel);
      setText('i18n-forgot-submit', t.forgotSubmit);
      setText('i18n-forgot-smtp', t.forgotSmtp);
      setText('i18n-forgot-back', t.forgotBack);
      return;
    }

    if (page === 'post') {
      document.title = t.documentTitle;
      setText('i18n-post-title', t.postTitle);
      setText('i18n-post-body', t.postBody);
      setText('i18n-post-back', t.postBack);
      return;
    }

    document.title = t.documentTitle;
    setText('i18n-brand', t.brand);
    setText('i18n-title', t.title);
    setHtml('i18n-note', configured
      ? fill(t.noteReady, { username: '<strong>' + escapeHtml(username) + '</strong>' })
      : t.noteMissing);
    setText('i18n-username', t.username);
    setText('i18n-password', t.password);
    setText('i18n-forgot', t.forgot);
    setText('i18n-submit', t.submit);
  }

  function setText(id, value) {
    var el = document.getElementById(id);
    if (el) el.textContent = value;
  }

  function setHtml(id, value) {
    var el = document.getElementById(id);
    if (el) el.innerHTML = value;
  }

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function mountSelector() {
    var host = document.getElementById('cpn-lang-host');
    if (!host) return;
    host.innerHTML =
      '<label class="lang">' +
      '<span id="cpn-lang-label" class="lang-label"></span>' +
      '<select id="cpn-lang" aria-label="Language">' +
      Object.keys(LABELS).map(function (code) {
        return '<option value="' + code + '">' + LABELS[code] + '</option>';
      }).join('') +
      '</select></label>';
    document.getElementById('cpn-lang').addEventListener('change', function (event) {
      var next = normalize(event.target.value);
      persist(next);
      apply(next);
    });
  }

  document.addEventListener('DOMContentLoaded', function () {
    mountSelector();
    var locale = readStored();
    persist(locale);
    apply(locale);
  });
})();
</script>
"#;
