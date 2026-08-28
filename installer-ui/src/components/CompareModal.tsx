import { Check, X } from 'lucide-react';
import type { ServerEngine } from '../types';
import { ServerBrandIcon } from './ServerBrandIcon';

interface Props {
  isOpen: boolean;
  selectedServer: ServerEngine | null;
  onClose: () => void;
  onSelectServer: (server: ServerEngine) => void;
}

const options: Array<{ id: ServerEngine; name: string; description: string; features: string[] }> = [
  { id: 'openlitespeed', name: 'OpenLiteSpeed', description: 'Rendimiento, compatibilidad con reescrituras Apache y caché LSCache.', features: ['Caché LSCache integrado', 'Ideal para WordPress', 'Bajo consumo de recursos'] },
  { id: 'nginx', name: 'Nginx', description: 'Servidor estable y ampliamente utilizado para contenido estático y proxy inverso.', features: ['Máxima estabilidad', 'Excelente proxy inverso', 'Amplia documentación'] },
  { id: 'caddy', name: 'Caddy', description: 'Servidor moderno con configuración sencilla y HTTPS automático.', features: ['HTTPS automático', 'Caddyfile sencillo', 'Seguridad por defecto'] },
];

export function CompareModal({ isOpen, selectedServer, onClose, onSelectServer }: Props) {
  if (!isOpen) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-xs" onClick={onClose}>
      <div className="bg-white rounded-2xl max-w-3xl w-full p-6 md:p-8 shadow-2xl border border-[#e0e0e0] max-h-[90vh] overflow-y-auto" onClick={(event) => event.stopPropagation()}>
        <header className="flex items-start justify-between gap-5 pb-5 border-b border-[#e8e8ea]">
          <div><h2 className="text-[21px] font-semibold">Comparativa de servidores web</h2><p className="text-sm text-[#5f5e60]">Encuentra la opción adecuada para tu proyecto.</p></div>
          <button type="button" onClick={onClose} className="icon-button" aria-label="Cerrar"><X size={20} /></button>
        </header>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 py-6">
          {options.map((option) => {
            const selected = selectedServer === option.id;
            return (
              <article key={option.id} className={`compare-card ${selected ? 'compare-card-active' : ''}`}>
                <div className="h-11 flex items-center"><ServerBrandIcon server={option.id} size={38} /></div>
                <h3 className="font-semibold mt-3">{option.name}</h3>
                <p className="text-xs leading-5 text-[#5f5e60] mt-2">{option.description}</p>
                <ul className="text-[13px] space-y-2 my-5 flex-1">
                  {option.features.map((feature) => <li key={feature} className="flex gap-2"><Check size={16} className="text-[#0071e3] shrink-0" />{feature}</li>)}
                </ul>
                <button type="button" onClick={() => { onSelectServer(option.id); onClose(); }} className={`selection-button ${selected ? 'selection-button-active' : ''}`} aria-pressed={selected}>Seleccionar</button>
              </article>
            );
          })}
        </div>
      </div>
    </div>
  );
}
