# CPN install and upgrade options

CPN is **alpha-only** for now. Prefer a disposable test host, keep backups, and read [Platform Support](SUPPORT.md) before production-like installs.

## Preferred one-liners (host, then GitHub raw)

Use `bash` (or `sh`) with process substitution. Arguments after `<(...)` are passed to the script (`$1`, `$2`, ...).

### Install (newest published Release with packages)

```bash
bash <(curl -fsSL https://cpn.newstargeted.com/install.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh || wget -O - https://cpn.newstargeted.com/install.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh)
```

### Upgrade (existing `cpn-installer`)

```bash
bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh || curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh || wget -O - https://cpn.newstargeted.com/upgrade.sh || wget -O - https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh)
```

`upgrade.sh` upgrades the package, then **automatically runs** `cpn-installer --upgrade` when a panel install is detected. Repo-root `preUpgrade.sh` is a GitHub alias of the same script.

Use `curl -fsSL` so a down or non-200 host fails and the next URL runs.

## Pin a git ref or Release (`-b` / `--ref`)

```bash
bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh) -b 1.0.0-dev
bash <(curl -fsSL https://cpn.newstargeted.com/install.sh) --branch v0.2.6-alpha.22
bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh) --ref v0.2.6-alpha.21 --bypass
```

Behavior:

| Input | Effect |
| --- | --- |
| `-b REF`, `--branch REF`, `--ref REF` | Sets `CPN_BRANCH`. Resolves packages via a matching **GitHub Release** tag (`REF` or `vREF`). |
| `CPN_BRANCH` / `CPN_REF` | Same as `-b` when the flag is omitted. |
| `CPN_RELEASE_TAG` | Explicit Release tag; wins over `-b` for package selection. |
| No pin | Newest non-draft Release that has `SHA256SUMS` + matching EL/arch RPM or arch `.deb` (prereleases included unless `CPN_STABLE_ONLY=1`). |

If `-b` is set but **no Release** exists for that ref, the script exits with a clear English error. It does not invent packages from source.

Shared helpers live in [`scripts/cpn-bootstrap-lib.sh`](../scripts/cpn-bootstrap-lib.sh) (also served at `https://cpn.newstargeted.com/cpn-bootstrap-lib.sh`).

## Raw GitHub URLs (correct form)

These work:

```text
https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/install.sh
https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/stable/scripts/upgrade.sh
https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/<ref>/scripts/install.sh
https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/<ref>/scripts/upgrade.sh
```

These **do not** work as raw script URLs (browser HTML / 404):

```text
https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/v1.0.0-dev/install.sh
https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/1.0.0-dev/upgrade.sh
```

