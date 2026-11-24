use super::*;

const SAMPLE: &str = "<?php\nnamespace Tachyon;\nuse MailSo\\Exceptions\\ClientException;\nclass ActionsAdmin\n{\n\tpublic function DoAdminLogin() : array\n\t{\n\t\t$sLogin = trim($this->GetActionParam('Login', ''));\n\t\tif (!ok) {\n\t\t\t$this->LoggerAuthHelper(null, $sLogin, true);\n\t\t\t$this->loginErrorDelay();\n\t\t\tthrow new ClientException(Notifications::AuthError);\n\t\t}\n\t\t$sToken = $this->setAdminAuthToken();\n\t\treturn $this->DefaultResponse($sToken ? $this->AppData(true) : false);\n\t}\n\n\tpublic function DoAdminLogout() : array\n\t{\n\t\treturn [];\n\t}\n\n\tpublic static function AdminAppData(Actions $oActions, array &$aResult): void\n\t{\n\t\t$oConfig = $oActions->Config();\n\t\t$aResult['Admin'] = [];\n\t\t$aResult['Auth'] = $oActions->IsAdminLoggined(false);\n\t}\n}\n";

#[test]
fn hooks_are_injected_in_the_right_functions_only_once() {
    let (out, outcome) = patched_source(SAMPLE).expect("supported");
    assert_eq!(outcome, PatchOutcome::Patched);
    assert!(out.contains("CPN-ADMIN-BRIDGE-BEGIN v3"));
    assert!(out.contains("CPN-ADMIN-SSO-BEGIN v3"));
    assert!(out.contains("CPN-ADMIN-SSO-LOGOUT-BEGIN v1"));
    assert!(out.contains("/api/internal/tachyon-admin/verify"));
    assert!(out.contains("/api/internal/tachyon-admin/session"));
    assert!(out.contains("'Cookie: ' . $cpnSCookie"));
    let login_pos = out.find("function DoAdminLogin()").unwrap();
    let login_hook = out.find("CPN-ADMIN-BRIDGE-BEGIN").unwrap();
    let logout_pos = out.find("function DoAdminLogout()").unwrap();
    let logout_hook = out.find("CPN-ADMIN-SSO-LOGOUT-BEGIN").unwrap();
    let app_pos = out.find("function AdminAppData(").unwrap();
    let sso_hook = out.find("// CPN-ADMIN-SSO-BEGIN").unwrap();
    let auth_line = out.find("$aResult['Auth'] =").unwrap();
    assert!(login_pos < login_hook && login_hook < logout_pos);
    assert!(logout_pos < logout_hook && logout_hook < app_pos);
    assert!(app_pos < sso_hook && sso_hook < auth_line);
    // Original local check stays below the hook as the fallback when the config is absent.
    assert!(out.contains("$sLogin = trim("));
    let (again, outcome2) = patched_source(&out).unwrap();
    assert_eq!(outcome2, PatchOutcome::AlreadyCurrent);
    assert_eq!(again, out);
}

#[test]
fn old_v1_login_hook_is_upgraded_in_place() {
    let v1 = SAMPLE.replacen(
        "\t{\n\t\t$sLogin",
        "\t{\n\t\t// CPN-ADMIN-BRIDGE-BEGIN v1\n\t\told();\n\t\t// CPN-ADMIN-BRIDGE-END\n\t\t$sLogin",
        1,
    );
    let (out, outcome) = patched_source(&v1).unwrap();
    assert_eq!(outcome, PatchOutcome::Patched);
    assert!(!out.contains("BRIDGE-BEGIN v1"));
    assert!(!out.contains("old();"));
    assert_eq!(out.matches("CPN-ADMIN-BRIDGE-BEGIN").count(), 1);
}

#[test]
fn sso_hooks_are_optional_when_upstream_layout_differs() {
    let no_app = SAMPLE
        .split("\n\tpublic static function AdminAppData")
        .next()
        .unwrap();
    let raw = format!("{no_app}\n}}\n");
    let (out, _) = patched_source(&raw).unwrap();
    assert!(out.contains("CPN-ADMIN-BRIDGE-BEGIN v3"));
    assert!(!out.contains("// CPN-ADMIN-SSO-BEGIN"));
    assert_eq!(
        patched_source(&out).unwrap().1,
        PatchOutcome::AlreadyCurrent
    );
}

#[test]
fn strip_restores_original() {
    let (out, _) = patched_source(SAMPLE).unwrap();
    assert_eq!(strip_bridge_block(&out), SAMPLE);
}

#[test]
fn unsupported_layout_is_not_patched() {
    assert!(patched_source("<?php\nclass X { function Other() {} }\n").is_none());
    let no_tokens =
        "<?php\nclass X {\n\tpublic function DoAdminLogin() : array\n\t{\n\t\treturn [];\n\t}\n}\n";
    assert!(patched_source(no_tokens).is_none());
}

