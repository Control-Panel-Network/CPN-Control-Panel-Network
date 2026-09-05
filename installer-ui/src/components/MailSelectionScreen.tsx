import { ArrowRight, Mail, Mails } from 'lucide-react';
import { siRoundcube, siThunderbird } from 'simple-icons';
import type { MailSystem } from '../types';
import { useI18n } from '../i18n';

interface Props {
  selectedMail: MailSystem | null;
  onSelectMail: (mail: MailSystem) => void;
  onContinue: () => void;
}

function MailIcon({ mail }: { mail: MailSystem }) {
  if (mail === 'roundcube' || mail === 'thunderbird') {
    const icon = mail === 'roundcube' ? siRoundcube : siThunderbird;
    return (
      <svg
        width="44"
        height="44"
        viewBox="0 0 24 24"
        role="img"
        aria-label={icon.title}
        style={{ color: `#${icon.hex}` }}
      >
        <path fill="currentColor" d={icon.path} />
      </svg>
    );
  }
  return mail === 'snappymail' ? (
    <Mails size={44} strokeWidth={1.6} className="text-[#147a62]" />
  ) : (
    <Mail size={44} strokeWidth={1.6} className="text-[#6b7280]" />
  );
}

export function MailSelectionScreen({ selectedMail, onSelectMail, onContinue }: Props) {
  const { t } = useI18n();
  const options: Array<{ id: MailSystem; name: string; description: string; legacy?: boolean }> = [
    {
      id: 'snappymail',
      name: 'SnappyMail',
      description: 'IMAP, SMTP, Sieve, OpenPGP.',
    },
    {
      id: 'roundcube',
      name: 'Roundcube',
      description: 'Plugins, contacts, filters.',
    },
    {
      id: 'thunderbird',
      name: 'Thunderbird',
      description: 'Desktop client (no HTTP service).',
    },
  ];

  return (
    <div className="min-h-screen px-6 md:px-12 py-16 flex flex-col items-center justify-center max-w-6xl mx-auto w-full">
      <div className="text-center mb-12 w-full">
        <h1 className="text-[34px] leading-[1.47] font-semibold tracking-tight text-[#1a1c1d] mb-2">
          {t.selectMailTitle}
        </h1>
        <p className="text-[17px] leading-[1.47] text-[#5f5e60] max-w-2xl mx-auto">
          {t.selectMailIntro}
        </p>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 w-full max-w-4xl">
        {options.map((option) => {
          const selected = selectedMail === option.id;
          return (
            <article
              key={option.id}
              onClick={() => onSelectMail(option.id)}
              className={`utility-card bg-white border rounded-lg p-6 flex flex-col cursor-pointer ${selected ? 'border-[#0066cc] ring-2 ring-[#0066cc]/20' : 'border-[#e0e0e0] hover:border-[#c1c6d5]'}`}
            >
              <div className="h-12 flex items-center mb-5">
                <MailIcon mail={option.id} />
              </div>
              <h2 className="text-[17px] font-semibold mb-1">
                {option.name}
                {option.legacy ? ' (legacy)' : ''}
              </h2>
              <p className="text-[14px] leading-[1.43] text-[#5f5e60] mb-7 flex-1">
                {option.description}
              </p>
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  onSelectMail(option.id);
                }}
                className={`selection-button ${selected ? 'selection-button-active' : ''}`}
                aria-pressed={selected}
              >
                {t.selectLabel}
              </button>
            </article>
          );
        })}
      </div>
      <div className="mt-8 flex flex-col items-center gap-3">
        <button
          type="button"
          onClick={onContinue}
          disabled={!selectedMail}
          className="primary-button min-w-52"
        >
          {t.continueLabel} <ArrowRight size={18} />
        </button>
        <p className="text-sm text-[#667085]">{t.nothingInstallsYet}</p>
      </div>
    </div>
  );
}
