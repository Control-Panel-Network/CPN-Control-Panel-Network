# Panel bootstrap (installer)

## Where first-account data is stored

During installation (after the web server is ready, and after mail selection in the UI flow) the installer writes:

- Path: `/var/lib/cpn/panel-bootstrap.json`
- Mode: `0600`
- Contents: username, recovery email, password policy, language, SHA-256 password hash + salt
- Secrets: guest/local disk only; never commit this file

Hash format: `sha256(salt_hex + "|" + utf8_password)` as lowercase hex. Panel should read this on first boot and may upgrade the algorithm later.

## Login URL after install

Panel Next.js app is not fully wired yet. The installer hosts the interim login landing:

- Primary (auto-opened from CompleteScreen): `http://<host>:2087/login?token=<installer-token>`
- Also exposed as `panel_login_url` in `/api/status`
- Secondary: `http://<host>:2087/status?token=<installer-token>` (technical status)
- Forgot password landing: `/forgot-password`

When Panel ships on its own listener or vhost, keep `/` as login and migrate consumers to that base URL while still reading `/var/lib/cpn/panel-bootstrap.json`.

## Installer UI flow

1. Language (en / es / nb modules under `installer-ui/src/i18n/locales/`)
2. Web server install
3. Mail system install
4. First account + password policy
5. Complete screen auto-opens panel login

## Languages

Drop-in locale files: `installer-ui/src/i18n/locales/{en,es,nb}.ts`. Register in `installer-ui/src/i18n/index.tsx` (`CATALOG` + `SUPPORTED_LOCALES`).
