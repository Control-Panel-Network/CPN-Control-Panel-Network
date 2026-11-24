//! PHP snippets injected into the webmail admin action file by
//! `tachyon_admin_bridge_install`. Kept separate so the installer stays small.
//!
//! Three hooks, all owner-only and fail-closed:
//! 1. `DoAdminLogin`: password (+ authenticator code) checked by the panel over loopback.
//! 2. `AdminAppData`: single sign-on from a valid CPN panel owner session cookie.
//! 3. `DoAdminLogout`: remember an explicit sign-out so SSO does not instantly re-admit.

pub const LOGIN_BEGIN: &str = "// CPN-ADMIN-BRIDGE-BEGIN";
pub const LOGIN_END: &str = "// CPN-ADMIN-BRIDGE-END";
pub const SSO_BEGIN: &str = "// CPN-ADMIN-SSO-BEGIN";
pub const SSO_END: &str = "// CPN-ADMIN-SSO-END";
pub const LOGOUT_BEGIN: &str = "// CPN-ADMIN-SSO-LOGOUT-BEGIN";
pub const LOGOUT_END: &str = "// CPN-ADMIN-SSO-LOGOUT-END";

pub const LOGIN_VERSION: &str = "v2";
pub const SSO_VERSION: &str = "v1";
pub const LOGOUT_VERSION: &str = "v1";

/// Injected at the top of `DoAdminLogin()` (tabs match upstream style).
pub const PHP_LOGIN_HOOK: &str = r##"		// CPN-ADMIN-BRIDGE-BEGIN v2
		// Webmail admin sign-in is delegated to the CPN panel owner account (owner only).
		// Mailbox IMAP login is unchanged. Create /var/lib/cpn-webmail/admin-bridge.disabled
		// and re-run CPN webmail heal to restore the local admin_login / admin_password.
		$cpnCfgPath = '/var/lib/cpn-webmail/admin-bridge.json';
		if (@\file_exists($cpnCfgPath)) {
			$cpnLogin = \trim((string) $this->GetActionParam('Login', ''));
			$cpnPass = (string) $this->GetActionParam('Password', '');
			$cpnTotp = (string) $this->GetActionParam('TOTP', '');
			$cpnOk = false;
			$cpnReason = '';
			$cpnCfg = \json_decode((string) @\file_get_contents($cpnCfgPath), true);
			$cpnUrl = \is_array($cpnCfg) ? (string) ($cpnCfg['url'] ?? '') : '';
			$cpnToken = \is_array($cpnCfg) ? (string) ($cpnCfg['token'] ?? '') : '';
			$cpnHost = \strtolower((string) \parse_url($cpnUrl, PHP_URL_HOST));
			if (\strlen($cpnLogin) && \strlen($cpnPass) && \strlen($cpnToken)
				&& $this->Config()->Get('security', 'allow_admin_panel', true)
				&& \in_array($cpnHost, ['127.0.0.1', 'localhost', '[::1]', '::1'], true)
			) {
				$cpnBody = \json_encode(['username' => $cpnLogin, 'password' => $cpnPass, 'totp' => $cpnTotp], JSON_UNESCAPED_SLASHES);
				$cpnEndpoint = \rtrim($cpnUrl, '/') . '/api/internal/tachyon-admin/verify';
				$cpnHeaders = ['Content-Type: application/json', 'Accept: application/json', 'X-CPN-Bridge-Token: ' . $cpnToken];
				$cpnRaw = false;
				if (\function_exists('curl_init')) {
					$cpnCh = \curl_init($cpnEndpoint);
					\curl_setopt_array($cpnCh, [
						CURLOPT_POST => true,
						CURLOPT_POSTFIELDS => $cpnBody,
						CURLOPT_HTTPHEADER => $cpnHeaders,
						CURLOPT_RETURNTRANSFER => true,
						CURLOPT_TIMEOUT => 10,
						CURLOPT_CONNECTTIMEOUT => 3,
						CURLOPT_FOLLOWLOCATION => false,
						CURLOPT_SSL_VERIFYPEER => false,
						CURLOPT_SSL_VERIFYHOST => 0,
					]);
					$cpnRaw = \curl_exec($cpnCh);
					unset($cpnCh);
				} else {
					$cpnCtx = \stream_context_create([
						'http' => ['method' => 'POST', 'header' => \implode("\r\n", $cpnHeaders), 'content' => $cpnBody, 'timeout' => 10, 'ignore_errors' => true, 'follow_location' => 0],
						'ssl' => ['verify_peer' => false, 'verify_peer_name' => false],
					]);
					$cpnRaw = @\file_get_contents($cpnEndpoint, false, $cpnCtx);
				}
				$cpnJson = \is_string($cpnRaw) ? \json_decode($cpnRaw, true) : null;
				$cpnOk = \is_array($cpnJson) && true === ($cpnJson['ok'] ?? false);
				$cpnReason = \is_array($cpnJson) ? (string) ($cpnJson['reason'] ?? '') : '';
			}
			unset($cpnPass, $cpnBody, $cpnToken, $cpnCfg);
			if (!$cpnOk) {
				// Fixed texts keyed by a short panel hint (only sent after the password was verified).
				$cpnMsgs = [
					'mfa_required' => 'Authenticator code required. Enter the current code from your authenticator app (or a backup code) in the TOTP code field.',
					'mfa_failed' => 'Authenticator code is wrong or expired. Enter the current code (or an unused backup code) and try again.',
					'passkey_only_owner' => 'This owner account uses passkeys only. Add an authenticator app in CPN Security, or open this page while signed in to the CPN panel as the owner.',
					'mfa_enroll_required' => 'Finish multi-factor enrollment in the CPN panel first, then sign in here again.',
					'must_change_password' => 'Change the CPN panel password first, then sign in here again.',
					'rate_limited' => 'Too many attempts. Wait a few minutes and try again.',
				];
				$this->LoggerAuthHelper(null, $cpnLogin, true);
				$this->loginErrorDelay();
				throw new ClientException(Notifications::AuthError, null, $cpnMsgs[$cpnReason] ?? '');
			}
			if (isset($_COOKIE['cpn_sso_off']) && !\headers_sent()) {
				\setcookie('cpn_sso_off', '', ['expires' => 1, 'path' => '/']);
			}
			$sToken = $this->setAdminAuthToken();
			return $this->DefaultResponse($sToken ? $this->AppData(true) : false);
		}
		// CPN-ADMIN-BRIDGE-END
