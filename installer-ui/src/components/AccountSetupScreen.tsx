import { useMemo, useState } from 'react';
import { ArrowRight, KeyRound, RefreshCw } from 'lucide-react';
import { setupAccount } from '../api';
import { useI18n } from '../i18n';
import type { PasswordPolicy } from '../types';

interface Props {
  initialPolicy: PasswordPolicy;
  language: string;
  onCompleted: (generatedPassword?: string | null) => void;
}

export function AccountSetupScreen({ initialPolicy, language, onCompleted }: Props) {
  const { t, locale } = useI18n();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [generate, setGenerate] = useState(false);
  const [recoveryEmail, setRecoveryEmail] = useState('');
  const [policy, setPolicy] = useState<PasswordPolicy>(initialPolicy);
  const [generatedPreview, setGeneratedPreview] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const canSubmit = useMemo(() => {
    if (!recoveryEmail.trim()) return false;
    if (!generate && !password) return false;
    return true;
  }, [generate, password, recoveryEmail]);

  const submit = async () => {
    if (generatedPreview) {
      onCompleted(generatedPreview);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await setupAccount({
        username: username.trim(),
        password: generate ? undefined : password,
        generate_password: generate,
        recovery_email: recoveryEmail.trim(),
        password_policy: policy,
        language: locale || language,
      });
      if (result.generated_password) {
        setGeneratedPreview(result.generated_password);
        return;
      }
      onCompleted(null);
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
        <h1 className="text-[34px] leading-[1.2] font-semibold tracking-tight mt-2">{t.accountTitle}</h1>
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

          <div>
            <div className="flex flex-wrap gap-2 mb-3">
              <button
                type="button"
                className={!generate ? 'language-chip language-chip-active' : 'language-chip'}
                onClick={() => setGenerate(false)}
              >
                {t.useOwnPassword}
              </button>
              <button
                type="button"
                className={generate ? 'language-chip language-chip-active' : 'language-chip'}
                onClick={() => {
                  setGenerate(true);
                  setPassword('');
                }}
              >
                <RefreshCw size={14} /> {t.generatePassword}
              </button>
            </div>
            {!generate && (
              <label className="block">
                <span className="text-sm font-semibold">{t.passwordLabel}</span>
                <input
                  className="field-input mt-2"
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  autoComplete="new-password"
                />
                <span className="field-hint">{t.passwordHint}</span>
              </label>
            )}
            {generate && (
              <p className="text-sm text-[#5f5e60] flex items-center gap-2">
                <KeyRound size={16} /> {t.passwordHint}
              </p>
            )}
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

          <fieldset className="border border-[#e5e8ec] rounded-2xl p-4">
            <legend className="px-1 text-sm font-semibold">{t.policyTitle}</legend>
            <label className="flex items-center justify-between gap-4 py-2">
              <span>{t.policyMinLength}</span>
              <input
                className="field-input w-24"
                type="number"
                min={4}
                max={128}
                value={policy.min_length}
                onChange={(event) =>
                  setPolicy({ ...policy, min_length: Number(event.target.value) || 8 })
                }
              />
            </label>
            <label className="flex items-center justify-between gap-4 py-2">
              <span>{t.policyRequireSpecial}</span>
              <input
                type="checkbox"
                checked={policy.require_special}
                onChange={(event) =>
                  setPolicy({ ...policy, require_special: event.target.checked })
                }
              />
            </label>
            <label className="flex items-center justify-between gap-4 py-2">
              <span>{t.policyRequireUpper}</span>
              <input
                type="checkbox"
                checked={policy.require_uppercase}
                onChange={(event) =>
                  setPolicy({ ...policy, require_uppercase: event.target.checked })
                }
              />
            </label>
            <label className="flex items-center justify-between gap-4 py-2">
              <span>{t.policyRequireNumber}</span>
              <input
                type="checkbox"
                checked={policy.require_number}
                onChange={(event) =>
                  setPolicy({ ...policy, require_number: event.target.checked })
                }
              />
            </label>
          </fieldset>

          {generatedPreview && (
            <div className="generated-box">
              <p className="text-sm font-semibold mb-2">{t.generatedPasswordNote}</p>
              <code className="break-all">{generatedPreview}</code>
              <button
                type="button"
                className="secondary-button mt-3"
                onClick={async () => {
                  await navigator.clipboard.writeText(generatedPreview);
                  setCopied(true);
                }}
              >
                {copied ? t.copied : t.copyPassword}
              </button>
            </div>
          )}

          {error && <p className="error-box">{error}</p>}

          <button
            type="button"
            className="primary-button w-full"
            disabled={!canSubmit || busy}
            onClick={() => {
              void submit();
            }}
          >
            {busy ? t.accountSaving : generatedPreview ? t.continueLabel : t.saveAccount}{' '}
            <ArrowRight size={18} />
          </button>
        </div>
      </div>
    </section>
  );
}
