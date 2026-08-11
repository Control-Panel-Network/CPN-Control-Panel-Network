export type ScreenType = 'preparing' | 'selection' | 'installing' | 'mail' | 'complete';

export type ServerEngine = 'openlitespeed' | 'nginx' | 'caddy';
export type MailSystem = 'snappymail' | 'rainloop' | 'roundcube' | 'thunderbird';

export type InstallerPhase =
  | 'preparing'
  | 'ready'
  | 'downloading'
  | 'installing'
  | 'testing'
  | 'completed'
  | 'failed';

export interface EnvironmentInfo {
  is_vps: boolean;
  virtualization: string | null;
  firewall: string | null;
  port: number;
  addresses: string[];
}

export interface InstallerStatus {
  phase: InstallerPhase;
  progress: number;
  message: string;
  selected_server: ServerEngine | null;
  selected_mail: MailSystem | null;
  environment: EnvironmentInfo | null;
  error: string | null;
}

export type InstallerEvent =
  | { type: 'snapshot'; status: InstallerStatus }
  | { type: 'progress'; status: InstallerStatus }
  | { type: 'log'; line: string; level: 'info' | 'success' | 'error' }
  | { type: 'completed'; status: InstallerStatus }
  | { type: 'error'; status: InstallerStatus };
