# Default `info@` mailbox on Host email

When Host email (or a new mail domain / website with local mail) is installed, CPN creates **`info@<apex>`** as the first mailbox unless the operator opts out.

## When it runs

- Email host package **Start** (`start_email_stack`)
- Installer mail stage (webmail + local Postfix/Dovecot)
- After website create (`after_site_created`) when local mail mode is on

## Opt-out

Setup Wizard → **Install without a mail domain** (`without_mail_domain` / legacy `skip_rdns`).

Also skipped when mail mode is **External**.

## Password

A strong password is generated with the panel password policy. It is stored in `mail-accounts.json` and written once to `mailbox-info-<apex>.initial.pass` under the CPN data directory (mode 600). Existing `info@` mailboxes are never overwritten.
