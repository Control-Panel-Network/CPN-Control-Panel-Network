import { Check, ExternalLink } from 'lucide-react';
import type { MailSystem, ServerEngine } from '../types';

export function CompleteScreen({ server, mail }: { server: ServerEngine | null; mail: MailSystem | null }) {
  const token = new URLSearchParams(window.location.search).get('token') ?? '';
  return (
    <section className="min-h-screen px-6 grid place-items-center">
      <div className="text-center max-w-xl">
        <div className="success-icon mx-auto"><Check size={30} /></div>
        <p className="eyebrow mt-7">INSTALACIÓN COMPLETADA</p>
        <h1 className="text-4xl sm:text-5xl font-semibold tracking-[-0.04em] mt-2">Todo está listo</h1>
        <p className="text-[#667085] text-lg mt-4">{server && mail ? `${server} y ${mail} están instalados y han superado sus comprobaciones.` : 'El servidor está listo.'}</p>
        <a className="primary-button mt-8 mx-auto" href={`/api/status?token=${encodeURIComponent(token)}`} target="_blank" rel="noreferrer">Ver estado técnico <ExternalLink size={17} /></a>
      </div>
    </section>
  );
}
