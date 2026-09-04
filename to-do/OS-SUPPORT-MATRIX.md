# CPN OS support matrix

Date: 04/09/2026

## Why some rows stay Partial / Not yet

Detection and dnf/apt recipes cover many Linux guests. That is not the same as full lab proof on every guest. Windows Server has a separate **Phase A** path (installer UI + bootstrap) without Linux package parity.

Status meanings (match `src/os_support.rs`):

- **supported**: detection + install recipes implemented **and** smoke evidence (lab VM and/or `tests/docker-matrix.sh`)
- **partial**: allowlisted; recipes run via the family path, or Windows Phase A (UI + service + account bootstrap); less smoke evidence, or an external blocker remains
- **not yet**: known target outside the installable allowlist (installer refuses with a helpful message)
- **host-only**: hypervisor for Linux guests; not a CPN install target by itself

## Product reality

CPN is primarily a **Linux** control-panel installer (Rust + RPM/apt). Windows Server 2016+ can run **Phase A** (native installer UI and account data under `C:\ProgramData\CPN`). It does **not** get dnf/apt web/mail recipe parity.

| Role | What CPN means by it |
|---|---|
| **Guest OS** | Where `cpn-installer` runs (Linux guests with recipes, or Windows Server Phase A) |
| **Host / hypervisor** | Where those guests run (VirtualBox, Hyper-V). Documented for labs |

## Guest targets

| Guest OS | Status | Package path | Evidence / notes |
|---|---|---|---|
| AlmaLinux 10 | **supported** | dnf | Lab VM verified (SSH 2223 / UI 2088) |
| AlmaLinux 9 | **supported** | dnf | Lab VM verified (SSH 2222 / UI 2087); default Docker matrix image |
| AlmaLinux 8 | **partial** | dnf | Recipes + Remi PHP 8.2; promote after nginx matrix smoke |
| Rocky Linux 9 | **supported** | dnf | Same EL9 recipe family; CI/os-matrix nginx smoke |
| Rocky Linux 8 | **partial** | dnf | Same EL8 path; promote after matrix smoke |
| RHEL 9 | **partial** | dnf | Allowlisted; **subscription/repos are operator responsibility** (no CPN-owned RHEL entitlement) |
| RHEL 8 | **partial** | dnf | Same subscription blocker as RHEL 9 |
| CloudLinux 8 | **partial** | dnf | Detected when `ID=cloudlinux`; **no public ISO/lab image in this workspace** |
| CentOS Stream 9 | **partial** | dnf | Detected when `ID=centos` major 9; promote after matrix smoke |
| Ubuntu 24.04 | **supported** | apt | Apt recipes + OLS apt keyring bootstrap; lab/matrix verification in progress |
| Ubuntu 22.04 | **supported** | apt | Apt recipes + OLS apt keyring bootstrap; lab/matrix verification in progress |
| Ubuntu 20.04 | **partial** | apt | Allowlisted (focal OLS suite exists); older PHP/repos; thinner evidence |
| Debian 11/12/13 | **partial** | apt | Detection + Ubuntu-like apt path (nginx/Caddy/OLS/PHP); not full matrix yet |
| openEuler 20-24 | **partial** | dnf | Detection + dnf family path; package names may diverge; no lab ISO here |
| Other RHEL derivatives | **not yet** | dnf (planned) | Clear error when not in allowlist |
| Windows Server 2016+ | **partial** | Windows Phase A | Native `cpn-installer.exe`, service, `C:\ProgramData\CPN`; no dnf/apt. See `to-do/WINDOWS-SERVER-INSTALL.md` |
| Windows Server 2012 / 2012 R2 | **not yet** | n/a | Modern Rust / MSVC does not support these hosts; use Hyper-V Linux guests |
| VirtualBox / Hyper-V | **host-only** | n/a | Hypervisors for Linux guests |

## Before vs after Windows Phase A

| Area | Before | After |
|---|---|---|
| Windows Server as install target | **host-only** | **partial** Phase A for 2016+; **not yet** for 2012/2012 R2 |
| Data directory | `/var/lib/cpn` only | Platform path (`C:\ProgramData\CPN` on Windows) |
| Packaging | RPM / deb | Also `packaging/windows/` zip + PowerShell |

## Host / hypervisor notes (labs)

