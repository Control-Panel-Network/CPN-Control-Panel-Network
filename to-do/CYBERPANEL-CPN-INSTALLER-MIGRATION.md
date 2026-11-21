# CyberPanel / classic panel: installer migration path

Generic operator notes for CPN `install.sh` and `cpn-installer` when another panel is already on the host.

## Detection

- CyberPanel is detected as foreign panel id `classic`.
- The installer prints a migration banner on startup (web and `--cli`).
- The web UI shows a **Fresh install** vs **Migrate** step when a panel is detected.

## Recommended path (migrate)

1. `curl -fsSL https://cpn.newstargeted.com/install.sh | sudo bash`
2. `sudo cpn-installer --migrate` (or choose Migrate in the web UI / CLI prompt).
3. Complete the CPN install (reuse LiteSpeed `lsphp*` and existing Postfix/Dovecot when present).
4. `cpn panel-migrate checklist && cpn panel-migrate detect && cpn panel-migrate plan --id classic && cpn panel-migrate apply --id classic --yes`
5. Wipe the old panel only after a verified apply.

## Fresh install on a occupied host

Use only when no migration is planned. Expect package overlap (Remi php-fpm vs `lsphp*`, AppStream postfix vs postfix3).

## CLI flags

- `cpn-installer --migrate`
- `cpn-installer --fresh`

## LiteSpeed hosts

When `lsphp*` is installed, the installer skips Remi modular `php-fpm` to avoid `/var/lib/php/opcache` conflicts.