Pin scripts to a branch by putting the branch name in the **raw** path, then pass `-b` only when you also want that Release (if published):

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/Control-Panel-Network/CPN-Control-Panel-Network/1.0.0-dev/scripts/upgrade.sh) -b 1.0.0-dev
```

### `1.0.0-dev` tracking branch

`1.0.0-dev` is an optional **long-lived tracking branch** for future 1.x work. It is **not** a stable 1.0 product release. Prefer published `0.2.x-alpha.*` Releases for packages today. Create or update the branch from `stable` when maintainers want a named tip:

```bash
git fetch origin
git branch 1.0.0-dev origin/stable   # first time
git push -u origin 1.0.0-dev
# optional alias name:
git branch v1.0.0-dev 1.0.0-dev && git push -u origin v1.0.0-dev
```

## Environment variables

| Variable | Purpose |
| --- | --- |
| `CPN_RELEASE_TAG` | Pin exact GitHub Release tag (example: `v0.2.6-alpha.22`). |
| `CPN_BRANCH` / `CPN_REF` | Same intent as `-b` (Release resolution). |
| `CPN_STABLE_ONLY=1` | Skip prereleases once a non-prerelease Latest exists. |
| `CPN_REQUIRE_GPG` | Default `1`: require `SHA256SUMS.asc` + expected fingerprint. |
| `CPN_ALLOW_UNSIGNED=1` | Lab only: allow missing GPG assets. |
| `CPN_UPGRADE_BYPASS=1` | Same as `upgrade.sh --bypass`. |
| `CPN_GITHUB_REPO` | Override `owner/name` (default Control-Panel-Network/CPN-Control-Panel-Network). |

## Docker refresh (`--bypass`)

Opt-in only. Refreshes CPN-managed compose under `/var/lib/cpn/docker` and containers labeled `com.cpn.managed=1`. Preserves volumes. Does not touch unlabeled user containers.

```bash
bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh) --bypass
# or
CPN_UPGRADE_BYPASS=1 bash <(curl -fsSL https://cpn.newstargeted.com/upgrade.sh)
# or after package tip is installed:
sudo cpn-installer --upgrade --bypass
```

## Retag migration (leftover `1.0.0` / `1.0.1` RPM)

Former GitHub `v1.0.0` / `v1.0.1` tags were renamed onto the `0.2.x-alpha` line. Labs that still have package identity **1.0.0** or **1.0.1** sort **newer** than `0.2.6` in RPM/semver compares.

Official `upgrade.sh` / `install.sh` and `cpn-installer --upgrade` treat that as a **retag migration**: replace with tip `0.2.x` via `rpm -Uvh --oldpackage` (erase+install fallback) or `apt-get --allow-downgrades`. Non-interactive when you run the official upgrade path.

After a successful upgrade (or when Version Management / `--version-check` runs as root), CPN **reconciles** a stale `install-manifest.json` that still says `1.0.0`/`1.0.1` whenever the live RPM is already on `0.2.x`. The Version Management "Installed package" line then matches the RPM/Cargo identity (including prerelease, for example `0.2.6-alpha.21`).

## GitHub Releases cache (Version Management)

Release lists are cached on disk at `/var/lib/cpn/github-releases-cache.json` (override data root with `CPN_DATA_DIR`).

| Setting | Default | Env |
|---|---|---|
| Cache TTL | 30 minutes | `CPN_RELEASES_CACHE_TTL_SECS` |
| Manual check min interval | 60 seconds | `CPN_RELEASES_CHECK_MIN_INTERVAL_SECS` |

On GitHub HTTP 403/429, CPN serves the last good cache when present and shows an English rate-limit note (no tight retries). Optional token for higher limits: `CPN_GITHUB_TOKEN` or `GITHUB_TOKEN`, or file `/var/lib/cpn/secrets/github-token` (mode 600; never commit).

## What upgrade preserves vs refreshes

**Preserved (never wiped by upgrade/repair unless `--reset-data`):**

- Website docroots under `/home/<domain>/` (and subdomain homes)
- Panel accounts, plans, billing/plan assignments, bootstrap, MFA, SSL prefs
- Plugin/app configs and instance data under `/var/lib/cpn` and site `apps/` trees
- Email / webmail data (`/var/lib/cpn-webmail`, live SnappyMail/Roundcube trees)
- Docker stacks and volumes by default (opt-in refresh only with `--bypass` for CPN-managed compose)

**May refresh (already installed only; never drops databases):**

- CPN core package (`cpn-installer` / `cpn`) to the selected release
- MariaDB server packages via `dnf upgrade` / `apt-get --only-upgrade`, then restart if active
- OpenLiteSpeed and PHP/php-fpm packages when already present, then restart active units
- Stale CPN packaging/staging under `/var/tmp/cpn-*` and similar allowlisted paths

Post-upgrade verification checks panel `/login`, web server units, and MariaDB when present.

## After package install

```bash
sudo cpn-installer          # interactive Web vs SSH/CLI
sudo cpn-installer --web
sudo cpn-installer --cli
sudo cpn panel url          # print login URL from live config
```

See also [Releases and Verification](RELEASES.md), [CLI](CLI.md), and [Changelog](CHANGELOG.md).
