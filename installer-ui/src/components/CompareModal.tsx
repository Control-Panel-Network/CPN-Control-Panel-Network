import { Check, X } from 'lucide-react';
import type { ServerEngine } from '../types';
import { useI18n } from '../i18n';
import { ServerBrandIcon } from './ServerBrandIcon';

interface Props {
  isOpen: boolean;
  selectedServer: ServerEngine | null;
  onClose: () => void;
  onSelectServer: (server: ServerEngine) => void;
}

const optionIds: ServerEngine[] = ['openlitespeed', 'nginx', 'caddy'];
const optionNames: Record<ServerEngine, string> = {
  openlitespeed: 'OpenLiteSpeed',
  nginx: 'Nginx',
  caddy: 'Caddy',
};

export function CompareModal({ isOpen, selectedServer, onClose, onSelectServer }: Props) {
  const { t } = useI18n();
  if (!isOpen) return null;

  const metaFor = (id: ServerEngine) => {
    if (id === 'openlitespeed') {
      return { description: t.compareOpenlitespeedDesc, features: t.compareOpenlitespeedFeatures };
    }
    if (id === 'nginx') {
      return { description: t.compareNginxDesc, features: t.compareNginxFeatures };
    }
    return { description: t.compareCaddyDesc, features: t.compareCaddyFeatures };
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs"
      onClick={onClose}
    >
      <div
        className="bg-white rounded-2xl max-w-3xl w-full p-6 md:p-8 shadow-2xl border border-[#e0e0e0] max-h-[90vh] overflow-y-auto"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="flex items-start justify-between gap-5 pb-5 border-b border-[#e8e8ea]">
          <div>
            <h2 className="text-[21px] font-semibold">{t.compareTitle}</h2>
            <p className="text-sm text-[#5f5e60]">{t.compareIntro}</p>
          </div>
          <button type="button" onClick={onClose} className="icon-button" aria-label={t.closeLabel}>
            <X size={20} />
          </button>
        </header>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 py-6">
          {optionIds.map((id) => {
            const selected = selectedServer === id;
            const meta = metaFor(id);
            return (
              <article key={id} className={`compare-card ${selected ? 'compare-card-active' : ''}`}>
                <div className="h-11 flex items-center">
                  <ServerBrandIcon server={id} />
                </div>
                <h3 className="font-semibold mt-3">{optionNames[id]}</h3>
                <p className="text-xs leading-5 text-[#5f5e60] mt-2">{meta.description}</p>
                <ul className="text-[13px] space-y-2 my-5 flex-1">
                  {meta.features.map((feature) => (
                    <li key={feature} className="flex gap-2">
                      <Check size={16} className="text-emerald-600 shrink-0" />
                      {feature}
                    </li>
                  ))}
                </ul>
                <button
                  type="button"
                  onClick={() => {
                    onSelectServer(id);
                    onClose();
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
      </div>
    </div>
  );
}
