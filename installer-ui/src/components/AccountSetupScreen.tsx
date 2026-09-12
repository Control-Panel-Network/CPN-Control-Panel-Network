import { useMemo, useState } from "react";
import { ArrowRight, Eye, EyeOff, RefreshCw } from "lucide-react";
import { setupAccount } from "../api";
import { useI18n } from "../i18n";
import type { PasswordPolicy } from "../types";

interface Props {
  initialPolicy: PasswordPolicy;
  language: string;
  onCompleted: () => void;
}

type TlsMode = "starttls" | "tls" | "none";

const PASSWORD_LENGTH = 20;
const UPPER = "ABCDEFGHJKLMNPQRSTUVWXYZ";
const LOWER = "abcdefghijkmnopqrstuvwxyz";
const DIGITS = "23456789";
const SYMBOLS = "!@#$%&*+-=?_";

function randomIndex(max: number): number {
  const value = new Uint32Array(1);
  window.crypto.getRandomValues(value);
  return value[0] % max;
}

function generateAsciiPassword(): string {
  const all = UPPER + LOWER + DIGITS + SYMBOLS;
  const chars = [
    UPPER[randomIndex(UPPER.length)],
    LOWER[randomIndex(LOWER.length)],
    DIGITS[randomIndex(DIGITS.length)],
    SYMBOLS[randomIndex(SYMBOLS.length)],
  ];
  while (chars.length < PASSWORD_LENGTH) {
    chars.push(all[randomIndex(all.length)]);
  }
  for (let index = chars.length - 1; index > 0; index -= 1) {
    const swap = randomIndex(index + 1);
    [chars[index], chars[swap]] = [chars[swap], chars[index]];
  }
  return chars.join("");
}

