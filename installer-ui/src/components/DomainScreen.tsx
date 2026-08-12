import { ArrowRight, CheckCircle2, Globe2, LoaderCircle } from 'lucide-react';
import { FormEvent, useState } from 'react';
import type { DomainValidation } from '../types';

interface Props { onValidate: (domain: string) => Promise<DomainValidation> }

export function DomainScreen({ onValidate }: Props) {
  const [domain, setDomain] = useState('');
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<DomainValidation | null>(null);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setChecking(true);
    try { setResult(await onValidate(domain)); }
    catch (error) { setResult({ valid: false, resolvable: false, cloudflare: false, normalized: null, nameservers: [], error: error instanceof Error ? error.message : 'No se pudo validar el dominio' }); }
    finally { setChecking(false); }
  };

  return (
    <section className="min-h-screen flex items-center justify-center px-6 bg-white">
      <form onSubmit={submit} className="w-full max-w-xl">
        <Globe2 size={36} strokeWidth={1.5} className="text-[#0071e3] mb-8" />
        <h1 className="text-[40px] leading-tight font-semibold tracking-[-0.035em] text-[#1d1d1f]">¿Qué dominio quieres usar?</h1>
        <p className="mt-3 text-[17px] leading-relaxed text-[#6e6e73]">Comprobaremos su formato, resolución DNS y proveedor autoritativo.</p>
        <label htmlFor="domain" className="sr-only">Dominio</label>
        <input id="domain" value={domain} onChange={(event) => { setDomain(event.target.value); setResult(null); }} placeholder="example.com" autoComplete="url" spellCheck={false} className="mt-10 w-full border-0 border-b border-[#d2d2d7] px-0 py-4 text-2xl outline-none focus:border-[#0071e3]" />
        {result && !result.valid && <p className="mt-4 text-sm text-[#b42318]" role="alert">{result.error}</p>}
        {result?.valid && <p className="mt-4 flex items-center gap-2 text-sm text-[#18794e]"><CheckCircle2 size={17} /> {result.normalized} está correctamente delegado{result.cloudflare ? ' en Cloudflare' : ''}.</p>}
        <button type="submit" disabled={!domain.trim() || checking || result?.valid} className="primary-button mt-9">
          {checking ? <><LoaderCircle size={18} className="animate-spin" /> Comprobando</> : <>Comprobar dominio <ArrowRight size={18} /></>}
        </button>
      </form>
    </section>
  );
}
