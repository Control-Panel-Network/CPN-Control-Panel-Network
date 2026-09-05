import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import en from './locales/en';
import es from './locales/es';
import nb from './locales/nb';
import {
  normalizeLocale,
  type LocaleCode,
  type LocaleMessages,
  SUPPORTED_LOCALES,
} from './types';

const CATALOG: Record<LocaleCode, LocaleMessages> = { en, es, nb };
const STORAGE_KEY = 'cpn-installer-locale';

function readStoredLocale(): LocaleCode {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored) return normalizeLocale(stored);
  } catch {
    // Fall back to the browser when storage is unavailable.
  }
  const preferred = navigator.languages.find((language) => /^(es|en|nb|nn|no)(-|$)/i.test(language));
  return normalizeLocale(preferred || navigator.language);
}

interface I18nContextValue {
  locale: LocaleCode;
  t: LocaleMessages;
  setLocale: (locale: LocaleCode) => void;
  locales: LocaleCode[];
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({
  children,
  initialLocale,
  onLocaleChange,
}: {
  children: ReactNode;
  initialLocale?: string | null;
  onLocaleChange?: (locale: LocaleCode) => void;
}) {
  const [locale, setLocaleState] = useState<LocaleCode>(() =>
    normalizeLocale(initialLocale ?? readStoredLocale()),
  );
  useEffect(() => { document.documentElement.lang = locale; }, [locale]);

  const setLocale = useCallback(
    (next: LocaleCode) => {
      const normalized = normalizeLocale(next);
      setLocaleState(normalized);
      try {
        window.localStorage.setItem(STORAGE_KEY, normalized);
      } catch {
        // Local storage may be unavailable in locked-down browsers.
      }
      onLocaleChange?.(normalized);
    },
    [onLocaleChange],
  );

  const value = useMemo(
    () => ({
      locale,
      t: CATALOG[locale],
      setLocale,
      locales: SUPPORTED_LOCALES,
    }),
    [locale, setLocale],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const value = useContext(I18nContext);
  if (!value) {
    throw new Error('useI18n must be used inside I18nProvider');
  }
  return value;
}

/** Compatibility alias used by older screens. */
export const useLocale = useI18n;

export function formatMessage(template: string, vars: Record<string, string>): string {
  return Object.entries(vars).reduce(
    (text, [key, value]) => text.replaceAll(`{${key}}`, value),
    template,
  );
}

export { normalizeLocale, SUPPORTED_LOCALES };
export type { LocaleCode, LocaleMessages };