export function AccountSetupScreen({
  initialPolicy,
  language,
  onCompleted,
}: Props) {
  const { t, locale } = useI18n();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [passwordConfirm, setPasswordConfirm] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [recoveryEmail, setRecoveryEmail] = useState("");
  const policy: PasswordPolicy = {
    ...initialPolicy,
    min_length: Math.max(12, initialPolicy.min_length),
    require_special: true,
    require_uppercase: true,
    require_number: true,
  };
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [smtpEnabled, setSmtpEnabled] = useState(false);
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState(587);
  const [smtpTls, setSmtpTls] = useState<TlsMode>("starttls");
  const [smtpFrom, setSmtpFrom] = useState("");
  const [smtpUser, setSmtpUser] = useState("");
  const [smtpPassword, setSmtpPassword] = useState("");
  const [sendUsernameEmail, setSendUsernameEmail] = useState(false);
  const [includePasswordInEmail, setIncludePasswordInEmail] = useState(false);

  const canSubmit = useMemo(() => {
    if (!recoveryEmail.trim()) return false;
    if (!password || password !== passwordConfirm) return false;
    if (smtpEnabled && (!smtpHost.trim() || !smtpFrom.trim())) return false;
    return true;
  }, [
    password,
    passwordConfirm,
    recoveryEmail,
    smtpEnabled,
    smtpHost,
    smtpFrom,
  ]);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const result = await setupAccount({
        username: username.trim(),
        password,
        generate_password: false,
        recovery_email: recoveryEmail.trim(),
        password_policy: policy,
        language: locale || language,
        smtp: smtpEnabled
          ? {
              host: smtpHost.trim(),
              port: smtpPort,
              tls_mode: smtpTls,
              from_address: smtpFrom.trim(),
              username: smtpUser.trim() || undefined,
              password: smtpPassword || undefined,
            }
          : undefined,
        send_username_email: sendUsernameEmail,
        include_password_in_email: includePasswordInEmail,
      });
      if (result.setup_email_error && !result.setup_email_sent) {
        setError(result.setup_email_error);
      }
      onCompleted();
    } catch (err) {
      setError(err instanceof Error ? err.message : t.accountError);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="min-h-screen px-6 py-12 flex flex-col items-center">
      <div className="w-full max-w-xl">
        <p className="eyebrow">{t.accountEyebrow}</p>
        <h1 className="text-[34px] leading-[1.2] font-semibold tracking-tight mt-2">
          {t.accountTitle}
        </h1>
        <p className="text-[17px] text-[#5f5e60] mt-3">{t.accountIntro}</p>

        <div className="panel mt-8 p-6 space-y-5">
          <label className="block">
            <span className="text-sm font-semibold">{t.usernameLabel}</span>
            <input
              className="field-input mt-2"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              placeholder={t.usernamePlaceholder}
              autoComplete="username"
            />
            <span className="field-hint">{t.usernameHint}</span>
          </label>

          <div className="space-y-4">
            <label className="block">
              <span className="text-sm font-semibold">{t.passwordLabel}</span>
              <span className="password-input-row mt-2">
                <input
                  className="field-input"
                  type={showPassword ? "text" : "password"}
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  autoComplete="new-password"
                />
                <button
                  type="button"
                  className="password-icon-button"
                  aria-label={showPassword ? "Hide password" : "Show password"}
                  onClick={() => setShowPassword((value) => !value)}
                >
                  {showPassword ? <EyeOff size={18} /> : <Eye size={18} />}
                </button>
              </span>
              <span className="field-hint">{t.passwordHint}</span>
            </label>
            <label className="block">
              <span className="text-sm font-semibold">
                {t.passwordConfirmLabel}
              </span>
              <input
                className="field-input mt-2"
                type={showPassword ? "text" : "password"}
                value={passwordConfirm}
                onChange={(event) => setPasswordConfirm(event.target.value)}
                autoComplete="new-password"
              />
              {passwordConfirm && password !== passwordConfirm && (
                <span className="field-error">{t.passwordMismatch}</span>
              )}
            </label>
            <button
              type="button"
              className="secondary-button"
              onClick={() => {
                const generated = generateAsciiPassword();
                setPassword(generated);
                setPasswordConfirm(generated);
                setShowPassword(true);
              }}
            >
              <RefreshCw size={16} /> {t.generatePassword}
            </button>
          </div>

          <label className="block">
            <span className="text-sm font-semibold">{t.emailLabel}</span>
            <input
              className="field-input mt-2"
              type="email"
              value={recoveryEmail}
              onChange={(event) => setRecoveryEmail(event.target.value)}
              placeholder={t.emailPlaceholder}
              autoComplete="email"
            />
            <span className="field-hint">{t.emailHint}</span>
          </label>

          <fieldset className="border border-[#e5e8ec] rounded-2xl p-4 space-y-3">
            <legend className="px-1 text-sm font-semibold">
              {t.smtpOptionalTitle}
            </legend>
            <p className="text-sm text-[#5f5e60]">{t.smtpOptionalHint}</p>
            <label className="flex items-center justify-between gap-4 py-1">
              <span>{t.smtpEnableLabel}</span>
              <input
                type="checkbox"
                checked={smtpEnabled}
                onChange={(event) => {
                  setSmtpEnabled(event.target.checked);
                  if (!event.target.checked) {
                    setSendUsernameEmail(false);
                    setIncludePasswordInEmail(false);
                  }
                }}
              />
            </label>
            {smtpEnabled && (
              <>
                <label className="block">
                  <span className="text-sm font-semibold">
                    {t.smtpHostLabel}
                  </span>
                  <input
                    className="field-input mt-2"
                    value={smtpHost}
                    onChange={(event) => setSmtpHost(event.target.value)}
                    placeholder="smtp.example.com"
                    autoComplete="off"
                  />
                </label>
                <div className="grid grid-cols-2 gap-3">
                  <label className="block">
                    <span className="text-sm font-semibold">
                      {t.smtpPortLabel}
                    </span>
                    <input
                      className="field-input mt-2"
                      type="number"
                      min={1}
                      max={65535}
                      value={smtpPort}
                      onChange={(event) =>
                        setSmtpPort(Number(event.target.value) || 587)
                      }
                    />
                  </label>
                  <label className="block">
                    <span className="text-sm font-semibold">
                      {t.smtpTlsLabel}
                    </span>
                    <select
                      className="field-input mt-2"
                      value={smtpTls}
                      onChange={(event) =>
                        setSmtpTls(event.target.value as TlsMode)
                      }
                    >
                      <option value="starttls">{t.smtpTlsStarttls}</option>
                      <option value="tls">{t.smtpTlsTls}</option>
                      <option value="none">{t.smtpTlsNone}</option>
                    </select>
                  </label>
                </div>
                <label className="block">
                  <span className="text-sm font-semibold">
                    {t.smtpFromLabel}
                  </span>
                  <input
                    className="field-input mt-2"
                    type="email"
                    value={smtpFrom}
                    onChange={(event) => setSmtpFrom(event.target.value)}
                    placeholder="noreply@example.com"
                    autoComplete="off"
                  />
                </label>
                <label className="block">
                  <span className="text-sm font-semibold">
                    {t.smtpUserLabel}
                  </span>
                  <input
                    className="field-input mt-2"
                    value={smtpUser}
                    onChange={(event) => setSmtpUser(event.target.value)}
                    autoComplete="off"
                  />
                </label>
                <label className="block">
                  <span className="text-sm font-semibold">
                    {t.smtpPasswordLabel}
                  </span>
                  <input
                    className="field-input mt-2"
                    type="password"
                    value={smtpPassword}
                    onChange={(event) => setSmtpPassword(event.target.value)}
                    autoComplete="new-password"
                  />
                </label>
                <label className="flex items-start justify-between gap-4 py-1">
                  <span>
                    <span className="block">{t.smtpSendUsernameLabel}</span>
                    <span className="field-hint">{t.smtpSendUsernameHint}</span>
                  </span>
                  <input
                    type="checkbox"
                    checked={sendUsernameEmail}
                    onChange={(event) =>
                      setSendUsernameEmail(event.target.checked)
                    }
                  />
                </label>
                <label className="flex items-start justify-between gap-4 py-1">
                  <span>
                    <span className="block">{t.smtpIncludePasswordLabel}</span>
                    <span className="field-hint">
                      {t.smtpIncludePasswordHint}
                    </span>
                  </span>
                  <input
                    type="checkbox"
                    checked={includePasswordInEmail}
                    disabled={!sendUsernameEmail}
                    onChange={(event) =>
                      setIncludePasswordInEmail(event.target.checked)
                    }
                  />
                </label>
              </>
            )}
          </fieldset>

          <p className="password-policy-note">
            {t.policyTitle}: {t.policyMinLength} {policy.min_length} ·{" "}
            {t.policyRequireUpper} · {t.policyRequireNumber} ·{" "}
            {t.policyRequireSpecial}.
          </p>

          {error && <p className="error-box">{error}</p>}

          <button
            type="button"
            className="primary-button w-full"
            disabled={!canSubmit || busy}
            onClick={() => {
              void submit();
            }}
          >
            {busy ? t.accountSaving : t.saveAccount} <ArrowRight size={18} />
          </button>
        </div>
      </div>
    </section>
  );
}
