import type { LocaleMessages } from "../types";

const en: LocaleMessages = {
  languageName: "English",
  languageLabel: "Language",
  preparingEyebrow: "SERVER SETUP",
  preparingTitle: "Getting everything ready...",
  preparingAria: "Preparing the installer",
  initialMessage: "Getting everything ready...",
  readyMessage: "The system is ready to continue",
  unknownError: "Unknown error",
  statusFetchError: "Could not query the installer",
  startServerError: "Could not start the installation",
  startMailError: "Could not start the mail installation",
  selectServerTitle: "Choose your web server",
  selectServerIntro:
    "Pick the web engine that best fits your project. Supported Linux guests include AlmaLinux, Rocky, Ubuntu, and related EL targets. Windows Server and hypervisors host those guests; they are not native panel installs. You can change the engine later from the panel.",
  listenPortLabel: "Installer listen port",
  listenPortHint:
    "Default is 2087 (Cloudflare-friendly, same family as cPanel WHM HTTPS). Lab installs may use another free port such as 8787. Ports below 1024 usually need root.",
  listenPortApply: "Save port",
  listenPortSaved: "Listen port preference saved.",
  listenPortRestartHint:
    "Port saved. Restart the installer with --port {port} to apply. This session stays on the current port.",
  listenPortInvalid: "Enter a port between 1 and 65535.",
  oldPortPolicyLabel: "What should happen on the old port?",
  oldPortPolicyHint:
    "Choose how long (if at all) the previous port should redirect browsers to the new port after you restart.",
  oldPortPolicyRedirect1m: "Keep redirect from the old port for 1 month",
  oldPortPolicyRedirect3m: "Keep redirect from the old port for 3 months",
  oldPortPolicyDeny: "Deny access on the old port (no redirect)",
  panelHostnameLabel: "Panel hostname (optional subdomain)",
  panelHostnameHint:
    "Use a DNS name such as panel.example.com for HTTPS login without a port in the URL. You must point DNS at this server and terminate TLS on 443 with a reverse proxy to the CPN listen port.",
  panelHostnamePlaceholder: "panel.example.com",
  panelPublicUrlLabel: "External panel URL (optional)",
  panelPublicUrlHint:
    "Base URL browsers and password-reset emails should use (scheme + host + optional port). For VirtualBox NAT, set the host forward, for example http://127.0.0.1:2089. This is preferred over the hostname when set.",
  panelPublicUrlPlaceholder: "http://127.0.0.1:2089",
  networkSave: "Save networking",
  selectLabel: "Select",
  compareLink: "Not sure which one to pick? Compare features",
  compareTitle: "Web server comparison",
  compareIntro: "Find the option that fits your project.",
  closeLabel: "Close",
  continueLabel: "Continue",
  nothingInstallsYet: "Nothing is installed until you press Continue.",
  databaseTitle: "Database defaults",
  databaseHint:
    "MariaDB and phpMyAdmin install by default with the web server. You can skip either. CPN uses MariaDB as the only MySQL-compatible host database.",
  databaseMariadb: "MariaDB (default)",
  databaseNone: "Skip local database engine",
  databasePhpmyadmin: "Also install phpMyAdmin (default on)",
  phpVersionTitle: "PHP version",
  phpVersionHint:
    "Default is PHP 8.5 on AlmaLinux/RHEL 9+. Other supported versions remain selectable. If 8.5 packages are missing, CPN falls back and records the choice.",
  serverOpenlitespeedDesc:
    "High performance with low resource use. Ideal for WordPress and busy sites thanks to built-in LSCache.",
  serverNginxDesc:
    "Industry standard. Robust, very stable, and excellent for static content and reverse proxy roles.",
  serverCaddyDesc:
    "Modern web server with automatic HTTPS by default. Minimal configuration and strong security out of the box.",
  compareOpenlitespeedDesc:
    "Performance, Apache rewrite compatibility, and LSCache.",
  compareNginxDesc:
    "Stable server widely used for static content and reverse proxy.",
  compareCaddyDesc: "Modern server with simple config and automatic HTTPS.",
  compareOpenlitespeedFeatures: [
    "Built-in LSCache",
    "Great for WordPress",
    "Low resource use",
  ],
  compareNginxFeatures: [
    "Maximum stability",
    "Excellent reverse proxy",
    "Broad documentation",
  ],
  compareCaddyFeatures: [
    "Automatic HTTPS",
    "Simple Caddyfile",
    "Secure defaults",
  ],
  selectMailTitle: "Choose your mail system",
  selectMailIntro:
    "Install a webmail client or desktop mail app for this server.",
  installingTitle: "Installing",
  installingSubtitle: "Please keep this window open.",
  phaseDownloading: "Downloading",
  phaseInstalling: "Installing",
  phaseTesting: "Verifying services and configuration",
  phaseFailed: "Failed",
  completeEyebrow: "INSTALLATION COMPLETE",
  completeTitle: "Everything is ready",
  completeSummaryBoth:
    "{server} and {mail} are installed and passed their checks.",
  completeSummaryServer: "{server} is installed and passed its checks.",
  completeSummaryReady: "The server is ready.",
  openPanelLogin: "Open panel login",
  technicalStatus: "View technical status",
  backToInstaller: "Back to installer",
  openingPanelHint: "Opening the panel login page...",
  accountEyebrow: "FIRST ACCOUNT",
  accountTitle: "Create the administrator account",
  accountIntro:
    "This account signs in to the panel. Choose a unique username (names like admin/root/support are reserved). Leave the password blank to auto-generate a strong one shown once at the end.",
  usernameLabel: "Username",
  usernameHint:
    "Required. Reserved names (admin, root, support, …) are blocked. Letters (including Å), numbers, and symbols are allowed.",
  usernamePlaceholder: "panelowner",
  passwordLabel: "Password",
  passwordConfirmLabel: "Confirm password",
  passwordHint:
    "Leave blank to auto-generate. Otherwise use at least 8 characters with an uppercase letter and a number. Special characters are allowed but not required. Common weak passwords are blocked.",
  generatePassword: "Generate and fill password",
  useOwnPassword:
    "Leave both password fields empty to auto-generate, or enter your own.",
  generatedPasswordNote: "Copy this password now. It is shown only once.",
  copyPassword: "Copy",
  copied: "Copied",
  emailLabel: "Recovery email",
  emailHint: "Used for the forgotten-password flow in the panel.",
  emailPlaceholder: "you@example.com",
  policyTitle: "Password policy",
  policyMinLength: "Minimum length",
  policyRequireSpecial: "Require a special character",
  policySpecialOptional: "Special characters optional",
  policyRequireUpper: "Require an uppercase letter",
  policyRequireNumber: "Require a number",
  saveAccount: "Save account and continue",
  accountSaving: "Saving...",
  accountSaved: "Account saved",
  passwordMismatch: "Passwords do not match",
  accountError: "Could not save the account",
  smtpOptionalTitle: "Outbound email (optional)",
  smtpOptionalHint:
    "Configure external SMTP for account notices, or leave it empty to use local Postfix on Linux (installed automatically). Windows needs external SMTP. Secrets stay on this server.",
  smtpEnableLabel: "Configure external SMTP for outbound mail",
  smtpHostLabel: "SMTP host",
  smtpPortLabel: "Port",
  smtpTlsLabel: "Encryption",
  smtpTlsStarttls: "STARTTLS",
  smtpTlsTls: "TLS",
  smtpTlsNone: "None (lab only)",
  smtpFromLabel: "From address",
  smtpUserLabel: "SMTP username",
  smtpPasswordLabel: "SMTP password",
  smtpSendUsernameLabel:
    "Email the username to the recovery address during setup",
  smtpSendUsernameHint:
    "Sends username and login URL. Password is omitted unless you opt in below.",
  smtpIncludePasswordLabel:
    "Also include the password in that email (not recommended)",
  smtpIncludePasswordHint:
    "Only enable if you accept sending the password in plaintext email.",
  maintenanceEyebrow: "EXISTING INSTALL",
  maintenanceTitle: "Upgrade, repair, or continue",
  maintenanceIntro:
    "CPN is already installed on this host. Choose how to proceed. Repair overwrites core package files only.",
  maintenanceUpdateAvailable:
    "Installed {installed}. Newer release available: {latest}.",
  maintenanceUpToDate:
    "Installed version {version}. No newer stable release detected.",
  maintenanceChooseVersion: "Release / tag",
  maintenanceConfirmDowngrade:
    "I understand this will downgrade to an older release.",
  maintenanceOverwrite: "Will overwrite (core)",
  maintenancePreserve: "Preserved by default",
  maintenanceUpgradeLatest: "Upgrade to latest",
  maintenanceChooseVersionAction: "Apply chosen version",
  maintenanceRepair: "Repair / overwrite core files",
  maintenanceConfigOnly: "Continue configuration only",
  maintenanceBusy: "Working... keep this window open.",
};

export default en;
