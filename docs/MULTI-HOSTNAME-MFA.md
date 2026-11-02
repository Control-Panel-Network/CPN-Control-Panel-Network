# Multi-hostname panels and MFA

One CPN **install** is one data directory (`/var/lib/cpn` by default): one account database, one TOTP/passkey store under `mfa/`, one session store.

Extra DNS names that reverse-proxy to the same listen port are **aliases**, not separate panels. Separate servers (or separate data dirs) are separate installs with separate MFA.

## Primary URL vs aliases

| Setting | Purpose |
|---|---|
| `panel_hostname` / `panel_public_url` | Primary public URL for emails, MOTD, `cpn panel url`, phone-home, and HTTPS front setup |
| `panel_host_aliases.json` | Extra hostnames allowed for Host/Origin (CSRF) and WebAuthn origin allowlisting |

```bash
cpn network show
sudo cpn network add-host-alias --hostname vmi123.example.net
sudo cpn network list-host-aliases
sudo cpn network remove-host-alias --hostname vmi123.example.net
```

CPN does **not** force a browser redirect from an alias to the primary hostname. If Contabo (or Cloudflare/OLS) sends you to another name, that is the web server or DNS front, not the panel app rewriting login links (login forms use relative `/login` paths).

## Passkeys (WebAuthn)

Browsers bind passkeys to a **Relying Party ID** derived from the page hostname (RP ID is the Host without port; loopback uses `localhost`).

- Enroll on `https://panel.example.com` → RP ID `panel.example.com`.
- Open `https://other.example.net` on the **same** install → RP ID `other.example.net`.
- The first passkey will **not** satisfy the second hostname. That is browser security, not a CPN bug.

Persisted files:

- `/var/lib/cpn/mfa/webauthn-origins.json` : remembered allowed origins (never shrinks on upgrade)
- `/var/lib/cpn/mfa/webauthn-rpid` : loopback RP ID (`localhost`)

`panel_public_url` and host aliases are added as **allowed origins** for ceremonies on the current Host. They do not make one passkey work on an unrelated domain.

**Recommendation:** pick one primary HTTPS hostname for day-to-day login and passkeys. Use aliases for emergency or alternate DNS; use TOTP there, or register a second passkey on that hostname.

## TOTP

Authenticator apps are bound to the account secret, not the hostname. TOTP works on every hostname that reaches the same install. Session cookies are host-only (`Path=/`, no `Domain=`), so signing in on hostname A does not carry the session to hostname B; sign in again (password + TOTP) on B.

## One install vs many

| Setup | MFA |
|---|---|
| One install, one hostname | One passkey RP ID; TOTP once |
| One install, several aliases | Same TOTP; passkeys per hostname (or stick to primary) |
| Two installs (two servers / data dirs) | Separate MFA entirely |

## Live proxy tips

- Terminate TLS on 443 (OLS/nginx/Caddy) and proxy to the plain-HTTP listen port (default `2087`).
- Never use `https://IP:2087` (listen port is HTTP only).
- Optional IP backup: `cpn network set-ip-backup --on` (HTTP on `SERVER_IP:listen_port`).