### VirtualBox (Windows host)

Existing CPN lab VMs:

- `CPN-AlmaLinux-9`: host SSH `2222`, installer UI `2087`
- `CPN-AlmaLinux-10`: host SSH `2223`, installer UI `2088` (host forward to guest `2087`)

Credentials for local labs stay outside the repo (private path), never committed.

### Hyper-V / Windows Server

Windows Server can:

1. Act as **Hyper-V host** for Alma/Ubuntu guests (full Linux recipes), and/or
2. Run **Phase A** natively on Server 2016+ (installer UI + bootstrap only)

WSL2 is not a supported guest target for systemd + firewall recipes.

## Packaging and Docker matrix

- `scripts/build-rpm.sh`: RHEL-family RPM build hosts (Alma/Rocky/RHEL/CentOS 8-10)
- `scripts/build-deb.sh` / `scripts/docker-build-deb.sh`: Ubuntu/Debian `.deb` helper
- `packaging/windows/Build-WindowsZip.ps1`: Windows zip (`cpn-installer.exe`, `cpn.exe`, install scripts)
- `tests/docker-matrix.sh`: multi-image via `CPN_TEST_IMAGES` (default `almalinux:9.8`); uses docker or podman

```bash
# Rocky 9 + Alma 8 nginx-only (RPM already built)
CPN_TEST_IMAGES="rockylinux:9 almalinux:8" \
CPN_TEST_SCOPE=server CPN_TEST_SERVERS=nginx \
./tests/docker-matrix.sh

# Ubuntu 22.04 (build .deb first)
./scripts/docker-build-deb.sh
CPN_TEST_IMAGE=ubuntu:22.04 CPN_TEST_SCOPE=server CPN_TEST_SERVERS=nginx \
./tests/docker-matrix.sh
```

## Lab verification notes (04/09/2026)

- **AL9 / AL10 VirtualBox labs**: SSH reachable (ports 2222 / 2223). Host-only evidence for AlmaLinux 9/10 remains valid.
- **Nested podman on AL10**: `almalinux:8` / `rockylinux:9` nginx matrix did **not** complete in this pass (systemd guest bring-up flaky under nested containers). Do not treat that as a recipe failure.
- **GitHub `os-matrix.yml`**: Rocky 9 nginx smoke on `main` pushes that touch OS paths; extended images via `workflow_dispatch` (not on untrusted PRs; issue #7).
- **Ubuntu `.deb`**: `scripts/docker-build-deb.sh` added; full Ubuntu nginx smoke still pending extended matrix run.
- **Windows**: CI builds `x86_64-pc-windows-msvc`; install scripts under `packaging/windows/`. Promote beyond Partial after lab smoke on Server 2019/2022.

## Still blocked from full **supported**

| Guest | Why not fully supported yet |
|---|---|
| AlmaLinux 8 / Rocky 8 / CentOS Stream 9 | Recipes ready; keep **partial** until nginx matrix smoke is green |
| RHEL 8/9 | Needs a real Red Hat subscription and CDN repos; UBI alone is not a full panel guest |
| CloudLinux 8 | No CloudLinux ISO/entitlement in the lab; detection only |
| Ubuntu 20.04 | Older LTS; keep Partial until dedicated smoke |
| Debian / openEuler | Recipe path exists; full nginx/Caddy/OLS/mail matrix not finished |
| Windows Server 2016+ | Phase A only; IIS helpers (Phase B) and mail (Phase C) not shipped |
| Windows Server 2012 / 2012 R2 | Toolchain / runtime not supported |
| Extra lab VMs | AL9/AL10 already use significant disk/RAM; create more guests only when needed |

## Code map

- `src/os_support.rs`: detection, allowlist, support tiers, apt codenames, Windows builds
- `src/paths.rs`: platform data directory defaults
- `src/install_recipes.rs`: dnf/apt recipes, Caddy/OLS/PHP helpers
- `src/install_server.rs`: orchestration for web server install (refuses Linux recipes on Windows)
- `packaging/windows/`: zip + service install scripts
- `tests/docker-matrix.sh`: functional matrix
- `.github/workflows/os-matrix.yml`: privileged Rocky/extended smoke (not on untrusted PRs)
- `to-do/WINDOWS-SERVER-INSTALL.md`: Windows operator guide
