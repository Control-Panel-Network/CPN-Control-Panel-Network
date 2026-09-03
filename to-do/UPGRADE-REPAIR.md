# CPN installer upgrade, downgrade, and repair

When `cpn-installer` is re-run on a host that already has CPN, it detects the existing install and offers maintenance actions instead of only a fresh install path.

## Detection

Existing install is detected from any of:

- `/var/lib/cpn/install-manifest.json` (preferred)
- `/var/lib/cpn/panel-bootstrap.json`
- RPM package `cpn-installer` (`rpm -q`)
- Binary `/usr/bin/cpn-installer`

Override data root with `CPN_DATA_DIR` (labs / non-root tests).

## Manifest

Path: `/var/lib/cpn/install-manifest.json`

Records:

- `package_version` / `release_tag`
- `source` (`rpm`, `binary`, `local`, `unknown`)
- `core_files` (paths repair may overwrite)
- `preserve_paths` (kept unless `--reset-data`)
- selected server / mail engines when known

Written after a successful server or mail install, and after upgrade / repair / downgrade.

## What upgrade vs repair overwrites

| Action | Behavior |
|---|---|
| **Upgrade** | Resolves latest (or chosen newer) GitHub Release; installs RPM asset if present, else binary asset; updates manifest. |
| **Downgrade** | Same package apply path for an older tag; requires UI confirmation or CLI `--yes`. |
| **Repair** | Re-applies the chosen (default: installed) release with force/replace semantics for core packaged files. |
| **Config only** | No package changes; continues mail / account / panel setup. |

### Overwritten (core)

Typical list from the manifest (optional entries skipped if missing):

- `/usr/bin/cpn-installer`
- `/usr/bin/cpn` (when shipped)
- `cpn-installer.service` unit paths
- `cpn-webmail.service` / `openlitespeed.service` when present

### Preserved by default

- `/var/lib/cpn/panel-bootstrap.json` (first account; never wiped on repair by default)
- `/var/lib/cpn/accounts/`
- `/var/lib/cpn/sites/`
- `/var/lib/cpn/smtp.json`, `/var/lib/cpn/smtp/`, `/var/lib/cpn/secrets/`

Pass `--reset-data` (CLI) or `reset_data: true` (API) to clear those paths. Do not invent or rewrite secrets.

## Package source

Default: GitHub Releases API for `Control-Panel-Network/CPN-Control-Panel-Network`.

Env overrides:

- `CPN_GITHUB_REPO` (owner/name)
- `CPN_PACKAGE_SOURCE` (label only)

Preferred asset: `*cpn-installer*.rpm`. Fallback: binary named `cpn-installer*`.

### Still stubbed / lab fallback

- If a release has **no** RPM or binary asset, the installer errors with guidance to build from git and install the local RPM (current AlmaLinux lab path via `scripts/build-rpm.sh` / VM `cpn-build-v02`).
- Re-running web-server / mail **recipes** as part of repair is not automatic; repair focuses on CPN packaged core files. Re-install engines from the normal UI if needed.
- Operator CLI `cpn version-check` is reserved for the parallel CLI PR; installer already exposes `cpn-installer --version-check` with the same JSON shape intent.

## CLI

```bash
sudo cpn-installer --version
sudo cpn-installer --version-check
sudo cpn-installer --upgrade
sudo cpn-installer --upgrade --to 0.2.0
sudo cpn-installer --repair
sudo cpn-installer --repair --to 0.2.0
sudo cpn-installer --downgrade --to 0.1.0 --yes
sudo cpn-installer --repair --reset-data   # destructive to /var/lib/cpn site data
```

## Web UI + API

When an existing install is detected, phase is `maintenance`.

- `GET /api/version-check?token=...`
- `GET /api/releases?token=...`
- `POST /api/maintenance?token=...` body:

```json
{
  "action": "upgrade",
  "version": "0.2.0",
  "confirm_downgrade": false,
  "reset_data": false
}
```

Actions: `upgrade` | `downgrade` | `repair` | `config_only`.

## Coordination with other PRs

- Login / SMTP and CLI branches: rebase carefully; keep maintenance modules (`manifest`, `releases`, `upgrade`, `cli_maintenance`, `maintenance_api`) self-contained.
- Do not wipe `panel-bootstrap` on repair unless the operator opts into reset.
