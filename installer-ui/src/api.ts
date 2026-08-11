import type { InstallerEvent, InstallerStatus, MailSystem, ServerEngine } from './types';

const accessToken = new URLSearchParams(window.location.search).get('token') ?? '';
const apiUrl = (path: string) => `${path}?token=${encodeURIComponent(accessToken)}`;

export async function getStatus(): Promise<InstallerStatus> {
  const response = await fetch(apiUrl('/api/status'));
  if (!response.ok) throw new Error('No se pudo consultar el instalador');
  return response.json();
}

export async function startMailInstall(mail: MailSystem): Promise<void> {
  const response = await fetch(apiUrl('/api/install/mail'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ mail }),
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null);
    throw new Error(payload?.error ?? 'No se pudo iniciar la instalación del correo');
  }
}

export async function startServerInstall(server: ServerEngine): Promise<void> {
  const response = await fetch(apiUrl('/api/install/server'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ server }),
  });

  if (!response.ok) {
    const payload = await response.json().catch(() => null);
    throw new Error(payload?.error ?? 'No se pudo iniciar la instalación');
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