#[test]
fn login_hook_fails_closed_and_never_logs_password() {
    let hook = PHP_LOGIN_HOOK;
    // Config present but unreadable or invalid must deny, not fall through to local auth.
    assert!(hook.contains("if (!$cpnOk)"));
    assert!(hook.contains("throw new ClientException(Notifications::AuthError, null,"));
    // Only loopback targets are allowed (TLS verify is off for loopback only).
    assert!(hook.contains("'127.0.0.1', 'localhost'"));
    assert!(hook.contains("unset($cpnPass"));
    assert!(!hook.contains("error_log"));
    assert!(!hook.contains("curl_close"));
    // Distinct, fixed messages for the authenticator step.
    assert!(hook.contains("'mfa_required' => 'Authenticator code required."));
    assert!(hook.contains("'panel_updating'"));
}

#[test]
fn sso_hook_is_owner_session_only_and_fail_closed() {
    let hook = PHP_SSO_HOOK;
    assert!(hook.contains("'127.0.0.1', 'localhost'"));
    // Only panel session cookies are forwarded, nothing else from the browser.
    assert!(hook.contains("cpn_panel_session"));
    // Cross-site navigations never auto-admit; explicit sign-out disables SSO.
    assert!(hook.contains("HTTP_SEC_FETCH_SITE"));
    assert!(hook.contains("cpn_sso_off"));
    // $oActions can be the base action class: never call the private setAdminAuthToken().
    assert!(!hook.contains("setAdminAuthToken"));
    assert!(!hook.contains("instanceof self"));
    assert!(hook.contains("SessionAdminKey"));
    assert!(hook.contains("Cookie: "));
    assert!(hook.contains("catch (\\Throwable"));
    assert!(!hook.contains("error_log"));
    assert!(PHP_LOGOUT_HOOK.contains("cpn_sso_off"));
    assert!(PHP_LOGOUT_HOOK.contains("'httponly' => true"));
}

#[test]
fn config_roundtrip_keeps_token_and_updates_url() {
    let dir = std::env::temp_dir().join(format!(
        "cpn-bridge-cfg-{}-{}",
        std::process::id(),
        crate::account::now_unix()
    ));
    let path = dir.join("admin-bridge.json");
    assert!(write_config_if_needed(&path, "http://127.0.0.1:2086").unwrap());
    let first: BridgeConfig =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(first.token.len(), 64);
    assert!(!write_config_if_needed(&path, "http://127.0.0.1:2086").unwrap());
    assert!(write_config_if_needed(&path, "http://127.0.0.1:2090").unwrap());
    let second: BridgeConfig =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(first.token, second.token);
    assert_eq!(second.url, "http://127.0.0.1:2090");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn webmail_only_update_is_detected_and_hooks_are_restored() {
    // A webmail-only update unpacks a fresh upstream tree (new version directory) and
    // never runs CPN. The heal must notice and re-apply every hook there.
    let root = std::env::temp_dir().join(format!(
        "cpn-bridge-upgrade-{}-{}",
        std::process::id(),
        crate::account::now_unix()
    ));
    let old_dir = root.join("v/4.2.3/app/libraries/Tachyon");
    let new_dir = root.join("v/4.2.4/app/libraries/Tachyon");
    std::fs::create_dir_all(&old_dir).unwrap();
    std::fs::create_dir_all(&new_dir).unwrap();
    let (patched, _) = patched_source(SAMPLE).unwrap();
    std::fs::write(old_dir.join("ActionsAdmin.php"), &patched).unwrap();
    // Upgrade: pristine upstream copy lands in the new version directory.
    std::fs::write(new_dir.join("ActionsAdmin.php"), SAMPLE).unwrap();

    let mut found = Vec::new();
    walk_for_actions_admin(&root, 0, &mut found);
    found.sort();
    assert_eq!(found.len(), 2);
    let needs: Vec<bool> = found
        .iter()
        .map(|p| file_needs_heal(&std::fs::read_to_string(p).unwrap()))
        .collect();
    // 4.2.3 (already hooked) is fine; 4.2.4 (fresh upstream) needs the heal.
    assert_eq!(needs, vec![false, true]);

    // Re-apply, exactly what ensure_admin_bridge does per file.
    let raw = std::fs::read_to_string(&found[1]).unwrap();
    let (fixed, outcome) = patched_source(&raw).unwrap();
    assert_eq!(outcome, PatchOutcome::Patched);
    std::fs::write(&found[1], &fixed).unwrap();
    assert!(!file_needs_heal(&fixed));
    for hook in ["BRIDGE-BEGIN v3", "SSO-BEGIN v3", "SSO-LOGOUT-BEGIN v1"] {
        assert!(fixed.contains(hook), "{hook} missing after re-apply");
    }
    // Unsupported layouts never trigger an endless heal loop.
    assert!(!file_needs_heal("<?php\nclass X {}\n"));
    let _ = std::fs::remove_dir_all(&root);
}
