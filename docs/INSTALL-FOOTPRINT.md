# Default install footprint

Measured **19/10/2026**. Latest published GitHub Release on `Control-Panel-Network/CPN-Control-Panel-Network` is **v1.4.4** (`gh release list`, marked Latest, published `2026-10-15T22:44:25Z`, changelog date 16/10/2026).

The default install is the matching **cpn-installer** package (RPM or deb). That download is about **7.3 MiB** on Enterprise Linux and about **6.8 MiB** on Debian or Ubuntu. MariaDB, the web server, and phpMyAdmin are separate OS packages and are not included in that figure.

## Measured sizes

Bytes come from the GitHub Release asset `size` field (`gh release view v1.4.4 --repo Control-Panel-Network/CPN-Control-Panel-Network --json assets`). MiB is bytes / 1048576. MB is bytes / 1000000. Both are rounded to 3 decimal places. The byte count is exact.

| What | Size | How measured | Date |
| --- | --- | --- | --- |
| Panel RPM, EL9 (`cpn-installer-1.4.4-1.el9.x86_64.rpm`) | 7655324 bytes (7.301 MiB, 7.655 MB) | GitHub Release asset size, [v1.4.4](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases/tag/v1.4.4) | 19/10/2026 |
| Panel RPM, EL10 (`cpn-installer-1.4.4-1.el10.x86_64.rpm`) | 7652153 bytes (7.298 MiB, 7.652 MB) | Same GitHub Release asset size | 19/10/2026 |
| Panel deb (`cpn-installer_1.4.4_amd64.deb`) | 7165776 bytes (6.834 MiB, 7.166 MB) | Same GitHub Release asset size | 19/10/2026 |
| Checksums (`SHA256SUMS`) | 719 bytes | Same GitHub Release asset size; body saved with curl and matches 719 bytes | 19/10/2026 |
| Checksum signature (`SHA256SUMS.asc`) | 833 bytes | Same GitHub Release asset size | 19/10/2026 |
| Package signing key (`RPM-GPG-KEY-CPN`) | 1769 bytes | Same GitHub Release asset size | 19/10/2026 |
| Unpacked installer binary asset (`cpn-installer`) | 20521672 bytes (19.571 MiB, 20.522 MB) | Same GitHub Release asset size. This is the loose ELF, not the RPM the one-liner installs | 19/10/2026 |
| Unpacked CLI binary asset (`cpn`) | 3691736 bytes (3.521 MiB, 3.692 MB) | Same GitHub Release asset size | 19/10/2026 |
| Public install script `https://cpn.newstargeted.com/install.sh` | 18808 bytes | `Content-Length` and curl download byte count, HTTP 200, `application/x-sh`. Script was not executed | 19/10/2026 |
| GitHub raw fallback `.../stable/scripts/install.sh` | 19140 bytes | `Content-Length` and curl download byte count, HTTP 200. Differs from the site copy on this date | 19/10/2026 |
| Docker Hub `master3395/cpn-installer:latest` and `:almalinux9` | 116017330 bytes compressed (110.643 MiB, 116.017 MB) | Hub API `full_size` and OCI manifest layer sum (linux/amd64). Same digest for both tags | 19/10/2026 |
| Docker Hub `:almalinux10` | 130198400 bytes compressed (124.166 MiB, 130.198 MB) | Same Hub API field and OCI layer sum | 19/10/2026 |
| Docker Hub `:ubuntu26.04` | 74465288 bytes compressed (71.016 MiB, 74.465 MB) | Same Hub API field and OCI layer sum | 19/10/2026 |

Windows zip on the same release, for completeness: `cpn-windows-x86_64.zip` is 9955550 bytes (9.494 MiB, 9.956 MB).

## Internet and bandwidth

`scripts/install.sh` (the file behind the public one-liner) downloads four release files, then installs that local package:

1. The OS package (EL9 RPM, EL10 RPM, or amd64 deb).
2. `SHA256SUMS` (719 bytes).
3. `SHA256SUMS.asc` (833 bytes) when the release has it.
4. `RPM-GPG-KEY-CPN` (1769 bytes).

File bytes for the public one-liner plus those four files (GitHub API JSON for the release list is extra and was not sized):

| Guest package | Bytes | MiB | MB |
| --- | ---: | ---: | ---: |
| EL9 RPM + script from cpn.newstargeted.com | 7677453 | 7.322 | 7.677 |
| EL10 RPM + script from cpn.newstargeted.com | 7674282 | 7.319 | 7.674 |
| amd64 deb + script from cpn.newstargeted.com | 7187905 | 6.855 | 7.188 |

`dnf install` / `apt-get install` of that local package can still fetch declared dependencies and, on apt, index files. Those mirror totals were not measured.

MariaDB, OpenLiteSpeed (or another web server), and phpMyAdmin are chosen later by `cpn-installer`. Their download size depends on the distro mirrors. This page does not give a mirror total.

After the package is installed, runtime bandwidth is not a fixed number. The monthly `cpn bandwidth-reset` timer (`src/panel_ops_bandwidth_reset.rs`) only resets local counters. It does not download a payload. Larger GitHub downloads (release checks, tip upgrades) run when an operator uses Version Management or `upgrade.sh`, not as a silent bulk fetch from a finished default package install.

## Stack on top of the panel package

Previously recorded operator figures, not re-measured on 19/10/2026, and not stated in `docs/INSTALL.md` or the README: a core panel install around 17 MB, and a host with MariaDB, OpenLiteSpeed, and phpMyAdmin around 320 MB. Those describe a stack on top of the package. They are not the v1.4.4 RPM or deb size in the table above.

## Not measured

- Installed disk after `rpm` or `dpkg` (`rpm -qi` / `du` on a lab). No lab was changed for this page.
- Unpacked Docker image size on disk. The table is compressed registry layer bytes. Image config blobs were 4662 bytes (`latest` / `almalinux9`), 4668 bytes (`almalinux10`), and 8454 bytes (`ubuntu26.04`). Those blobs are metadata, not the unpacked rootfs.
- Git checkout size. The story here is the published package, not the git tree.
- Distro mirror totals for MariaDB, the web server, and phpMyAdmin.
