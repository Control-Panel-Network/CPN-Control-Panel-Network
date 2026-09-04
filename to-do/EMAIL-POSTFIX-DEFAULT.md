# Email: Postfix default MTA

## Rule

If outbound SMTP is skipped or empty during install/account setup, CPN installs and enables **Postfix** on supported Linux guests (dnf/apt) and persists localhost SMTP settings so panel mail has a working path.

## Behavior

1. **Installer / account setup:** no external SMTP input triggers `ensure_postfix_default()` (Postfix package + `systemctl enable --now postfix`), then writes `smtp.json` with `127.0.0.1` port 25 or 587 (`tls=none`) and the recovery address as from-address when valid.
2. **Outbound send:** `mail_outbound` prefers configured SMTP; otherwise uses local Postfix when the unit/ports are ready (forgot-password and setup username email).
3. **Panel Email / Apps:** shows Postfix as the default local MTA. Switching later to external SMTP does **not** remove Postfix unless the operator uninstalls the Email app.
4. **Mailboxes:** every **enabled** mailbox must have valid external SMTP **or** a verified Postfix-local binding. Create/enable rejects invalid SMTP.
5. **Windows Phase A:** Postfix packages are not available; setup continues without breaking the Windows path. Configure external SMTP for outbound mail on Windows.

## Related

- Apps Email stack: `to-do/APPS.md`
- Mailboxes UI: Panel **Email** (`/email`)
