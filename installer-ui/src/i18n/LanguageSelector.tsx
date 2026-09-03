import { useI18n, type LocaleCode } from '../i18n';

const LABELS: Record<LocaleCode, string> = {
  en: 'English',
  es: 'Español',
  nb: 'Norsk',
};

export function LanguageSelector({ className = '' }: { className?: string }) {
  const { locale, setLocale, locales, t } = useI18n();

  return (
    <label className={`language-selector ${className}`.trim()}>
      <span className="sr-only">{t.languageLabel}</span>
      <select
        value={locale}
        onChange={(event) => setLocale(event.target.value as LocaleCode)}
        aria-label={t.languageLabel}
      >
        {locales.map((code) => (
          <option key={code} value={code}>
            {LABELS[code]}
          </option>
        ))}
      </select>
    </label>
  );
}