"##;

/// Injected in `AdminAppData()` right before the `Auth` flag is computed.
pub const PHP_SSO_HOOK: &str = r##"		// CPN-ADMIN-SSO-BEGIN v1
		// Reuse a valid CPN panel OWNER session. The panel verifies the session cookie over
		// loopback (owner only; users, resellers and ACL accounts are refused there). Skipped
		// after an explicit admin sign-out and for cross-site navigations. Fails closed.
		$cpnSCfgPath = '/var/lib/cpn-webmail/admin-bridge.json';
		if ($oActions instanceof self
			&& empty($_COOKIE['cpn_sso_off'])
			&& $oConfig->Get('security', 'allow_admin_panel', true)
			&& \in_array((string) ($_SERVER['HTTP_SEC_FETCH_SITE'] ?? ''), ['', 'same-origin', 'same-site', 'none'], true)
			&& @\file_exists($cpnSCfgPath)
			&& !$oActions->IsAdminLoggined(false)
		) {
			try {
				$cpnSJar = [];
				foreach ($_COOKIE as $cpnSName => $cpnSVal) {
					if (\is_string($cpnSName) && \is_string($cpnSVal)
						&& \preg_match('/^cpn_panel_session(_[0-9]{1,5})?$/', $cpnSName)
						&& \preg_match('/^[A-Za-z0-9_\-]{16,2048}$/', $cpnSVal)
					) {
						$cpnSJar[] = $cpnSName . '=' . $cpnSVal;
					}
				}
				$cpnSCfg = \json_decode((string) @\file_get_contents($cpnSCfgPath), true);
				$cpnSUrl = \is_array($cpnSCfg) ? (string) ($cpnSCfg['url'] ?? '') : '';
				$cpnSToken = \is_array($cpnSCfg) ? (string) ($cpnSCfg['token'] ?? '') : '';
				$cpnSHost = \strtolower((string) \parse_url($cpnSUrl, PHP_URL_HOST));
				if ($cpnSJar && \strlen($cpnSToken)
					&& \in_array($cpnSHost, ['127.0.0.1', 'localhost', '[::1]', '::1'], true)
				) {
					$cpnSBody = \json_encode(['cookie' => \implode('; ', $cpnSJar), 'host' => (string) ($_SERVER['HTTP_HOST'] ?? '')], JSON_UNESCAPED_SLASHES);
					$cpnSEndpoint = \rtrim($cpnSUrl, '/') . '/api/internal/tachyon-admin/session';
					$cpnSHeaders = ['Content-Type: application/json', 'Accept: application/json', 'X-CPN-Bridge-Token: ' . $cpnSToken];
					$cpnSRaw = false;
					if (\function_exists('curl_init')) {
						$cpnSCh = \curl_init($cpnSEndpoint);
						\curl_setopt_array($cpnSCh, [
							CURLOPT_POST => true,
							CURLOPT_POSTFIELDS => $cpnSBody,
							CURLOPT_HTTPHEADER => $cpnSHeaders,
							CURLOPT_RETURNTRANSFER => true,
							CURLOPT_TIMEOUT => 5,
							CURLOPT_CONNECTTIMEOUT => 3,
							CURLOPT_FOLLOWLOCATION => false,
							CURLOPT_SSL_VERIFYPEER => false,
							CURLOPT_SSL_VERIFYHOST => 0,
						]);
						$cpnSRaw = \curl_exec($cpnSCh);
						unset($cpnSCh);
					} else {
						$cpnSCtx = \stream_context_create([
							'http' => ['method' => 'POST', 'header' => \implode("\r\n", $cpnSHeaders), 'content' => $cpnSBody, 'timeout' => 5, 'ignore_errors' => true, 'follow_location' => 0],
							'ssl' => ['verify_peer' => false, 'verify_peer_name' => false],
						]);
						$cpnSRaw = @\file_get_contents($cpnSEndpoint, false, $cpnSCtx);
					}
					$cpnSJson = \is_string($cpnSRaw) ? \json_decode($cpnSRaw, true) : null;
					if (\is_array($cpnSJson) && true === ($cpnSJson['ok'] ?? false)) {
						$oActions->setAdminAuthToken();
					}
				}
			} catch (\Throwable $cpnSEx) {
				// Stay signed out; the normal login form still works.
			}
			unset($cpnSJar, $cpnSCfg, $cpnSToken, $cpnSBody, $cpnSHeaders, $cpnSRaw, $cpnSJson);
		}
		// CPN-ADMIN-SSO-END
"##;

/// Injected at the top of `DoAdminLogout()`.
pub const PHP_LOGOUT_HOOK: &str = r##"		// CPN-ADMIN-SSO-LOGOUT-BEGIN v1
		// Explicit sign-out: do not silently re-admit from the CPN session until this browser
		// session ends or the owner signs in with the form again.
		if (@\file_exists('/var/lib/cpn-webmail/admin-bridge.json') && !\headers_sent()) {
			\setcookie('cpn_sso_off', '1', [
				'path' => '/',
				'httponly' => true,
				'samesite' => 'Lax',
				'secure' => !empty($_SERVER['HTTPS']) || 'https' === \strtolower((string) ($_SERVER['HTTP_X_FORWARDED_PROTO'] ?? '')),
			]);
		}
		// CPN-ADMIN-SSO-LOGOUT-END
"##;
