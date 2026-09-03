import type {
  AccountSetupResponse,
  InstallerEvent,
  InstallerStatus,
  MailSystem,
  PasswordPolicy,
  ServerEngine,
} from './types';

const accessToken = new URLSearchParams(window.location.search).get('token') ?? '';
const apiUrl = (path: string) => `${path}?token=${encodeURIComponent(accessToken)}`;

async function readError(response: Response, fallback: string): Promise<string> {
  const payload = await response.json().catch(() => null);
  return payload?.error ?? fallback;
}

export async function getStatus(): Promise<InstallerStatus> {
  const response = await fetch(apiUrl('/api/status'), {
    headers: { Accept: 'application/json' },
  });
  if (!response.ok) throw new Error('status_fetch_failed');
  return response.json();
}

export async function setLanguage(language: string): Promise<InstallerStatus> {
  const response = await fetch(apiUrl('/api/language'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify({ language }),
  });
  if (!response.ok) throw new Error(await readError(response, 'language_failed'));
  return response.json();
}

export interface ListenPortResponse {
  status: InstallerStatus;
  listen_port: number;
  preferred_listen_port: number;
  restart_required: boolean;
  message: string;
}

export async function setListenPort(port: number): Promise<ListenPortResponse> {
  const response = await fetch(apiUrl('/api/listen-port'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify({ port }),
  });
  if (!response.ok) throw new Error(await readError(response, 'listen_port_failed'));
  return response.json();
}

/** @deprecated alias kept for older call sites */
export const setInstallerLanguage = setLanguage;

export async function setupAccount(payload: {
  username?: string;
  password?: string;
  generate_password: boolean;
  recovery_email: string;
  password_policy: PasswordPolicy;
  language?: string;
  smtp?: {
    host: string;
    port?: number;
    tls_mode?: 'starttls' | 'tls' | 'none';
    from_address: string;
    username?: string;
    password?: string;
  };
  send_username_email?: boolean;
  include_password_in_email?: boolean;
}): Promise<AccountSetupResponse> {
  const response = await fetch(apiUrl('/api/account/setup'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!response.ok) throw new Error(await readError(response, 'account_failed'));
  return response.json();
}

export async function startMailInstall(mail: MailSystem): Promise<void> {
  const response = await fetch(apiUrl('/api/install/mail'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ mail }),
  });
  if (!response.ok) {
    throw new Error(await readError(response, 'mail_install_failed'));
  }
}

export async function startServerInstall(server: ServerEngine): Promise<void> {
  const response = await fetch(apiUrl('/api/install/server'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ server }),
  });
  if (!response.ok) {
    throw new Error(await readError(response, 'server_install_failed'));
  }
}

export async function startMaintenance(payload: {
  action: import('./types').MaintenanceAction;
  version?: string;
  confirm_downgrade?: boolean;
  reset_data?: boolean;
}): Promise<void> {
  const response = await fetch(apiUrl('/api/maintenance'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!response.ok) {
    throw new Error(await readError(response, 'maintenance_failed'));
  }
}

export function connectInstallerEvents(onEvent: (event: InstallerEvent) => void) {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const socket = new WebSocket(`${protocol}//${window.location.host}${apiUrl('/api/events')}`);

  socket.addEventListener('message', (message) => {
    try {
      onEvent(JSON.parse(message.data) as InstallerEvent);
    } catch {
      // Ignore malformed frames and keep the connection alive.
    }
  });

  return socket;
}

export function resolvePanelLoginUrl(status: InstallerStatus): string {
  if (status.panel_login_url) return status.panel_login_url;
  const path = status.panel_login_path || '/login';
  if (path.startsWith('http')) return path;
  const token = new URLSearchParams(window.location.search).get('token') ?? '';
  return `${path}?token=${encodeURIComponent(token)}`;
}
