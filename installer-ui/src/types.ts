export type ScreenType =
  | 'preparing'
  | 'selection'
  | 'installing'
  | 'mail'
  | 'account'
  | 'complete';

export type ServerEngine = 'openlitespeed' | 'nginx' | 'caddy';
export type MailSystem = 'snappymail' | 'roundcube' | 'thunderbird';

export type InstallerPhase =
  | 'preparing'
  | 'ready'
  | 'downloading'
  | 'installing'
  | 'testing'
  | 'completed'
  | 'failed'
  | 'account';

export interface EnvironmentInfo {
  is_vps: boolean;
  is_container?: boolean;
  virtualization: string | null;
  firewall: string | null;
  port: number;
  addresses: string[];
}

export interface PasswordPolicy {
  min_length: number;
  require_special: boolean;
  require_uppercase: boolean;
  require_number: boolean;
}

export interface AccountPublic {
  username: string;
  recovery_email: string;
  configured: boolean;
}

export interface InstallerStatus {
  phase: InstallerPhase;
  progress: number;
  message: string;
  selected_server: ServerEngine | null;
  selected_mail: MailSystem | null;
  environment: EnvironmentInfo | null;
  error: string | null;
  language?: string;
  account?: AccountPublic | null;
  password_policy?: PasswordPolicy;
  panel_login_path?: string;
  panel_login_url?: string | null;
  version?: string;
  server_ready?: boolean;
  mail_client_ready?: boolean;
  mail_backend_ready?: boolean;
  external_ports_configured?: boolean;
  access_note?: string | null;
  mail_releases?: Array<{ id: string; label: string; version: string; released_on: string }>;
}

export type InstallerEvent =
  | { type: 'snapshot'; status: InstallerStatus }
  | { type: 'progress'; status: InstallerStatus }
  | { type: 'log'; line: string; level: 'info' | 'success' | 'error' }
  | { type: 'completed'; status: InstallerStatus }
  | { type: 'error'; status: InstallerStatus };

export interface AccountSetupResponse {
  account: AccountPublic;
  generated_password?: string | null;
  panel_login_url?: string | null;
  setup_email_sent?: boolean;
  setup_email_error?: string | null;
}
