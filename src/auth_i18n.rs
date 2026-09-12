//! Client-side i18n script for panel auth HTML pages (login, forgot, token gate).

pub const PANEL_I18N_SCRIPT: &str = r#"
<script>
(function () {
  var STORAGE_KEY = 'cpn-panel-locale';
  var REMEMBER_USER_KEY = 'cpn_remember_username';
  var LEGACY_USER_KEY = 'cpn-panel-last-username';
  var COOKIE_KEY = 'cpn_locale';
  var LABELS = { en: 'English', es: 'Español', nb: 'Norsk' };
  var M = {
    en: {
      languageLabel: 'Language',
      title: 'Sign in',
      documentTitle: 'Sign in · CPN Panel',
      brand: 'CPN PANEL',
      username: 'Username',
      password: 'Password',
      remember: 'Remember me',
      forgot: 'Reset password',
      submit: 'Sign in',
      or: 'or',
      passkey: 'Sign in with passkey',
      showPassword: 'Show password',
      hidePassword: 'Hide password',
      loginError: 'Invalid username or password.',
      forgotTitle: 'Reset your password',
      forgotDocumentTitle: 'Forgot password · CPN Panel',
      forgotIntro: 'For security, password recovery is performed directly on the server.',
      forgotAccount: 'Run this command in the server terminal:',
      forgotSubmit: 'Request reset',
      forgotSmtp: 'The command securely asks for and confirms the new password without putting it in shell history.',
      forgotBack: 'Back to sign in',
      forgotAckTitle: 'Check your inbox',
      forgotAckBody: 'If an account matches the details you entered, a reset message with a one-time link will be sent when mail delivery is available. For security, this page does not confirm whether an account exists.',
      resetTitle: 'Choose a new password',
      resetDocumentTitle: 'Reset password · CPN Panel',
      resetIntro: 'Enter and confirm a new password that meets the panel password policy.',
      resetPassword: 'New password',
      resetConfirm: 'Confirm password',
      resetSubmit: 'Save new password',
      resetInvalidTitle: 'Reset link unavailable',
      resetInvalidHint: 'Request a new reset from the forgot password page.',
      authBlockedTitle: 'Open the installer URL with its token',
      authBlockedBody: 'Installation is not finished yet. Use the full URL printed in the installer console, including the ?token=... query parameter.',
      authBlockedLogin: 'If installation already finished, open panel login.'
    },
    es: {
      languageLabel: 'Idioma',
      title: 'Iniciar sesión',
      documentTitle: 'Inicio de sesión · CPN Panel',
      brand: 'CPN PANEL',
      username: 'Usuario',
      password: 'Contraseña',
      remember: 'Recuérdame',
      forgot: 'Restablecer contraseña',
      submit: 'Entrar',
      or: 'o',
      passkey: 'Entrar con passkey',
      showPassword: 'Mostrar contraseña',
      hidePassword: 'Ocultar contraseña',
      loginError: 'Usuario o contraseña no válidos.',
      forgotTitle: 'Restablece tu contraseña',
      forgotDocumentTitle: 'Contraseña olvidada · CPN Panel',
      forgotIntro: 'Por seguridad, la recuperación se realiza directamente en el servidor.',
      forgotAccount: 'Ejecuta este comando en la terminal del servidor:',
      forgotSubmit: 'Solicitar restablecimiento',
      forgotSmtp: 'El comando solicita y confirma la nueva contraseña de forma segura, sin guardarla en el historial del shell.',
      forgotBack: 'Volver al inicio de sesión',
      forgotAckTitle: 'Revisa tu bandeja de entrada',
      forgotAckBody: 'Si una cuenta coincide con los datos introducidos, se enviará un mensaje con un enlace de un solo uso cuando el correo esté disponible. Por seguridad, esta página no confirma si la cuenta existe.',
      resetTitle: 'Elige una nueva contraseña',
      resetDocumentTitle: 'Restablecer contraseña · CPN Panel',
      resetIntro: 'Introduce y confirma una nueva contraseña que cumpla la política del panel.',
      resetPassword: 'Nueva contraseña',
      resetConfirm: 'Confirmar contraseña',
      resetSubmit: 'Guardar nueva contraseña',
      resetInvalidTitle: 'Enlace de restablecimiento no disponible',
      resetInvalidHint: 'Solicita un nuevo restablecimiento desde la página de contraseña olvidada.',
      authBlockedTitle: 'Abre la URL del instalador con su token',
      authBlockedBody: 'La instalación aún no ha terminado. Usa la URL completa impresa en la consola del instalador, incluyendo el parámetro ?token=...',
      authBlockedLogin: 'Si la instalación ya terminó, abre el inicio de sesión del panel.'
    },
    nb: {
      languageLabel: 'Språk',
      title: 'Logg inn',
      documentTitle: 'Logg inn · CPN Panel',
      brand: 'CPN PANEL',
      username: 'Brukernavn',
      password: 'Passord',
      remember: 'Husk meg',
      forgot: 'Tilbakestill passord',
      submit: 'Logg inn',
      or: 'eller',
      passkey: 'Logg inn med passkey',
      showPassword: 'Vis passord',
      hidePassword: 'Skjul passord',
      loginError: 'Ugyldig brukernavn eller passord.',
      forgotTitle: 'Tilbakestill passordet',
      forgotDocumentTitle: 'Glemt passord · CPN Panel',
      forgotIntro: 'Av sikkerhetshensyn utføres passordgjenoppretting direkte på serveren.',
      forgotAccount: 'Kjør denne kommandoen i serverterminalen:',
      forgotSubmit: 'Be om tilbakestilling',
      forgotSmtp: 'Kommandoen spør sikkert etter og bekrefter det nye passordet uten å legge det i skallhistorikken.',
      forgotBack: 'Tilbake til innlogging',
      forgotAckTitle: 'Sjekk innboksen',
      forgotAckBody: 'Hvis en konto matcher opplysningene du oppga, sendes en melding med engangslenke når e-post er tilgjengelig. Av sikkerhetshensyn bekrefter ikke denne siden om kontoen finnes.',
      resetTitle: 'Velg et nytt passord',
      resetDocumentTitle: 'Tilbakestill passord · CPN Panel',
      resetIntro: 'Skriv inn og bekreft et nytt passord som oppfyller panelets passordpolicy.',
      resetPassword: 'Nytt passord',
      resetConfirm: 'Bekreft passord',
      resetSubmit: 'Lagre nytt passord',
      resetInvalidTitle: 'Tilbakestillingslenke utilgjengelig',
      resetInvalidHint: 'Be om en ny tilbakestilling fra glemt-passord-siden.',
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
    var preferred = (navigator.languages || []).find(function (language) {
      return /^(es|en|nb|nn|no)(-|$)/i.test(language);
    });
    return normalize(preferred || navigator.language || document.documentElement.getAttribute('data-initial-locale') || 'en');
  }

  function persist(locale) {
    try { window.localStorage.setItem(STORAGE_KEY, locale); } catch (e) {}
    writeCookie(locale);
  }

  function apply(locale) {
    var t = M[locale] || M.en;
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
      setText('i18n-forgot-account', t.forgotAccount);
      setText('i18n-forgot-submit', t.forgotSubmit);
      setText('i18n-forgot-smtp', t.forgotSmtp);
      setText('i18n-forgot-back', t.forgotBack);
      return;
    }

    if (page === 'reset-password') {
      document.title = t.resetDocumentTitle;
      setText('i18n-brand', t.brand);
      setText('i18n-title', t.resetTitle);
      setText('i18n-reset-intro', t.resetIntro);
      setText('i18n-reset-password', t.resetPassword);
      setText('i18n-reset-confirm', t.resetConfirm);
      setText('i18n-reset-submit', t.resetSubmit);
      setText('i18n-forgot-back', t.forgotBack);
      return;
    }

    if (page === 'reset-invalid') {
      document.title = t.resetDocumentTitle;
      setText('i18n-brand', t.brand);
      setText('i18n-title', t.resetInvalidTitle);
      setText('i18n-reset-invalid-hint', t.resetInvalidHint);
      setText('i18n-forgot-back', t.forgotSubmit);
      return;
    }

    if (page === 'post') {
      document.title = t.documentTitle;
      return;
    }

    document.title = t.documentTitle;
    setText('i18n-brand', t.brand);
    setText('i18n-title', t.title);
    setText('i18n-username', t.username);
    setText('i18n-password', t.password);
    setText('i18n-remember', t.remember);
    setText('i18n-forgot', t.forgot);
    setText('i18n-submit', t.submit);
    setText('i18n-or', t.or);
    setText('i18n-passkey', t.passkey);
    var err = document.getElementById('i18n-login-error');
    if (err && document.body.getAttribute('data-login-error') === '1') {
      err.hidden = false;
      err.textContent = t.loginError;
    }
    restoreRememberedUsername();
  }

  function clearRememberedUsername() {
    try {
      window.localStorage.removeItem(REMEMBER_USER_KEY);
      window.localStorage.removeItem(LEGACY_USER_KEY);
    } catch (e) {}
  }

  function readRememberedUsername() {
    try {
      var remembered = window.localStorage.getItem(REMEMBER_USER_KEY);
      if (remembered) return String(remembered);
      var legacy = window.localStorage.getItem(LEGACY_USER_KEY);
      if (legacy) {
        window.localStorage.setItem(REMEMBER_USER_KEY, legacy);
        window.localStorage.removeItem(LEGACY_USER_KEY);
        return String(legacy);
      }
    } catch (e) {}
    return '';
  }

  function restoreRememberedUsername() {
    var input = document.getElementById('username');
    var checkbox = document.getElementById('remember_me');
    var password = document.getElementById('password');
    if (password) password.value = '';
    if (!input) return;
    var remembered = readRememberedUsername();
    if (!remembered) {
      if (checkbox) checkbox.checked = false;
      return;
    }
    input.value = remembered;
    if (checkbox) checkbox.checked = true;
  }

  function bindLoginRemember() {
    var form = document.querySelector('form[action*="/login"]');
    var input = document.getElementById('username');
    var checkbox = document.getElementById('remember_me');
    if (!form || !input) return;
    form.addEventListener('submit', function () {
      try {
        var value = String(input.value || '').trim();
        var remember = checkbox && checkbox.checked;
        if (remember && value) {
          window.localStorage.setItem(REMEMBER_USER_KEY, value);
          window.localStorage.removeItem(LEGACY_USER_KEY);
        } else {
          clearRememberedUsername();
        }
      } catch (e) {}
    });
  }

  function setText(id, value) {
    var el = document.getElementById(id);
    if (el) el.textContent = value;
  }

  window.cpnTogglePassword = function (button) {
    var input = document.getElementById('password');
    if (!input) return;
    var revealing = input.type === 'password';
    input.type = revealing ? 'text' : 'password';
    var locale = readStored();
    var t = M[locale] || M.en;
    button.setAttribute('aria-label', revealing ? t.hidePassword : t.showPassword);
    button.textContent = revealing ? '🙈' : '👁';
  };

  function mountSelector() {
    var host = document.getElementById('cpn-lang-host');
    if (!host) return;
    host.innerHTML =
      '<label class="lang">' +
      '<span aria-hidden="true">🌐</span>' +
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
    bindLoginRemember();
    var locale = readStored();
    persist(locale);
    apply(locale);
  });
})();
</script>
"#;
