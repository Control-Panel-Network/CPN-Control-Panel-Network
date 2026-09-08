# Clean install evidence (AL9 VirtualBox)

Date: 08/09/2026
Branch: `fix/installer-e2e-clean`
Guest: CPN-AlmaLinux-9 only (AL10 not started)
No snapshot available; clean wipe used instead.

## Clean-install steps used

1. `git fetch` + hard reset guest checkout to `origin/fix/installer-e2e-clean`
2. `killall cpn-installer`; `rpm -e cpn-installer`; `rm -rf /var/lib/cpn`
3. Build fix-branch RPM via `./scripts/build-rpm.sh` (setsid + ignore SIGHUP)
4. Later iteration: `cargo build --release --locked` and copy binaries to `/usr/bin`
5. Start: `sudo cpn-installer --allow-remote --port 2087`
6. Bootstrap token from root-only file: `/var/lib/cpn/installer-bootstrap.token` (mode 0600, exclusive create)
7. API path: server (OpenLiteSpeed + MariaDB + phpMyAdmin) -> mail (SnappyMail) -> `/api/account/setup`
8. Post-install curls for `/`, `/login`, `/dashboard`, HEAD probes

## Verification results

| Check | Result |
| --- | --- |
| HEAD `/` and `/login` before account | 401 (not 404) |
| GET `/?token=...` SPA | 200 |
| Server install | pass |
| Mail install | pass (installer completed) |
| Account setup | 200; bootstrap token file cleared |
| GET `/` after ready | 302 -> `/login` (no `?token=` required) |
| GET `/login` | 200 Sign in page |
| HEAD `/login` | 200 |
| POST `/login` | 303 -> `/dashboard` + session cookie |
| Host NAT `http://127.0.0.1:2087/` | 302 |
| Host NAT `/login` | 200 |
| OLS `:80` CPN vhost | 200 `CPN OpenLiteSpeed` |

## Krao review follow-ups (08/09/2026)

| Item | Change |
| --- | --- |
| systemd StartLimit* | Moved to `[Unit]`; `scripts/verify-systemd-unit.sh` + CI |
| `--allow-remote` token | Abort startup if persist fails; unit tests for failure path |
| Bootstrap token 0600 | Exclusive `create_new` + mode 0600 + atomic rename; symlink/clobber tests |
| SnappyMail data | `APP_DATA_FOLDER_PATH=/var/lib/cpn-webmail/snappymail/`; deny rules on nginx/Caddy/OLS; HTTP UI + path denial check |
| `:80/:443` conflicts | Stop only confirmed listeners; disable enabled reclaimers; journal prior state + restore on failed install |

## SnappyMail security/functional assertions (installer)

- UI body must not match permission / Error 202 / data-folder errors
- UI body must look like SnappyMail (login markers)
- HTTP fetches of `/data`, `/temp`, `/logs` must not return 200/301/302
- Application data lives under `/var/lib/cpn-webmail/` (outside docroot) with SELinux fcontext best-effort
