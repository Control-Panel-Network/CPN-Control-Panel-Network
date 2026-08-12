import { ArrowRight, Cloud, Server } from 'lucide-react';
import { useState } from 'react';
import type { DnsProvider } from '../types';

interface Props {
  cloudflareAvailable: boolean;
  onLocal: () => Promise<void>;
  onCloudflare: () => Promise<void>;
}

export function DnsSelectionScreen({ cloudflareAvailable, onLocal, onCloudflare }: Props) {
  const [selected, setSelected] = useState<DnsProvider | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const continueSetup = async () => {
    if (!selected) return;
    setBusy(true); setError(null);
    try { await (selected === 'local' ? onLocal() : onCloudflare()); }
    catch (reason) { setError(reason instanceof Error ? reason.message : 'No se pudo continuar'); setBusy(false); }
  };
  const options = [
    { id: 'local' as const, name: 'DNS local', detail: 'Administrar los registros desde este servidor.', icon: Server, enabled: true },
    { id: 'cloudflare' as const, name: 'Cloudflare DNS', detail: 'Autorizar la zona con OAuth y verificar los permisos.', icon: Cloud, enabled: cloudflareAvailable },
  ];
  return (
    <section className="min-h-screen flex items-center justify-center px-6 bg-white">
      <div className="w-full max-w-xl">
        <h1 className="text-[40px] leading-tight font-semibold tracking-[-0.035em] text-[#1d1d1f]">Elige cómo gestionar el DNS</h1>
        <p className="mt-3 text-[17px] text-[#6e6e73]">Cloudflare solo aparece disponible cuando sus nameservers son autoritativos.</p>
        <div className="mt-10 border-y border-[#e5e5e7] divide-y divide-[#e5e5e7]">
          {options.map(({ id, name, detail, icon: Icon, enabled }) => (
            <button key={id} type="button" disabled={!enabled} onClick={() => setSelected(id)} className={`w-full py-6 flex items-center gap-4 text-left transition-opacity ${enabled ? '' : 'opacity-40 cursor-not-allowed'}`}>
              <Icon size={25} strokeWidth={1.6} className={selected === id ? 'text-[#0071e3]' : 'text-[#6e6e73]'} />
              <span className="flex-1"><strong className="block text-[17px] font-semibold">{name}</strong><span className="text-sm text-[#6e6e73]">{enabled ? detail : 'No detectado para este dominio'}</span></span>
              <span className={`h-5 w-5 rounded-full border grid place-items-center ${selected === id ? 'border-[#0071e3]' : 'border-[#a1a1a6]'}`}>{selected === id && <span className="h-2.5 w-2.5 rounded-full bg-[#0071e3]" />}</span>
            </button>
          ))}
        </div>
        {error && <p className="mt-4 text-sm text-[#b42318]">{error}</p>}
        <button type="button" onClick={continueSetup} disabled={!selected || busy} className="primary-button mt-9">{busy ? 'Conectando…' : 'Continuar'} <ArrowRight size={18} /></button>
      </div>
    </section>
  );
}
