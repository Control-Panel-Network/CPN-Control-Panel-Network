import { ArrowRight, CircleHelp } from 'lucide-react';
import type { ServerEngine } from '../types';
import { ServerBrandIcon } from './ServerBrandIcon';
import { useI18n } from '../i18n';
import { LanguageSelector } from '../i18n/LanguageSelector';

interface Props {
  selectedServer: ServerEngine | null;
  onSelectServer: (server: ServerEngine) => void;
  onContinue: () => void;
  onOpenCompare: () => void;
}

export function ServerSelectionScreen({
  selectedServer,
  onSelectServer,
  onContinue,
  onOpenCompare,
}: Props) {
  const { t } = useI18n();
  const servers: Array<{ id: ServerEngine; name: string; description: string }> = [
    { id: 'openlitespeed', name: 'OpenLiteSpeed', description: t.serverOpenlitespeedDesc },
    { id: 'nginx', name: 'Nginx', description: t.serverNginxDesc },
    { id: 'caddy', name: 'Caddy', description: t.serverCaddyDesc },
  ];

  return (
    <div className="min-h-screen px-6 md:px-12 py-16 flex flex-col items-center justify-center max-w-6xl mx-auto w-full">
      <div className="w-full flex justify-end mb-4"><LanguageSelector /></div>
      <div className="text-center mb-12 w-full">
        <h1 className="text-[34px] leading-[1.47] font-semibold tracking-tight text-[#1a1c1d] mb-2">
          {t.selectServerTitle}
        </h1>
        <p className="text-[17px] leading-[1.47] text-[#5f5e60] max-w-2xl mx-auto">
          {t.selectServerIntro}
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6 w-full max-w-5xl">
        {servers.map((server) => {
          const selected = selectedServer === server.id;
          return (
            <article
              key={server.id}
              onClick={() => onSelectServer(server.id)}
              className={`utility-card bg-white border rounded-lg p-6 flex flex-col cursor-pointer transition-all ${selected ? 'border-[#0066cc] ring-2 ring-[#0066cc]/20' : 'border-[#e0e0e0] hover:border-[#c1c6d5]'}`}
            >
              <div className="mb-6 h-12 flex items-center">
                <ServerBrandIcon server={server.id} />
              </div>
              <h2 className="text-[17px] font-semibold text-[#1a1c1d] mb-1">{server.name}</h2>
              <p className="text-[14px] leading-[1.43] text-[#5f5e60] mb-8 flex-1">{server.description}</p>
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  onSelectServer(server.id);
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

      <button type="button" onClick={onOpenCompare} className="compare-link mt-8">
        <CircleHelp size={17} /> {t.compareLink}
      </button>

      <div className="mt-8 flex flex-col items-center gap-3">
        <button
          type="button"
          onClick={onContinue}
          disabled={!selectedServer}
          className="primary-button min-w-52"
        >
          {t.continueLabel} <ArrowRight size={18} />
        </button>
        <p className="text-sm text-[#667085]">{t.nothingInstallsYet}</p>
      </div>
    </div>
  );
}
