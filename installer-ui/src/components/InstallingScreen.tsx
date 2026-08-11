import { CheckCircle2, Circle, LoaderCircle, XCircle } from 'lucide-react';
import type { InstallerStatus } from '../types';

function StepIcon({ state }: { state: 'pending' | 'active' | 'done' | 'error' }) {
  if (state === 'active') return <LoaderCircle size={24} className="text-[#0071e3] animate-spin" />;
  if (state === 'done') return <CheckCircle2 size={24} className="text-[#0071e3]" />;
  if (state === 'error') return <XCircle size={24} className="text-[#c2413b]" />;
  return <Circle size={24} className="text-[#a1a1a6]" />;
}

export function InstallingScreen({ status }: { status: InstallerStatus }) {
  const phase = status.phase;
  const failed = phase === 'failed';
  const downloading = phase === 'downloading';
  const installing = phase === 'installing';
  const testing = phase === 'testing';
  const downloadDone = installing || testing || phase === 'completed' || (failed && status.progress > 15);
  const installDone = testing || phase === 'completed';

  return (
    <section className="w-full min-h-screen flex flex-col items-center justify-center p-6 bg-white">
      <div className="w-full max-w-md">
        <h1 className="text-[34px] leading-[1.47] font-semibold text-[#1a1c1d] mb-12 text-left tracking-tighter">Instalando</h1>

        <div className="flex flex-col gap-8">
          <div className="flex items-center gap-[17px]">
            <StepIcon state={downloading ? 'active' : downloadDone ? 'done' : failed ? 'error' : 'pending'} />
            <span className="text-[17px] font-semibold text-[#1a1c1d]">{downloading ? `Descargando ${status.progress}%` : 'Descargando'}</span>
          </div>

          <div className="flex items-center gap-[17px]">
            <StepIcon state={installing ? 'active' : installDone ? 'done' : failed && downloadDone ? 'error' : 'pending'} />
            <span className={`text-[17px] ${installing || installDone ? 'font-semibold text-[#1a1c1d]' : 'text-[#7a7a7a]'}`}>
              {installing ? `Instalando ${status.progress}%` : 'Instalando'}
            </span>
          </div>

          <div className="flex items-center gap-[17px]">
            <StepIcon state={testing ? 'active' : phase === 'completed' ? 'done' : failed && installDone ? 'error' : 'pending'} />
            <span className={`text-[17px] ${testing || phase === 'completed' ? 'font-semibold text-[#1a1c1d]' : 'text-[#7a7a7a]'}`}>Probando tests</span>
          </div>
        </div>

        <p className="mt-10 text-[14px] text-[#7a7a7a] min-h-5" aria-live="polite">{status.message}</p>
        {failed && <p className="mt-3 text-[14px] text-[#c2413b]">{status.error}</p>}
      </div>
    </section>
  );
}
