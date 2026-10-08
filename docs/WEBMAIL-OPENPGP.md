# Webmail OpenPGP (classic PGP)

Status: enabled on install/heal in CPN panel (`feat/webmail-openpgp`).
Date: 08/10/2026

This is **classic OpenPGP** in self-hosted webmail (Thunderbird-like public-key encrypt).
It is **not** Proton end-to-end; a separate CPN-Plugins Proton integration may cover that later.

## Support matrix

| Client | Key save (contact public key) | Encrypt-to recipient | Own keypair manage | Backend |
|---|---|---|---|---|
| **Tachyon** (default) | Yes (Settings > OpenPGP / Security) | Yes (compose encrypt) | Yes | Built-in: OpenPGP.js, optional Mailvelope, optional server GnuPG |
| **SnappyMail** | Same as Tachyon | Same | Same | Same (upstream SnappyMail OpenPGP) |
| **NextSnapMail** | Same when lineage heal runs | Same | Same | Same RainLoop/SnappyMail lineage |
| **Roundcube** | Yes (Settings > PGP Keys via Enigma) | Yes | Yes (server keyring) | Enigma + GnuPG; keys under `/var/lib/cpn-webmail/roundcube-enigma` |
| **SOGo** | Not enabled by CPN in this change | - | - | Out of scope / scaffold |

## What CPN enables

### SnappyMail-family (`application.ini`)

- `openpgp = On`
- `gnupg = On` (when server `gpg`/`gpg2` is available; soft-installs `gnupg`/`gnupg2`)
- Applied by `ensure_webmail_openpgp_defaults()` from operator defaults / heal / Roundcube install

### Roundcube

- Plugin list includes `enigma`
- `plugins/enigma/config.inc.php` with `enigma_pgp_homedir` outside the HTTP docroot
- Soft-install GnuPG CLI

## Operator enablement

1. Install webmail Host package (Tachyon recommended): Plugins > Store > Host, or `cpn app install --name tachyon`.
2. Open Email > Webmail (panel notes EN+NO).
3. Heal existing installs: open Email > Webmail (triggers heal) or re-run webmail runtime / `cpn doctor --heal` paths that call SnappyMail operator defaults.
4. User path:
   - Tachyon/SnappyMail: log in > **Settings > OpenPGP / Security** (generate or import keys; import contact public keys from signed mail or `.asc`).
   - Roundcube: log in > **Settings > PGP Keys** (Enigma).

Admin UI path (SnappyMail-family): `/?admin` > Security > Allow OpenPGP (CPN already sets `openpgp = On` in `application.ini`).

## Privacy defaults (honest)

| Mode | Private key location | Encrypt plaintext on server? |
|---|---|---|
| OpenPGP.js (browser) | Browser / optional passphrase-protected backup in webmail data | Prefer no for body when encrypt runs in browser |
| Mailvelope | Browser extension store | No (encrypt before send) |
| GnuPG / Enigma (server) | Server under `/var/lib/cpn-webmail` | Yes (trust the host) |

CPN prefers documenting browser OpenPGP.js/Mailvelope for private keys, while enabling server GnuPG so attachment encrypt and Roundcube Enigma work like a shared host keyring.

## Gaps (not Thunderbird full parity)

- Autocrypt header import/send depends on upstream SnappyMail version; not separately configured by CPN.
- Mailvelope cannot Sign+Encrypt with selectable From key (upstream limitation).
- OpenPGP.js may not encrypt all attachments the way GnuPG does (upstream).
- Clearing browser storage can drop OpenPGP.js keys unless the user imported/backed up from server.
- SOGo OpenPGP not wired.
- No CPN-Plugins `openPgp` package required; upstream features are enough.
- Lab verify: confirm `openpgp = On` in `/var/lib/cpn-webmail/tachyon/.../application.ini` and Enigma plugin on Roundcube labs after deploy of this branch.

## Secrets

Never commit key material. Keys belong only under `/var/lib/cpn-webmail/` (mode 700 for Enigma home). HTTP access to that tree remains denied.
