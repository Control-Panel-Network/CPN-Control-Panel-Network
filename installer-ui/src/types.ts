export type ScreenType = 'preparing' | 'domain' | 'dns' | 'selection' | 'installing' | 'mail' | 'complete';
export type ServerEngine = 'openlitespeed' | 'nginx' | 'caddy';
export type MailSystem = 'snappymail' | 'rainloop' | 'roundcube' | 'thunderbird';
export type InstallerPhase = 'preparing' | 'ready' | 'configuring' | 'downloading' | 'installing' | 'testing' | 'completed' | 'failed_rolled_back' | 'failed_partial' | 'cancelled';
export type SetupStage = 'domain' | 'dns' | 'server' | 'mail' | 'complete';
export type DnsProvider = 'local' | 'cloudflare';

export interface EnvironmentInfo {
  is_vps: boolean;
  virtualization: string | null;
  firewall: string | null;
  port: number;
  addresses: string[];
  remote_access: boolean;
}

export interface InstallerStatus {
  phase: InstallerPhase;
  stage: SetupStage;
  progress: number;
  message: string;
  domain: string | null;
  domain_is_cloudflare: boolean;
  dns_provider: DnsProvider | null;
  cloudflare_connected: boolean;
  selected_server: ServerEngine | null;
  installed_server: ServerEngine | null;
  selected_mail: MailSystem | null;
  installed_mail: MailSystem | null;
  environment: EnvironmentInfo | null;
  panel_url: string | null;
  panel_admin_email: string | null;
  panel_admin_password: string | null;
  error: string | null;
  failed_phase: InstallerPhase | null;
}

export interface DomainValidation {
  valid: boolean;
  resolvable: boolean;
  cloudflare: boolean;
  normalized: string | null;
  nameservers: string[];
  error: string | null;
}

export type InstallerEvent =
  | { type: 'snapshot'; status: InstallerStatus }
  | { type: 'progress'; status: InstallerStatus }
  | { type: 'log'; line: string; level: 'info' | 'success' | 'error' }
  | { type: 'completed'; status: InstallerStatus }
  | { type: 'error'; status: InstallerStatus };
