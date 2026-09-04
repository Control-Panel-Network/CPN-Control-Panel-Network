import type {
  AccountSetupResponse,
  InstallerEvent,
  InstallerStatus,
  MailSystem,
  PasswordPolicy,
  ServerEngine,
} from './types';

const TOKEN_STORAGE_KEY = 'cpn_install_token';

function readTokenFromUrl(): string {
  return new URLSearchParams(window.location.search).get('token') ?? '';
}

function stripTokenFromUrl(): void {
  const url = new URL(window.location.href);
  if (!url.searchParams.has('token')) {
    return;
  }
  url.searchParams.delete('token');
  const next = `${url.pathname}${url.search}${url.hash}`;
  window.history.replaceState({}, document.title, next);
}

/** Capture bootstrap token once, then keep it out of the address bar (issue #1). */
function resolveAccessToken(): string {
  const fromUrl = readTokenFromUrl();
  if (fromUrl) {
    try {
      sessionStorage.setItem(TOKEN_STORAGE_KEY, fromUrl);
    } catch {
      // Private mode may block storage; Bearer header still works for this page load.
    }
    stripTokenFromUrl();
    return fromUrl;
  }
  try {
    return sessionStorage.getItem(TOKEN_STORAGE_KEY) ?? '';
  } catch {
    return '';
  }
}

let accessToken = resolveAccessToken();
let sessionBootstrapped = false;

function authHeaders(extra?: HeadersInit): Headers {
  const headers = new Headers(extra);
  headers.set('Accept', 'application/json');
  if (accessToken) {
    headers.set('Authorization', `Bearer ${accessToken}`);
    headers.set('X-CPN-Token', accessToken);
  }
  return headers;
}

async function ensureInstallSession(): Promise<void> {
  if (sessionBootstrapped || !accessToken) {
    return;
  }
  sessionBootstrapped = true;
  try {
    await fetch('/api/session', {
      method: 'POST',
      credentials: 'same-origin',
      headers: authHeaders({ 'Content-Type': 'application/json' }),
      body: JSON.stringify({ token: accessToken }),
    });
  } catch {
    // Cookie bootstrap is best-effort; Bearer still authorizes API calls.
  }
}

async function apiFetch(path: string, init: RequestInit = {}): Promise<Response> {
  await ensureInstallSession();
  const headers = authHeaders(init.headers);
  return fetch(path, {
    ...init,
    credentials: 'same-origin',
    headers,
  });
}

async function readError(response: Response, fallback: string): Promise<string> {
  const payload = await response.json().catch(() => null);
  return payload?.error ?? fallback;
}

export async function getStatus(): Promise<InstallerStatus> {
  const response = await apiFetch('/api/status');
  if (!response.ok) throw new Error('status_fetch_failed');
  return response.json();
}

export async function setLanguage(language: string): Promise<InstallerStatus> {
  const response = await apiFetch('/api/language', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
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

export async function setListenPort(
  port: number,
  options?: {
    old_port_policy?: 'redirect_1m' | 'redirect_3m' | 'deny';
    panel_hostname?: string;
  },
): Promise<ListenPortResponse> {
  const body: Record<string, unknown> = { port };
  if (options?.old_port_policy) {
    body.old_port_policy = options.old_port_policy;
  }
  if (options?.panel_hostname !== undefined) {
    body.panel_hostname = options.panel_hostname;
  }
  const response = await apiFetch('/api/listen-port', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
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
  const response = await apiFetch('/api/account/setup', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!response.ok) throw new Error(await readError(response, 'account_failed'));
  return response.json();
}

export async function startMailInstall(mail: MailSystem): Promise<void> {
  const response = await apiFetch('/api/install/mail', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ mail }),
  });
  if (!response.ok) {
    throw new Error(await readError(response, 'mail_install_failed'));
  }
}

export async function startServerInstall(
  server: ServerEngine,
  options?: { database?: import('./types').DatabaseEngine; install_phpmyadmin?: boolean },
): Promise<void> {
  const response = await apiFetch('/api/install/server', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      server,
      database: options?.database ?? 'mariadb',
      install_phpmyadmin: options?.install_phpmyadmin ?? true,
    }),
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
  const response = await apiFetch('/api/maintenance', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!response.ok) {
    throw new Error(await readError(response, 'maintenance_failed'));
  }
}

export function connectInstallerEvents(onEvent: (event: InstallerEvent) => void) {
  void ensureInstallSession();
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  // Prefer cookie session after /api/session; keep Bearer via short-lived storage only.
  // Do not put the install token back into the WebSocket URL (issue #1).
  const socket = new WebSocket(`${protocol}//${window.location.host}/api/events`);

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
  // Panel login must not carry the installer root token (issue #1 / #8).
  return path;
}
