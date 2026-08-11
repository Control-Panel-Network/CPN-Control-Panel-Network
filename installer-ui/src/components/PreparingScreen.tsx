import type { InstallerStatus } from '../types';

export function PreparingScreen({ status }: { status: InstallerStatus }) {
  return (
    <section className="min-h-screen grid place-items-center px-6">
      <div className="text-center max-w-xl">
        <p className="eyebrow">CONFIGURACIÓN DEL SERVIDOR</p>
        <h1 className="text-4xl sm:text-5xl font-semibold tracking-[-0.04em] mt-3">Estamos preparando todo...</h1>
        <p className="text-[#667085] text-lg mt-4">{status.message}</p>
        <div className="spinner mx-auto mt-10" role="status" aria-label="Preparando el instalador" />
        {status.error && <p className="error-box mt-8">{status.error}</p>}
      </div>
    </section>
  );
}
