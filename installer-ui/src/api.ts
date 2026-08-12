import type { DnsProvider, DomainValidation, InstallerEvent, InstallerStatus, MailSystem, ServerEngine } from './types';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, { credentials: 'same-origin', ...init });
  if (!response.ok) {
    const payload = await response.json().catch(() => null);
    throw new Error(payload?.error ?? await response.text().catch(() => '') ?? 'La operación no pudo completarse');
  }
  return response.status === 204 ? undefined as T : response.json();
}

const jsonPost = (body?: unknown): RequestInit => ({
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: body === undefined ? undefined : JSON.stringify(body),
});

export const getStatus = () => request<InstallerStatus>('/api/status');
export const validateDomain = (domain: string) => request<DomainValidation>('/api/domain/validate', jsonPost({ domain }));
export const configureDns = (provider: DnsProvider) => request<InstallerStatus>('/api/dns/configure', jsonPost({ provider }));
export const startCloudflareOAuth = () => request<{ authorization_url: string }>('/api/dns/cloudflare/start', jsonPost());
export const startMailInstall = (mail: MailSystem) => request<void>('/api/install/mail', jsonPost({ mail }));
export const startServerInstall = (server: ServerEngine) => request<void>('/api/install/server', jsonPost({ server }));

export function connectInstallerEvents(onEvent: (event: InstallerEvent) => void) {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const socket = new WebSocket(`${protocol}//${window.location.host}/api/events`);
  socket.addEventListener('message', (message) => {
    try { onEvent(JSON.parse(message.data) as InstallerEvent); } catch { /* Ignore malformed frames. */ }
  });
  return socket;
}
