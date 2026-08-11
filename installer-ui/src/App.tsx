import { useCallback, useEffect, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { PreparingScreen } from './components/PreparingScreen';
import { ServerSelectionScreen } from './components/ServerSelectionScreen';
import { InstallingScreen } from './components/InstallingScreen';
import { MailSelectionScreen } from './components/MailSelectionScreen';
import { CompleteScreen } from './components/CompleteScreen';
import { CompareModal } from './components/CompareModal';
import { connectInstallerEvents, getStatus, startMailInstall, startServerInstall } from './api';
import type { InstallerEvent, InstallerStatus, MailSystem, ScreenType, ServerEngine } from './types';

const INITIAL_STATUS: InstallerStatus = {
  phase: 'preparing', progress: 0, message: 'Estamos preparando todo...',
  selected_server: null, selected_mail: null, environment: null, error: null,
};

export default function App() {
  const [screen, setScreen] = useState<ScreenType>('preparing');
  const [selectedServer, setSelectedServer] = useState<ServerEngine | null>(null);
  const [selectedMail, setSelectedMail] = useState<MailSystem | null>(null);
  const [status, setStatus] = useState(INITIAL_STATUS);
  const [compareOpen, setCompareOpen] = useState(false);
  const reconnectTimer = useRef<number | undefined>(undefined);
  const completionTimer = useRef<number | undefined>(undefined);

  const handleEvent = useCallback((event: InstallerEvent) => {
    if ('status' in event) setStatus(event.status);
    if (event.type === 'progress' && ['downloading', 'installing', 'testing'].includes(event.status.phase)) {
      setScreen('installing');
    }
    if (event.type === 'completed') {
      window.clearTimeout(completionTimer.current);
      completionTimer.current = window.setTimeout(() => {
        setScreen(event.status.selected_mail ? 'complete' : 'mail');
      }, 2000);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let socket: WebSocket | undefined;
    const connect = () => {
      socket = connectInstallerEvents(handleEvent);
      socket.addEventListener('close', () => {
        if (!disposed) reconnectTimer.current = window.setTimeout(connect, 1500);
      });
    };
    getStatus().then((next) => {
      if (disposed) return;
      setStatus(next);
      setSelectedServer(next.selected_server);
      setSelectedMail(next.selected_mail);
      if (next.phase === 'ready') setScreen('selection');
      if (['downloading', 'installing', 'testing', 'failed'].includes(next.phase)) setScreen('installing');
      if (next.phase === 'completed') setScreen(next.selected_mail ? 'complete' : 'mail');
    }).catch((error) => setStatus((current) => ({ ...current, phase: 'failed', error: error.message })));
    connect();
    return () => {
      disposed = true;
      socket?.close();
      window.clearTimeout(reconnectTimer.current);
      window.clearTimeout(completionTimer.current);
    };
  }, [handleEvent]);

  const beginServerInstall = async () => {
    if (!selectedServer) return;
    setScreen('installing');
    setStatus((current) => ({ ...current, phase: 'downloading', progress: 0, error: null, selected_server: selectedServer, selected_mail: null }));
    try { await startServerInstall(selectedServer); }
    catch (error) { setStatus((current) => ({ ...current, phase: 'failed', error: error instanceof Error ? error.message : 'Error desconocido' })); }
  };

  const beginMailInstall = async () => {
    if (!selectedMail) return;
    setScreen('installing');
    setStatus((current) => ({ ...current, phase: 'downloading', progress: 0, error: null, selected_mail: selectedMail }));
    try { await startMailInstall(selectedMail); }
    catch (error) { setStatus((current) => ({ ...current, phase: 'failed', error: error instanceof Error ? error.message : 'Error desconocido' })); }
  };

  return (
    <main className="min-h-screen bg-[#f7f8fa] text-[#111827]">
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={screen} initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.45, ease: 'easeInOut' }} className="min-h-screen">
          {screen === 'preparing' && <PreparingScreen status={status} />}
          {screen === 'selection' && <ServerSelectionScreen selectedServer={selectedServer} onSelectServer={setSelectedServer} onContinue={beginServerInstall} onOpenCompare={() => setCompareOpen(true)} />}
          {screen === 'installing' && <InstallingScreen status={status} />}
          {screen === 'mail' && <MailSelectionScreen selectedMail={selectedMail} onSelectMail={setSelectedMail} onContinue={beginMailInstall} />}
          {screen === 'complete' && <CompleteScreen server={status.selected_server} mail={status.selected_mail} />}
        </motion.div>
      </AnimatePresence>
      <CompareModal isOpen={compareOpen} selectedServer={selectedServer} onClose={() => setCompareOpen(false)} onSelectServer={(server) => { setSelectedServer(server); setCompareOpen(false); }} />
    </main>
  );
}
