import type { InstallerStatus } from '../types';
import { useI18n } from '../i18n';
import { LanguageSelector } from '../i18n/LanguageSelector';

export function PreparingScreen({ status }: { status: InstallerStatus }) {
  const { t } = useI18n();
  return (
    <section className="min-h-screen grid place-items-center px-6">
      <div className="text-center max-w-xl w-full">
        <div className="flex justify-end mb-6"><LanguageSelector /></div>
        <p className="eyebrow">{t.preparingEyebrow}</p>
        <h1 className="text-4xl sm:text-5xl font-semibold tracking-[-0.04em] mt-3">
          {t.preparingTitle}
        </h1>
        <p className="text-[#667085] text-lg mt-4">{status.message || t.initialMessage}</p>
        <div className="spinner mx-auto mt-10" role="status" aria-label={t.preparingAria} />
        {status.error && <p className="error-box mt-8">{status.error}</p>}
      </div>
    </section>
  );
}
