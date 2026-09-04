# Panel authentication

Date: 04/09/2026

## What ships in the installer

The `cpn-installer` process owns panel login on the listen port (default `2087`):

| Route | Behaviour |
|-------|-----------|
| `GET /login` | Sign-in form (no installer token when `panel-bootstrap.json` exists) |
| `POST /login` | Verifies username/password against `/var/lib/cpn/panel-bootstrap.json` and `accounts/*.json` (PBKDF2, legacy SHA-256 still verifies and upgrades) |
| Success | Sets HttpOnly `cpn_panel_session` cookie (`SameSite=Lax`; `Secure` only on HTTPS / `X-Forwarded-Proto: https`), redirects to `/dashboard` |
| Failure | Re-renders the login form with an i18n error (en / es / nb) |
| `GET /dashboard` | Requires a valid session (optional `?preview=1` without session) |
| `GET /panel` | Redirects to `/dashboard` |
| `GET`/`POST` `/logout` and `/api/logout` | Clears the session cookie and returns to `/login` |

Remember me remains username-only (browser `localStorage`); passwords are never stored client-side.

Session HMAC secret order: `CPN_PANEL_SESSION_SECRET`, then `/var/lib/cpn/panel-session.secret` (created mode `0600`), then the in-process installer token, then a local-dev fallback.

## How the Panel UI is served

Lab and packaged installs serve the Panel dashboard **from the installer binary** at `/dashboard` (HTML that matches the Next.js Panel layout). No separate Node process is required on the guest.

The Next.js app under `Panel/` remains the reference UI and can run standalone (`npm run build && npm start`) against the same bootstrap file and cookie name when operators prefer a Node host. Cookie format matches `Panel/src/lib/auth.ts` so both paths can share sessions when they use the same secret.

## Responsive shell

Breakpoints used by the installer-served Panel and the Next.js `Panel/` app:

| Viewport | Sidebar | Metric cards | Health / activity |
|----------|---------|--------------|-------------------|
| ≥1024px (desktop) | Always visible | 3 columns | 2 columns |
| 680px to 1023px (tablet) | Off-canvas drawer; hamburger opens it | 2 columns | 1 column |
| &lt;680px (phone) | Off-canvas drawer; hamburger opens it | 1 column | 1 column |

On smaller screens the sidebar does not disappear without a control: the mobile header exposes an accessible hamburger (`aria-expanded`, `aria-controls`) plus backdrop click and Escape to close.

## Lab check

1. Open `http://127.0.0.1:2087/login` (AL9) or `:2088` (AL10 host forward).
2. Sign in with the panel account (not the installer `?token=`).
3. Expect a redirect to `/dashboard` with sidebar and "Signed in as …".
4. Narrow the viewport below 1024px: hamburger opens the drawer; overlay or Escape closes it.
5. `/logout` returns to `/login` and clears the cookie.

Installer SPA upgrade/repair at `/` still requires `?token=` when the phase is `maintenance`.
