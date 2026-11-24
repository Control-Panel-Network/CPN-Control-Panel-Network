# Panel state in MariaDB (`cpn_panel`, `cpn_mail`)

## What happens automatically

On every panel start (this is how package upgrades, tip upgrades, and fresh
installs all arrive here) the panel runs `panel_db::state::ensure`:

1. First run for an installed version (or after a failed run): **full migration**.
   - Back up `/var/lib/cpn` (tar, verified) and any existing `cpn_*` database
     (dump) under `/home/backup/cpn-panel-db/<unix-time>/` (mode 0700, newest 5 kept).
     If the backup cannot be verified, nothing else runs.
   - Create `cpn_panel` and `cpn_mail`, the accounts `cpn_panel` (DML only on
     those two databases) and `cpn_mail_auth` (SELECT on the `mailbox_auth` view
     only), both `@localhost`. Run checksummed schema migrations.
   - Import documents (copy-only; sources are never edited or deleted). Secret-bearing
     rows are AES-256-GCM encrypted at the application layer with
     `/var/lib/cpn/panel-db.key`, bound to the row identity.
   - Verify every file against its row, import classic mailbox hashes into `cpn_mail`
     entirely inside MariaDB (hashes never pass through the panel process), audit
     the live grants.
2. Later starts of the same version: cheap incremental sync (changed files only).

Fresh installs take the same path with nothing to import.

## What stays on disk (bootstrap)

`panel-db.json` (connection settings), `panel-db.key`, `panel-db-state.json`,
`installer-bootstrap.token`, `mariadb-admin.secret`, `mfa/mfa-encryption.key`,
lock files. Caches, logs, generated PHP, and previews are skipped. Journals
(`*.jsonl`, `audit/`) and SQLite files are reported as deferred.

## Authority

Files remain authoritative in this release; MariaDB is a verified, encrypted,
continuously synced copy plus the live `cpn_mail` passdb. Panel password changes
dual-write the Dovecot hash to `cpn_mail`. Moving individual stores to
DB-primary reads is a follow-up, store by store.

## Operator commands

```
cpn panel-db status
cpn panel-db plan
cpn panel-db migrate [--force]
cpn panel-db verify
cpn panel-db audit
cpn panel-db export --dest /root/cpn-export     # rollback copy, decrypted, 0600
cpn panel-db mail-cutover                       # Dovecot -> cpn_mail, auto rollback on failure
cpn panel-db mail-rollback
```

Opt out: `touch /var/lib/cpn/panel-db.disabled` or `CPN_PANEL_DB=off`.
