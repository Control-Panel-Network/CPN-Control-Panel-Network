import { Check, ExternalLink } from 'lucide-react';
import type { InstallerStatus } from '../types';

export function CompleteScreen({ status }: { status: InstallerStatus }) {
  return (
    <section className="min-h-screen px-6 grid place-items-center">
      <div className="text-center max-w-xl">
        <div className="success-icon mx-auto"><Check size={30} /></div>
        <p className="eyebrow mt-7">INSTALACIÓN COMPLETADA</p>
        <h1 className="text-4xl sm:text-5xl font-semibold tracking-[-0.04em] mt-2">Todo está listo</h1>
        <p className="text-[#667085] text-lg mt-4">{status.installed_server && status.installed_mail ? `${status.installed_server} y ${status.installed_mail} están instalados. El Panel seguirá funcionando cuando cierres este instalador.` : 'El servidor está listo.'}</p>
        {status.panel_admin_email && status.panel_admin_password && (
          <dl className="mt-8 text-left border-y border-[#e5e7eb] py-5 space-y-3">
            <div className="flex justify-between gap-5"><dt className="text-[#667085]">Usuario</dt><dd className="font-medium select-all">{status.panel_admin_email}</dd></div>
            <div className="flex justify-between gap-5"><dt className="text-[#667085]">Contraseña inicial</dt><dd className="font-mono text-sm select-all">{status.panel_admin_password}</dd></div>
          </dl>
        )}
        {status.panel_url && <a className="primary-button mt-8 mx-auto" href={status.panel_url} target="_blank" rel="noreferrer">Abrir el Panel <ExternalLink size={17} /></a>}
      </div>
    </section>
  );
}
