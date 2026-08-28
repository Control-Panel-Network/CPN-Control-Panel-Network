import { ArrowRight, CircleHelp } from 'lucide-react';
import type { ServerEngine } from '../types';
import { ServerBrandIcon } from './ServerBrandIcon';

interface Props {
  selectedServer: ServerEngine | null;
  onSelectServer: (server: ServerEngine) => void;
  onContinue: () => void;
  onOpenCompare: () => void;
}

const servers: Array<{ id: ServerEngine; name: string; description: string }> = [
  {
    id: 'openlitespeed',
    name: 'OpenLiteSpeed',
    description: 'Alto rendimiento y bajo consumo de recursos. Ideal para WordPress y sitios con alto tráfico gracias a su caché integrado (LSCache).',
  },
  {
    id: 'nginx',
    name: 'Nginx',
    description: 'El estándar de la industria. Robusto, extremadamente estable y perfecto para servir contenido estático y actuar como proxy inverso.',
  },
  {
    id: 'caddy',
    name: 'Caddy',
    description: 'Servidor web moderno con HTTPS automático por defecto. Configuración minimalista y excelente seguridad lista para usar.',
  },
];

export function ServerSelectionScreen({ selectedServer, onSelectServer, onContinue, onOpenCompare }: Props) {
  return (
    <div className="min-h-screen px-6 md:px-12 py-16 flex flex-col items-center justify-center max-w-6xl mx-auto w-full">
      <div className="text-center mb-12 w-full">
        <h1 className="text-[34px] leading-[1.47] font-semibold tracking-tight text-[#1a1c1d] mb-2">Selecciona tu servidor web</h1>
        <p className="text-[17px] leading-[1.47] text-[#5f5e60] max-w-2xl mx-auto">
          Elige el motor web que mejor se adapte a las necesidades de tu proyecto. Podrás cambiarlo más adelante desde el panel.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6 w-full max-w-5xl">
        {servers.map((server) => {
          const selected = selectedServer === server.id;
          return (
            <button
              key={server.id}
              type="button"
              onClick={() => onSelectServer(server.id)}
              className={`utility-card bg-white border rounded-lg p-6 flex flex-col cursor-pointer text-left transition-all ${selected ? 'border-[#0066cc] ring-2 ring-[#0066cc]/20' : 'border-[#e0e0e0] hover:border-[#c1c6d5]'}`}
              aria-pressed={selected}
            >
              <div className="mb-6 h-12 flex items-center"><ServerBrandIcon server={server.id} /></div>
              <h2 className="text-[17px] font-semibold text-[#1a1c1d] mb-1">{server.name}</h2>
              <p className="text-[14px] leading-[1.43] text-[#5f5e60] mb-8 flex-1">{server.description}</p>
              <span className={`selection-button ${selected ? 'selection-button-active' : ''}`}>Seleccionar</span>
            </button>
          );
        })}
      </div>

      <button type="button" onClick={onOpenCompare} className="compare-link mt-8">
        <CircleHelp size={17} /> ¿No estás seguro de cuál elegir? Compara características
      </button>

      <div className="mt-8 flex flex-col items-center gap-3">
        <button type="button" onClick={onContinue} disabled={!selectedServer} className="primary-button min-w-52">
          Continuar <ArrowRight size={18} />
        </button>
        <p className="text-sm text-[#667085]">Nada se instalará hasta que pulses Continuar.</p>
      </div>
    </div>
  );
}
