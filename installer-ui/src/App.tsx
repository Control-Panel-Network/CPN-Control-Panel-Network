import { useCallback, useEffect, useRef, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { PreparingScreen } from './components/PreparingScreen';
import { ServerSelectionScreen } from './components/ServerSelectionScreen';
import { InstallingScreen } from './components/InstallingScreen';
import { MailSelectionScreen } from './components/MailSelectionScreen';
import { CompleteScreen } from './components/CompleteScreen';
import { CompareModal } from './components/CompareModal';
import { AccountSetupScreen } from './components/AccountSetupScreen';
import {
  connectInstallerEvents,
  getStatus,
  resolvePanelLoginUrl,
  setLanguage,
  startMailInstall,
  startServerInstall,
} from './api';
import { I18nProvider, normalizeLocale, useI18n } from './i18n';
import type { InstallerEvent, InstallerStatus, MailSystem, PasswordPolicy, ScreenType, ServerEngine } from './types';

const DEFAULT_POLICY: PasswordPolicy = {
  min_length: 8,
  require_special: true,
  require_uppercase: true,
  require_number: true,
};

const INITIAL_STATUS: InstallerStatus = {
  phase: 'preparing',
  progress: 0,
  message: '',
  selected_server: null,
  selected_mail: null,
  environment: null,
  error: null,
  language: 'es',
  account: null,
  password_policy: DEFAULT_POLICY,
  panel_login_path: '/login',
  panel_login_url: null,
  server_ready: false,
};

function AppShell() {
  const { t, locale, setLocale } = useI18n();
  const [screen, setScreen] = useState<ScreenType>('preparing');
  const [selectedServer, setSelectedServer] = useState<ServerEngine | null>(null);
  const [selectedMail, setSelectedMail] = useState<MailSystem | null>(null);
  const [status, setStatus] = useState(INITIAL_STATUS);
  const [compareOpen, setCompareOpen] = useState(false);
  const reconnectTimer = useRef<number | undefined>(undefined);
  const completionTimer = useRef<number | undefined>(undefined);
  const skipLanguagePush = useRef(true);
  const localeRef = useRef(locale);
  localeRef.current = locale;

  const applyStatusScreen = useCallback((next: InstallerStatus, delayComplete = false) => {
    setStatus(next);
    setSelectedServer(next.selected_server);
    setSelectedMail(next.selected_mail);
    if (next.language) {
      const normalized = normalizeLocale(next.language);
      if (normalized !== localeRef.current) {
        skipLanguagePush.current = true;
        setLocale(normalized);
      }
    }

    if (next.phase === 'ready') {
      setScreen('selection');
      return;
    }
    if (['downloading', 'installing', 'testing', 'failed'].includes(next.phase)) {
      setScreen('installing');
      return;
    }
    if (next.phase === 'completed' || next.phase === 'account') {
      const go = () => {
        if (!next.selected_mail) {
          setScreen('mail');
          return;
        }
        if (!next.account?.configured) {
          setScreen('account');
          return;
        }
        setScreen('complete');
      };
      if (delayComplete) {
        window.clearTimeout(completionTimer.current);
        completionTimer.current = window.setTimeout(go, 1200);
      } else {
        go();
      }
    }
  }, [setLocale]);

  const handleEvent = useCallback((event: InstallerEvent) => {
    if (event.type === 'snapshot' || event.type === 'progress') {
      applyStatusScreen(event.status, false);
      return;
    }
    if (event.type === 'completed') {
      applyStatusScreen(event.status, true);
      return;
    }
    if (event.type === 'error') {
      applyStatusScreen(event.status, false);
    }
  }, [applyStatusScreen]);

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
      applyStatusScreen(next, false);
    }).catch(() => {
      if (disposed) return;
      setStatus((current) => ({ ...current, phase: 'failed', error: t.statusFetchError }));
      setScreen('installing');
    });
    connect();
    return () => {
      disposed = true;
      socket?.close();
      window.clearTimeout(reconnectTimer.current);
      window.clearTimeout(completionTimer.current);
    };
    // Intentionally omit t.* to avoid remount loops when locale changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [handleEvent, applyStatusScreen]);

  useEffect(() => {
    if (skipLanguagePush.current) {
      skipLanguagePush.current = false;
      return;
    }
    void setLanguage(locale)
      .then((next) => setStatus(next))
      .catch(() => undefined);
  }, [locale]);

  const beginServerInstall = async () => {
    if (!selectedServer) return;
    setScreen('installing');
    setStatus((current) => ({
      ...current,
      phase: 'downloading',
      progress: 0,
      error: null,
      selected_server: selectedServer,
      selected_mail: null,
    }));
    try {
      await startServerInstall(selectedServer);
    } catch (error) {
      setStatus((current) => ({
        ...current,
        phase: 'failed',
        error: error instanceof Error ? error.message : t.unknownError,
      }));
    }
  };

  const beginMailInstall = async () => {
    if (!selectedMail) return;
    setScreen('installing');
    setStatus((current) => ({
      ...current,
      phase: 'downloading',
      progress: 0,
      error: null,
      selected_mail: selectedMail,
    }));
    try {
      await startMailInstall(selectedMail);
    } catch (error) {
      setStatus((current) => ({
        ...current,
        phase: 'failed',
        error: error instanceof Error ? error.message : t.unknownError,
      }));
    }
  };

  const loginUrl = resolvePanelLoginUrl(status);

  return (
    <main className="min-h-screen bg-[#f7f8fa] text-[#111827]">
      <AnimatePresence mode="wait" initial={false}>
        <motion.div
          key={screen}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.45, ease: 'easeInOut' }}
          className="min-h-screen"
        >
          {screen === 'preparing' && <PreparingScreen status={status} />}
          {screen === 'selection' && (
            <ServerSelectionScreen
              selectedServer={selectedServer}
              onSelectServer={setSelectedServer}
              onContinue={beginServerInstall}
              onOpenCompare={() => setCompareOpen(true)}
            />
          )}
          {screen === 'installing' && <InstallingScreen status={status} />}
          {screen === 'mail' && (
            <MailSelectionScreen
              selectedMail={selectedMail}
              onSelectMail={setSelectedMail}
              onContinue={beginMailInstall}
            />
          )}
          {screen === 'account' && (
            <AccountSetupScreen
              initialPolicy={status.password_policy ?? DEFAULT_POLICY}
              language={locale}
              onCompleted={(nextStatus) => {
                if (nextStatus) setStatus(nextStatus);
                setScreen('complete');
              }}
            />
          )}
          {screen === 'complete' && (
            <CompleteScreen
              server={status.selected_server}
              mail={status.selected_mail}
              message={status.message}
              panelLoginUrl={loginUrl}
            />
          )}
        </motion.div>
      </AnimatePresence>
      <CompareModal
        isOpen={compareOpen}
        selectedServer={selectedServer}
        onClose={() => setCompareOpen(false)}
        onSelectServer={(server) => {
          setSelectedServer(server);
          setCompareOpen(false);
        }}
      />
    </main>
  );
}

export default function App() {
  return (
    <I18nProvider>
      <AppShell />
    </I18nProvider>
  );
}
