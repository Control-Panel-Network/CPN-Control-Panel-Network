# CPN OS support matrix

Date: 03/09/2026

## Product reality

CPN is a **Linux** control-panel installer (Rust + RPM, with apt recipes for Ubuntu guests). It is **not** a native Windows Server panel.

| Role | What CPN means by it |
|---|---|
| **Guest OS** | Where `cpn-installer` runs and installs packages (Alma, Rocky, Ubuntu, …) |
| **Host / hypervisor** | Where those guests run (VirtualBox, Hyper-V, Windows Server as host). Documented for labs; no native Windows install path |

## CyberPanel-aligned guest targets

| Guest OS | Status | Package path | Notes |
|---|---|---|---|
| AlmaLinux 10 | **supported** | dnf | Lab-verified earlier (see `ALMALINUX-10-GAP-ANALYSIS.md`) |
| AlmaLinux 9 | **supported** | dnf | Lab-verified; default Docker matrix image |
| AlmaLinux 8 | **partial** | dnf | Detected; PHP module `php:8.0`; needs more lab proof |
| Rocky Linux 9 | **supported** | dnf | Same EL9 recipe family as Alma 9 |
| Rocky Linux 8 | **partial** | dnf | Same EL8 path; needs lab proof |
| RHEL 9 | **partial** | dnf | Allowlisted; subscription/repos are operator responsibility |
| RHEL 8 | **partial** | dnf | Allowlisted; needs lab proof |
| CloudLinux 8 | **partial** | dnf | Detected when `ID=cloudlinux` |
| CentOS Stream 9 | **partial** | dnf | Detected when `ID=centos` major 9 |
| Ubuntu 24.04 | **supported** | apt | Code path present; **lab verification still needed** |
| Ubuntu 22.04 | **supported** | apt | Code path present; **lab verification still needed** |
| Ubuntu 20.04 | **partial** | apt | Allowlisted; older PHP/repos; verify before claiming production |
| Debian | **not yet** | apt (planned) | Clear error; best-effort community |
| openEuler | **not yet** | (planned) | Clear error; best-effort community |
| Other RHEL derivatives | **not yet** | dnf (planned) | Clear error when not in allowlist |
| Windows Server (as guest install target) | **host-only** | n/a | Not a native panel install; may host Linux VMs |
| VirtualBox / Hyper-V | **host-only** | n/a | Hypervisors for Linux guests |

Status meanings (match `src/os_support.rs`):

- **supported**: detection + install recipes implemented for that family
- **partial**: allowlisted; recipes run via family path; less lab evidence
- **not yet**: known CyberPanel/community target; installer refuses with a helpful message
- **host-only**: not a CPN guest install target

## Before this change vs after

| Area | Before | After (this PR) |
|---|---|---|
| OS gate | AlmaLinux 9/10 only | CyberPanel guest allowlist (see table) |
| Package manager | dnf only | dnf (RHEL-family) and apt (Ubuntu) |
| PHP | EL9 module / EL10 AppStream | EL8 `php:8.0`, EL9 `php:8.1`, EL10 AppStream, Ubuntu apt PHP |
| Caddy repo | COPR `epel-$major` | COPR on EL; Cloudsmith apt bootstrap on Ubuntu |
| Packaging | Alma 9/10 RPM | RHEL-family RPM build hosts (Alma/Rocky/RHEL/CentOS 8–10); Ubuntu `.deb` helper script (experimental) |
| Docs | Alma-only README/SECURITY | Matrix doc + README/SECURITY/UI copy |

## Host / hypervisor notes (labs)

### VirtualBox (Windows host)

Existing CPN lab VMs (examples):

- `CPN-AlmaLinux-9`: host SSH `2222`, installer UI `2087` (guest listens on `2087`)
- `CPN-AlmaLinux-10`: host SSH `2223`, installer UI `2088` (host forward to guest `2087`)

Typical NAT port forwards: guest `22` → host `2222`/`2223`, guest `2087` → host `2087` (AL9) or host `2088` (AL10). Older labs that forwarded guest `8787` should update VirtualBox NAT rules to `2087` (or keep a temporary host `8787`→guest `2087` bridge while migrating). Prefer SFTP/tarball into the guest if `vboxsf` or Guest Additions modules fail on some Alma kernels.

Credentials for local labs stay outside the repo (private path), never committed.

### Hyper-V (Windows host / Windows Server)

CyberPanel maintains a Hyper-V lab under `cyberpanel/test/hyperv/` (AlmaLinux-oriented smoke helpers). CPN can reuse the same idea: Generation 2 VM, nested virtualization only if you nest further hypervisors, and external or NAT switch with forwarded `22` / `2087`.

CPN does not ship Hyper-V scripts in this PR. Optional follow-up: mirror a thin `to-do/hyperv/` helper modeled on CyberPanel’s `up.ps1`.

### Windows Server

Use Windows Server as the **hypervisor host** (Hyper-V role) for Alma/Ubuntu guests. Do **not** expect `cpn-installer` to install a native Windows control panel. WSL2 is not a supported guest target for systemd + firewall recipes in this matrix.

### Nested virtualization

Enable nested virt only when the guest itself runs KVM/containers that need it. Plain CPN installer tests usually do not need nested virt.

## Packaging and Docker matrix

- `scripts/build-rpm.sh`: allows AlmaLinux, Rocky, RHEL, CentOS majors 8–10 (not Alma-only).
- `scripts/build-deb.sh`: experimental Ubuntu/Debian binary package helper (not full recipe parity CI yet).
- `tests/docker-matrix.sh`: still RPM + Alma by default; override with `CPN_TEST_IMAGE` (for example `rockylinux:9`). Ubuntu matrix needs a deb artifact first.

## Lab verification still needed

- Rocky Linux 9 full nginx/Caddy/OLS + mail matrix
- Ubuntu 22.04 and 24.04 apt path (nginx, Caddy Cloudsmith, OLS repo script, PHP webmail)
- AlmaLinux 8 / Rocky 8 / CloudLinux 8 smoke
- CentOS Stream 9 smoke
- Hyper-V guest bring-up notes with real port forwards on Windows Server

## Code map

- `src/os_support.rs`: detection, allowlist, support tiers
- `src/install_recipes.rs`: dnf/apt recipes, Caddy/PHP helpers
- `src/installer.rs`: orchestration, progress, webmail
