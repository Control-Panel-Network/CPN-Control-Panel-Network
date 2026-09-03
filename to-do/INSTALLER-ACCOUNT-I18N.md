# Installer account, i18n, and post-install login

Date: 03/09/2026

## What landed locally

- Installer UI language selector with `en`, `es`, and `nb` locale modules (session-persisted).
- First-account setup after mail install: username (default `admin`), password or generate, policy defaults (min 8, special, uppercase, number), recovery email.
- Bootstrap written to `/var/lib/cpn/panel-bootstrap.json` (mode 0600).
- Complete screen primary action opens panel login (`/login?token=...`), not technical status. Status remains a secondary link.
- Panel login form uses `method="post"` and links to `/forgot-password` (issue #8 partial).
- Installer also serves `/login` and `/forgot-password` HTML while the binary is running.

## How to test on AL9 lab

1. Rebuild and redeploy with `Priv\VirtualBox VMs\rebuild-fix-al9.sh` (or upload source + run that script on the guest).
2. Read token from guest `/tmp/cpn-installer.log`.
3. Open `http://127.0.0.1:8787/?token=TOKEN` (host port forward).
4. Switch language, install server + mail, create account, confirm auto-open of login.
5. `curl -H 'Accept: application/json' "http://127.0.0.1:8787/api/status?token=TOKEN"` should include `language`, `account`, `panel_login_url`, `version`, `server_ready`.

## GitHub issue notes

See `to-do/GITHUB-ISSUES-SWEEP.md`.
