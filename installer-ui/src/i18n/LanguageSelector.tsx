import { useEffect, useRef, useState } from 'react';
import { Globe2, Monitor, Moon, Sun, Check } from 'lucide-react';
import { useI18n, type LocaleCode } from '../i18n';

const LABELS: Record<LocaleCode, string> = {
  en: 'English',
  es: 'Español',
  nb: 'Norsk',
};

export function LanguageSelector({ className = '' }: { className?: string }) {
  const { locale, setLocale, locales, t } = useI18n();
  const [menu, setMenu] = useState<'language' | 'theme' | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const [theme, setTheme] = useState<'system' | 'light' | 'dark'>(() => {
    try { const saved = localStorage.getItem('cpn-installer-theme'); return saved === 'light' || saved === 'dark' ? saved : 'system'; } catch { return 'system'; }
  });
  useEffect(() => {
    const query = matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      document.documentElement.dataset.theme = theme === 'system' ? (query.matches ? 'dark' : 'light') : theme;
      try { localStorage.setItem('cpn-installer-theme', theme); } catch { /* Storage is optional. */ }
    };
    apply(); query.addEventListener('change', apply);
    return () => query.removeEventListener('change', apply);
  }, [theme]);
  useEffect(() => {
    const close = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) setMenu(null); };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, []);
  const names = locale === 'es' ? ['Sistema', 'Claro', 'Oscuro'] : locale === 'nb' ? ['System', 'Lys', 'Mørk'] : ['System', 'Light', 'Dark'];
  const ThemeIcon = theme === 'system' ? Monitor : theme === 'dark' ? Moon : Sun;
  return <div ref={root} className={`appearance-toolbar ${className}`} onKeyDown={(event) => { if (event.key === 'Escape') { setMenu(null); root.current?.querySelector<HTMLButtonElement>('button')?.focus(); } }}>
    <button type="button" aria-label={t.languageLabel} aria-expanded={menu === 'language'} onClick={() => setMenu(menu === 'language' ? null : 'language')}><Globe2 size={18} /><span>{locale.toUpperCase()}</span></button>
    <button type="button" aria-label={locale === 'es' ? 'Apariencia' : 'Appearance'} aria-expanded={menu === 'theme'} onClick={() => setMenu(menu === 'theme' ? null : 'theme')}><ThemeIcon size={18} /></button>
    {menu && <div className="appearance-popover">
      {menu === 'language' ? locales.map((code) => <button key={code} type="button" aria-pressed={locale === code} onClick={() => { setLocale(code); setMenu(null); }}>{LABELS[code]}{locale === code && <Check size={16} />}</button>) : (['system', 'light', 'dark'] as const).map((mode, index) => <button key={mode} type="button" aria-pressed={theme === mode} onClick={() => { setTheme(mode); setMenu(null); }}>{names[index]}{theme === mode && <Check size={16} />}</button>)}
    </div>}
  </div>;
}
