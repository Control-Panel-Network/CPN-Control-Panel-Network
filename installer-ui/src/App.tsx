import { useCallback, useEffect, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { flushSync } from 'react-dom';
import { configureDns, connectInstallerEvents, getStatus, startCloudflareOAuth, startMailInstall, startServerInstall, validateDomain } from './api';
import { CompleteScreen } from './components/CompleteScreen';
import { CompareModal } from './components/CompareModal';
import { DnsSelectionScreen } from './components/DnsSelectionScreen';
import { DomainScreen } from './components/DomainScreen';
import { InstallingScreen } from './components/InstallingScreen';
import { MailSelectionScreen } from './components/MailSelectionScreen';
import { PreparingScreen } from './components/PreparingScreen';
import { ServerSelectionScreen } from './components/ServerSelectionScreen';
import type { InstallerEvent, InstallerStatus, MailSystem, ScreenType, ServerEngine } from './types';

const INITIAL_STATUS: InstallerStatus = {
  phase: 'preparing', stage: 'server', progress: 0, message: 'Estamos preparando todo...', domain: null,
  domain_is_cloudflare: false, dns_provider: null, cloudflare_connected: false,
  selected_server: null, installed_server: null, selected_mail: null, installed_mail: null,
  environment: null, panel_url: null, panel_admin_email: null, panel_admin_password: null, error: null,
  failed_phase: null,
};

const screenFor = (status: InstallerStatus): ScreenType => {
  if (status.phase === 'preparing') return 'preparing';
  if (['configuring', 'downloading', 'installing', 'testing', 'failed_rolled_back', 'failed_partial', 'cancelled'].includes(status.phase)) return 'installing';
  if (status.stage === 'domain') return 'domain';
  if (status.stage === 'dns') return 'dns';
  if (status.stage === 'server') return 'selection';
  if (status.stage === 'mail') return 'mail';
  return 'complete';
};

export default function App() {
  const [screen, setScreen] = useState<ScreenType>('preparing');
  const [selectedServer, setSelectedServer] = useState<ServerEngine | null>(null);
  const [selectedMail, setSelectedMail] = useState<MailSystem | null>(null);
  const [status, setStatus] = useState(INITIAL_STATUS);
  const [compareOpen, setCompareOpen] = useState(false);
  const reconnectTimer = useRef<number | undefined>(undefined);
  const completionTimer = useRef<number | undefined>(undefined);

  const acceptStatus = useCallback((next: InstallerStatus, delayCompleted = false) => {
    setStatus(next);
    setSelectedServer(next.selected_server);
    setSelectedMail(next.selected_mail);
    window.clearTimeout(completionTimer.current);
    if (delayCompleted && next.phase === 'completed') {
      completionTimer.current = window.setTimeout(() => setScreen(screenFor(next)), 2000);
    } else setScreen(screenFor(next));
  }, []);

  const handleEvent = useCallback((event: InstallerEvent) => {
    if ('status' in event) acceptStatus(event.status, event.type === 'completed');
  }, [acceptStatus]);

  useEffect(() => {
    let disposed = false;
    let socket: WebSocket | undefined;
    const connect = () => {
      socket = connectInstallerEvents(handleEvent);
      socket.addEventListener('close', () => { if (!disposed) reconnectTimer.current = window.setTimeout(connect, 1500); });
    };
    getStatus().then((next) => { if (!disposed) acceptStatus(next); }).catch((error) => setStatus((current) => ({ ...current, phase: 'failed_partial', error: error.message })));
    connect();
    return () => { disposed = true; socket?.close(); window.clearTimeout(reconnectTimer.current); window.clearTimeout(completionTimer.current); };
  }, [acceptStatus, handleEvent]);

  const beginServerInstall = async () => {
    if (!selectedServer) return;
    flushSync(() => setStatus((current) => ({ ...current, phase: 'configuring', progress: 0, error: null, failed_phase: null, selected_server: selectedServer })));
    setScreen('installing');
    try { await startServerInstall(selectedServer); }
    catch (error) { setStatus((current) => ({ ...current, failed_phase: current.phase, phase: 'failed_partial', error: error instanceof Error ? error.message : 'Error desconocido' })); }
  };

  const beginMailInstall = async () => {
    if (!selectedMail) return;
    flushSync(() => setStatus((current) => ({ ...current, phase: 'configuring', progress: 0, error: null, failed_phase: null, selected_mail: selectedMail })));
    setScreen('installing');
    try { await startMailInstall(selectedMail); }
    catch (error) { setStatus((current) => ({ ...current, failed_phase: current.phase, phase: 'failed_partial', error: error instanceof Error ? error.message : 'Error desconocido' })); }
  };

  return (
    <main className="min-h-screen bg-white text-[#1d1d1f]">
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={screen} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.9, ease: 'easeInOut' }} className="min-h-screen">
          {screen === 'preparing' && <PreparingScreen status={status} />}
          {screen === 'domain' && <DomainScreen onValidate={validateDomain} />}
          {screen === 'dns' && <DnsSelectionScreen cloudflareAvailable={status.domain_is_cloudflare} oauthError={status.error} onLocal={async () => acceptStatus(await configureDns('local'))} onCloudflare={async () => { const response = await startCloudflareOAuth(); window.location.assign(response.authorization_url); }} />}
          {screen === 'selection' && <ServerSelectionScreen selectedServer={selectedServer} onSelectServer={setSelectedServer} onContinue={beginServerInstall} onOpenCompare={() => setCompareOpen(true)} />}
          {screen === 'installing' && <InstallingScreen status={status} />}
          {screen === 'mail' && <MailSelectionScreen selectedMail={selectedMail} onSelectMail={setSelectedMail} onContinue={beginMailInstall} />}
          {screen === 'complete' && <CompleteScreen status={status} />}
        </motion.div>
      </AnimatePresence>
      <CompareModal isOpen={compareOpen} selectedServer={selectedServer} onClose={() => setCompareOpen(false)} onSelectServer={(server) => { setSelectedServer(server); setCompareOpen(false); }} />
    </main>
  );
}
