import { useEffect, useState } from 'react';
import { ArrowRight, CircleHelp } from 'lucide-react';
import type { DatabaseEngine, ServerEngine } from '../types';
import { ServerBrandIcon } from './ServerBrandIcon';
import { useI18n } from '../i18n';
import { LanguageSelector } from '../i18n/LanguageSelector';

export type OldPortPolicy = 'redirect_1m' | 'redirect_3m' | 'deny';

interface Props {
  selectedServer: ServerEngine | null;
  listenPort: number;
  panelHostname?: string | null;
  database: DatabaseEngine;
  installPhpmyadmin: boolean;
  onSelectServer: (server: ServerEngine) => void;
  onDatabaseChange: (database: DatabaseEngine) => void;
  onPhpmyadminChange: (enabled: boolean) => void;
  onNetworkChange: (input: {
    port: number;
    oldPortPolicy?: OldPortPolicy;
    panelHostname?: string;
  }) => Promise<string | null>;
  onContinue: () => void;
  onOpenCompare: () => void;
}

export function ServerSelectionScreen({
  selectedServer,
  listenPort,
  panelHostname,
  database,
  installPhpmyadmin,
  onSelectServer,
  onDatabaseChange,
  onPhpmyadminChange,
  onNetworkChange,
  onContinue,
  onOpenCompare,
}: Props) {
  const { t } = useI18n();
  const [portDraft, setPortDraft] = useState(String(listenPort || 2087));
  const [hostnameDraft, setHostnameDraft] = useState(panelHostname || '');
  const [oldPortPolicy, setOldPortPolicy] = useState<OldPortPolicy>('redirect_1m');
  const [portBusy, setPortBusy] = useState(false);
  const [portMessage, setPortMessage] = useState<string | null>(null);
  const [portError, setPortError] = useState<string | null>(null);

  useEffect(() => {
    setPortDraft(String(listenPort || 2087));
  }, [listenPort]);

  useEffect(() => {
    setHostnameDraft(panelHostname || '');
  }, [panelHostname]);

  const servers: Array<{ id: ServerEngine; name: string; description: string }> = [
    { id: 'openlitespeed', name: 'OpenLiteSpeed', description: t.serverOpenlitespeedDesc },
    { id: 'nginx', name: 'Nginx', description: t.serverNginxDesc },
    { id: 'caddy', name: 'Caddy', description: t.serverCaddyDesc },
  ];

  const parsedPort = Number.parseInt(portDraft.trim(), 10);
  const portChanging =
    Number.isFinite(parsedPort) && parsedPort >= 1 && parsedPort <= 65535 && parsedPort !== listenPort;

  const applyNetwork = async () => {
    const parsed = Number.parseInt(portDraft.trim(), 10);
    if (!Number.isFinite(parsed) || parsed < 1 || parsed > 65535) {
      setPortError(t.listenPortInvalid);
      setPortMessage(null);
      return;
    }
    setPortBusy(true);
    setPortError(null);
    try {
      const message = await onNetworkChange({
        port: parsed,
        oldPortPolicy: parsed !== listenPort ? oldPortPolicy : undefined,
        panelHostname: hostnameDraft.trim(),
      });
      setPortMessage(message ?? t.listenPortSaved);
    } catch (error) {
      setPortMessage(null);
      setPortError(error instanceof Error ? error.message : t.listenPortInvalid);
    } finally {
      setPortBusy(false);
    }
  };

  return (
    <div className="min-h-screen px-6 md:px-12 py-16 flex flex-col items-center justify-center max-w-6xl mx-auto w-full">
      <div className="w-full flex justify-end mb-4"><LanguageSelector /></div>
      <div className="text-center mb-12 w-full">
        <h1 className="text-[34px] leading-[1.47] font-semibold tracking-tight text-[#1a1c1d] mb-2">
          {t.selectServerTitle}
        </h1>
        <p className="text-[17px] leading-[1.47] text-[#5f5e60] max-w-2xl mx-auto">
          {t.selectServerIntro}
        </p>
      </div>

      <div className="w-full max-w-xl mb-10 rounded-lg border border-[#e0e0e0] bg-white p-5 text-left">
        <label className="block text-[15px] font-semibold text-[#1a1c1d]" htmlFor="cpn-listen-port">
          {t.listenPortLabel}
        </label>
        <p className="text-[13px] leading-[1.45] text-[#5f5e60] mt-1 mb-3">{t.listenPortHint}</p>
        <div className="flex flex-col sm:flex-row gap-3 items-stretch sm:items-center">
          <input
            id="cpn-listen-port"
            type="number"
            min={1}
            max={65535}
            inputMode="numeric"
            value={portDraft}
            onChange={(event) => setPortDraft(event.target.value)}
            className="border border-[#c1c6d5] rounded-md px-3 py-2 w-full sm:w-40 text-[15px]"
          />
        </div>

        {portChanging && (
          <fieldset className="mt-4">
            <legend className="text-[14px] font-semibold text-[#1a1c1d]">{t.oldPortPolicyLabel}</legend>
            <p className="text-[13px] text-[#5f5e60] mt-1 mb-2">{t.oldPortPolicyHint}</p>
            <label className="flex items-start gap-2 text-[14px] text-[#1a1c1d] mb-2">
              <input
                type="radio"
                name="old-port-policy"
                checked={oldPortPolicy === 'redirect_1m'}
                onChange={() => setOldPortPolicy('redirect_1m')}
              />
              <span>{t.oldPortPolicyRedirect1m}</span>
            </label>
            <label className="flex items-start gap-2 text-[14px] text-[#1a1c1d] mb-2">
              <input
                type="radio"
                name="old-port-policy"
                checked={oldPortPolicy === 'redirect_3m'}
                onChange={() => setOldPortPolicy('redirect_3m')}
              />
              <span>{t.oldPortPolicyRedirect3m}</span>
            </label>
            <label className="flex items-start gap-2 text-[14px] text-[#1a1c1d]">
              <input
                type="radio"
                name="old-port-policy"
                checked={oldPortPolicy === 'deny'}
                onChange={() => setOldPortPolicy('deny')}
              />
              <span>{t.oldPortPolicyDeny}</span>
            </label>
          </fieldset>
        )}

        <label
          className="block text-[15px] font-semibold text-[#1a1c1d] mt-5"
          htmlFor="cpn-panel-hostname"
        >
          {t.panelHostnameLabel}
        </label>
        <p className="text-[13px] leading-[1.45] text-[#5f5e60] mt-1 mb-3">{t.panelHostnameHint}</p>
        <input
          id="cpn-panel-hostname"
          type="text"
          inputMode="url"
          autoComplete="off"
          placeholder={t.panelHostnamePlaceholder}
          value={hostnameDraft}
          onChange={(event) => setHostnameDraft(event.target.value)}
          className="border border-[#c1c6d5] rounded-md px-3 py-2 w-full text-[15px]"
        />

        <button
          type="button"
          onClick={() => void applyNetwork()}
          disabled={portBusy}
          className="selection-button mt-4"
        >
          {t.networkSave}
        </button>
        {portMessage && <p className="text-sm text-[#067647] mt-3">{portMessage}</p>}
        {portError && <p className="text-sm text-[#b42318] mt-3">{portError}</p>}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6 w-full max-w-5xl">
        {servers.map((server) => {
          const selected = selectedServer === server.id;
          return (
            <article
              key={server.id}
              onClick={() => onSelectServer(server.id)}
              className={`utility-card bg-white border rounded-lg p-6 flex flex-col cursor-pointer transition-all ${selected ? 'border-[#0066cc] ring-2 ring-[#0066cc]/20' : 'border-[#e0e0e0] hover:border-[#c1c6d5]'}`}
            >
              <div className="mb-6 h-12 flex items-center">
                <ServerBrandIcon server={server.id} />
              </div>
              <h2 className="text-[17px] font-semibold text-[#1a1c1d] mb-1">{server.name}</h2>
              <p className="text-[14px] leading-[1.43] text-[#5f5e60] mb-8 flex-1">{server.description}</p>
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  onSelectServer(server.id);
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

      <div className="w-full max-w-xl mt-10 rounded-lg border border-[#e0e0e0] bg-white p-5 text-left">
        <h2 className="text-[17px] font-semibold text-[#1a1c1d] mb-1">{t.databaseTitle}</h2>
        <p className="text-[13px] leading-[1.45] text-[#5f5e60] mb-4">{t.databaseHint}</p>
        <fieldset>
          <legend className="sr-only">{t.databaseTitle}</legend>
          <label className="flex items-start gap-2 text-[14px] text-[#1a1c1d] mb-2">
            <input
              type="radio"
              name="cpn-database"
              checked={database === 'mariadb'}
              onChange={() => onDatabaseChange('mariadb')}
            />
            <span>{t.databaseMariadb}</span>
          </label>
          <label className="flex items-start gap-2 text-[14px] text-[#1a1c1d] mb-2">
            <input
              type="radio"
              name="cpn-database"
              checked={database === 'mysql'}
              onChange={() => onDatabaseChange('mysql')}
            />
            <span>{t.databaseMysql}</span>
          </label>
          <label className="flex items-start gap-2 text-[14px] text-[#1a1c1d] mb-3">
            <input
              type="radio"
              name="cpn-database"
              checked={database === 'none'}
              onChange={() => onDatabaseChange('none')}
            />
            <span>{t.databaseNone}</span>
          </label>
        </fieldset>
        <label className="flex items-start gap-2 text-[14px] text-[#1a1c1d]">
          <input
            type="checkbox"
            checked={installPhpmyadmin}
            onChange={(event) => onPhpmyadminChange(event.target.checked)}
          />
          <span>{t.databasePhpmyadmin}</span>
        </label>
      </div>

      <button type="button" onClick={onOpenCompare} className="compare-link mt-8">
        <CircleHelp size={17} /> {t.compareLink}
      </button>

      <div className="mt-8 flex flex-col items-center gap-3">
        <button
          type="button"
          onClick={onContinue}
          disabled={!selectedServer}
          className="primary-button min-w-52"
        >
          {t.continueLabel} <ArrowRight size={18} />
        </button>
        <p className="text-sm text-[#667085]">{t.nothingInstallsYet}</p>
      </div>
    </div>
  );
}
