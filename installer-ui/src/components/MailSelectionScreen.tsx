import { ArrowRight, CloudRain, Mails } from 'lucide-react';
import { siRoundcube, siThunderbird } from 'simple-icons';
import type { MailSystem } from '../types';

interface Props { selectedMail: MailSystem | null; onSelectMail: (mail: MailSystem) => void; onContinue: () => void }

const options: Array<{ id: MailSystem; name: string; description: string }> = [
  { id: 'snappymail', name: 'SnappyMail', description: 'Webmail moderno, rápido y ligero con soporte IMAP, SMTP, Sieve y OpenPGP.' },
  { id: 'rainloop', name: 'RainLoop', description: 'Webmail ligero compatible con IMAP y SMTP. Se instala la última versión oficial disponible.' },
  { id: 'roundcube', name: 'Roundcube', description: 'Webmail completo y extensible con libreta de contactos, filtros y amplio ecosistema de plugins.' },
  { id: 'thunderbird', name: 'Thunderbird', description: 'Cliente de correo gráfico para escritorio. No expone un servicio web ni una dirección HTTP.' },
];

function MailIcon({ mail }: { mail: MailSystem }) {
  if (mail === 'rainloop') return <CloudRain size={44} strokeWidth={1.6} className="text-[#2563eb]" />;
  if (mail === 'roundcube' || mail === 'thunderbird') {
    const icon = mail === 'roundcube' ? siRoundcube : siThunderbird;
    return <svg width="44" height="44" viewBox="0 0 24 24" role="img" aria-label={icon.title} style={{ color: `#${icon.hex}` }}><path fill="currentColor" d={icon.path} /></svg>;
  }
  return <Mails size={44} strokeWidth={1.6} className="text-[#147a62]" />;
}

export function MailSelectionScreen({ selectedMail, onSelectMail, onContinue }: Props) {
  return (
    <div className="min-h-screen px-6 md:px-12 py-16 flex flex-col items-center justify-center max-w-6xl mx-auto w-full">
      <div className="text-center mb-12 w-full">
        <h1 className="text-[34px] leading-[1.47] font-semibold tracking-tight text-[#1a1c1d] mb-2">Selecciona tu sistema de correo</h1>
        <p className="text-[17px] leading-[1.47] text-[#5f5e60] max-w-2xl mx-auto">Elige el cliente de correo que quieres preparar en este servidor.</p>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 w-full max-w-4xl">
        {options.map((option) => {
          const selected = selectedMail === option.id;
          return (
            <article key={option.id} onClick={() => onSelectMail(option.id)} className={`utility-card bg-white border rounded-lg p-6 flex flex-col cursor-pointer ${selected ? 'border-[#0066cc] ring-2 ring-[#0066cc]/20' : 'border-[#e0e0e0] hover:border-[#c1c6d5]'}`}>
              <div className="h-12 flex items-center mb-5"><MailIcon mail={option.id} /></div>
              <h2 className="text-[17px] font-semibold mb-1">{option.name}</h2>
              <p className="text-[14px] leading-[1.43] text-[#5f5e60] mb-7 flex-1">{option.description}</p>
              <button type="button" onClick={(event) => { event.stopPropagation(); onSelectMail(option.id); }} className={`selection-button ${selected ? 'selection-button-active' : ''}`} aria-pressed={selected}>Seleccionar</button>
            </article>
          );
        })}
      </div>
      <div className="mt-8 flex flex-col items-center gap-3">
        <button type="button" onClick={onContinue} disabled={!selectedMail} className="primary-button min-w-52">Continuar <ArrowRight size={18} /></button>
        <p className="text-sm text-[#667085]">Nada se instalará hasta que pulses Continuar.</p>
      </div>
    </div>
  );
}
