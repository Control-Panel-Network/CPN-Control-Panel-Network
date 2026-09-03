export type ScreenType =
  | 'preparing'
  | 'maintenance'
  | 'selection'
  | 'installing'
  | 'mail'
  | 'account'
  | 'complete';

export type ServerEngine = 'openlitespeed' | 'nginx' | 'caddy';
export type MailSystem = 'snappymail' | 'roundcube' | 'thunderbird';

export type InstallerPhase =
  | 'preparing'
  | 'maintenance'
  | 'ready'
  | 'downloading'
  | 'installing'
  | 'testing'
  | 'completed'
  | 'failed'
  | 'account';

export type MaintenanceAction = 'upgrade' | 'downgrade' | 'repair' | 'config_only';

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

export interface ReleaseAsset {
  name: string;
  browser_download_url: string;
  content_type: string;
  size: number;
}

export interface CpnRelease {
  tag_name: string;
  version: string;
  name: string;
  published_at: string;
  prerelease: boolean;
  draft: boolean;
  html_url: string;
  assets: ReleaseAsset[];
  rpm_asset?: ReleaseAsset | null;
  binary_asset?: ReleaseAsset | null;
}

export interface MaintenancePlan {
  action: MaintenanceAction;
  target_version: string;
  overwrite_paths: string[];
  preserve_paths: string[];
  reset_data: boolean;
  summary: string;
}

export interface MaintenanceInfo {
  existing_install: boolean;
  installed_version: string;
  running_version: string;
  latest_version?: string | null;
  latest_tag?: string | null;
  update_available: boolean;
  downgrade_possible: boolean;
  repo: string;
  source: string;
  releases: CpnRelease[];
  has_manifest: boolean;
  has_bootstrap: boolean;
  plan?: MaintenancePlan | null;
  check_error?: string | null;
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
  maintenance?: MaintenanceInfo | null;
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
