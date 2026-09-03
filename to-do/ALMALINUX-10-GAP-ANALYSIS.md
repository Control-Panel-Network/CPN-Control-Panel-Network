# AlmaLinux 10 gap analysis (CPN)

Date: 03/09/2026

## Environment

- Host: Windows + VirtualBox 7.2.8
- Guests: `CPN-AlmaLinux-9` (SSH `2222`, UI `8787`) and `CPN-AlmaLinux-10` (SSH `2223`, UI `8788`)
- Repo: `d:\OneDrive - v-man\Dokumenter\GitHub\CPN-Control-Panel-Network`
- Lab credentials: `D:\OneDrive - v-man\Priv\VirtualBox VMs\CPN-lab-credentials.txt`

## Failures observed on AlmaLinux 10 (before patches)

1. `scripts/build-rpm.sh` refused / mis-documented AlmaLinux 9 only (checks `/etc/almalinux-release` message).
2. `packaging/cpn-installer.spec` with CRLF line endings broke `rpmbuild` `%prep` (`$'\r'`).
3. Caddy COPR repo hard-coded `epel-9` (would fail on EL10).
4. PHP recipe always ran `dnf module enable php:8.1` (EL10 uses AppStream PHP without that module stream).
5. `tests/docker-matrix.sh` hard-coded el9 RPM path and `almalinux:9.8` image.

## Fixes applied

- `build-rpm.sh`: allow AlmaLinux `VERSION_ID` major 9 or 10 via `/etc/os-release`
- `installer.rs`: `almalinux_major()`, dynamic Caddy `epel-$major`, PHP module enable only on EL9
- `docker-matrix.sh`: discover RPM glob + `CPN_TEST_IMAGE` override (default remains `almalinux:9.8`)
- `README.md` / `SECURITY.md` / RPM `%description`: document AlmaLinux 9 and 10
- `.gitattributes`: force LF for shell/spec files

## Verification on AlmaLinux 10

- Built: `cpn-installer-0.1.0-1.el10.x86_64.rpm`
- Installed RPM and started `cpn-installer`
- Host UI: `http://127.0.0.1:8788/?token=...`
- Nginx recipe `phase=completed`; `systemctl nginx` active; default page served
- Browser: mail-selection screen after nginx completion

## Verification on AlmaLinux 9

- Kickstart network install with `shutdown` (avoids DVD reinstall loop); boot then locked to disk only
- OS: AlmaLinux 9.8; SSH `cpn@127.0.0.1:2222` with lab password; sudo NOPASSWD
- Node 22 + rustup installed in guest
- Built from patched sources: `cpn-installer-0.1.0-1.el9.x86_64.rpm`
- Also produced el9 RPM via podman `almalinux:9.8` on the AL10 host as a cross-check
- Host UI: `http://127.0.0.1:8787/?token=...`
- Nginx recipe `phase=completed`; nginx active; host API status completed
- Browser: mail-selection screen after nginx completion

## Notes

- Prefer copying sources to guest local disk (or tarball upload) rather than building only on `vboxsf` / Windows CRLF mounts.
- VirtualBox Guest Additions kernel module may fail to build on some AlmaLinux 9.8 kernel/headers combinations; CPN lab used SFTP/tarball instead of shared-folder builds when `vboxsf` was unavailable.
- Do not leave install ISO/VISO attached with DVD-first boot after kickstart completes.

## Upstream

- Patches live in the local Windows clone. Open a PR upstream only when requested.
