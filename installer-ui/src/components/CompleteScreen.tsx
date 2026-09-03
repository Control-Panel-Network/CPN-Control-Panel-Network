import { useEffect, useMemo, useRef } from 'react';
import { Check, ExternalLink } from 'lucide-react';
import { formatMessage, useI18n } from '../i18n';
import type { MailSystem, ServerEngine } from '../types';

const SERVER_LABELS: Record<ServerEngine, string> = {
  openlitespeed: 'OpenLiteSpeed',
  nginx: 'Nginx',
  caddy: 'Caddy',
};

const MAIL_LABELS: Record<MailSystem, string> = {
  snappymail: 'SnappyMail',
  roundcube: 'Roundcube',
  thunderbird: 'Thunderbird',
};

function serverLabel(server: ServerEngine | null): string | null {
  return server ? SERVER_LABELS[server] : null;
}

function mailLabel(mail: MailSystem | null): string | null {
  return mail ? MAIL_LABELS[mail] : null;
}

export function CompleteScreen({
  server,
  mail,
  message,
  panelLoginUrl,
  autoOpen = true,
}: {
  server: ServerEngine | null;
  mail: MailSystem | null;
  message?: string | null;
  panelLoginUrl: string;
  autoOpen?: boolean;
}) {
  const { t } = useI18n();
  const token = new URLSearchParams(window.location.search).get('token') ?? '';
  const opened = useRef(false);
  const serverName = serverLabel(server);
  const mailName = mailLabel(mail);

  const summary = useMemo(() => {
    if (serverName && mailName) {
      return formatMessage(t.completeSummaryBoth, { server: serverName, mail: mailName });
    }
    if (serverName) {
      return formatMessage(t.completeSummaryServer, { server: serverName });
    }
    return t.completeSummaryReady;
  }, [serverName, mailName, t]);

  useEffect(() => {
    if (!autoOpen || opened.current || !panelLoginUrl) return;
    opened.current = true;
    const timer = window.setTimeout(() => {
      window.open(panelLoginUrl, '_blank', 'noopener,noreferrer');
    }, 900);
    return () => window.clearTimeout(timer);
  }, [autoOpen, panelLoginUrl]);

  return (
    <section className="min-h-screen px-6 grid place-items-center">
      <div className="text-center max-w-xl">
        <div className="success-icon mx-auto"><Check size={30} /></div>
        <p className="eyebrow mt-7">{t.completeEyebrow}</p>
        <h1 className="text-4xl sm:text-5xl font-semibold tracking-[-0.04em] mt-2">{t.completeTitle}</h1>
        <p className="text-[#667085] text-lg mt-4">{summary}</p>
        {message ? <p className="text-[#475467] text-base mt-3">{message}</p> : null}
        <p className="text-[#667085] text-sm mt-3">{t.openingPanelHint}</p>
        <div className="mt-8 flex flex-col sm:flex-row gap-3 justify-center items-center">
          <a
            className="primary-button"
            href={panelLoginUrl}
            target="_blank"
            rel="noreferrer"
          >
            {t.openPanelLogin} <ExternalLink size={17} />
          </a>
          <a
            className="secondary-button"
            href={`/status?token=${encodeURIComponent(token)}`}
            target="_blank"
            rel="noreferrer"
          >
            {t.technicalStatus} <ExternalLink size={17} />
          </a>
          <a
            className="secondary-button"
            href={`/?token=${encodeURIComponent(token)}`}
          >
            {t.backToInstaller}
          </a>
        </div>
      </div>
    </section>
  );
}
