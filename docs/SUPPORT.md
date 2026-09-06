# Platform Support and Existing Hosts

CPN is still alpha software. Support tiers describe how much validation a platform currently receives; they do not imply production-readiness for every web, mail, database, or plugin combination.

## Operating-system support

| Guest | Status | Package path | Notes |
|---|---|---|---|
| AlmaLinux 9 / 10 | Supported | RPM / dnf | Primary EL targets |
| Rocky Linux 9 | Supported | RPM / dnf | Automated Rocky smoke path |
| Ubuntu 22.04 / 24.04 | Supported | DEB / apt | Primary apt targets |
| AlmaLinux 8 | Partial | dnf recipes | Maintenance-era EL8; no native release RPM while its OpenSSL 1.1 toolchain cannot build CPN's WebAuthn dependency |
| Rocky Linux 8 / 10 | Partial | RPM / dnf | EL8 uses recipes only; EL10 has a native RPM; less CPN smoke evidence |
| RHEL 8 / 9 / 10 | Partial | RPM / dnf | EL8 uses recipes only; EL9/10 use matching RPMs and require working subscriptions/repos |
| CloudLinux 8 / 9 / 10 | Partial | RPM / dnf | EL8 uses recipes only; EL9/10 share RPM recipes; no public CPN lab matrix |
| CentOS Stream 9 / 10 | Partial | RPM / dnf | Shared EL recipes |
| Debian 12 / 13 | Partial | DEB / apt | Implemented apt path; matrix coverage is being expanded |
| Windows Server 2016+ | Partial | Windows ZIP | Phase A only; no Linux web/mail package parity |

## Refused targets

CPN currently refuses new installs on:

- Ubuntu 20.04
- Debian 11
- openEuler
- Windows Server 2012 / 2012 R2
- unknown or unsupported distributions

Ubuntu 20.04 and Debian 11 are no longer treated as normal new-install targets. openEuler is detected but refused because CPN's third-party web/mail repository stack has not been validated there.

## Existing software and repeat installs

CPN is designed to become increasingly idempotent rather than assuming every server is empty.

Current behavior includes:

- Nginx, Caddy, and OpenLiteSpeed recipes detect an existing selected server and reuse it instead of deliberately installing a second copy.
- Existing Caddy/LiteSpeed repository files that CPN must change are backed up through the install journal.
- OpenLiteSpeed configuration changes are journaled, and CPN avoids deleting administrator-owned systemd units while adopting an existing installation.
- MariaDB/MySQL defaults detect an existing database service and avoid replacing it with a conflicting engine.
- PHP setup keeps a sufficiently new existing PHP installation instead of blindly switching module streams.
- Firewall cleanup removes only rules CPN recorded as its own; pre-existing firewalld/UFW rules remain operator-owned.

## Migration limitations

CPN is not an automatic migration engine for arbitrary production hosts.

Manual review may still be required when a machine has:

- custom virtual-host layouts;
- nonstandard package locations;
- manually managed repositories;
- another service already bound to ports CPN expects;
- heavily customized OpenLiteSpeed, Nginx, Caddy, PHP, database, firewall, or mail configuration.

Use a test VM/VPS and keep backups while CPN remains in alpha.

For installation commands, see the root [README](../README.md). For operator commands after installation, see [CLI.md](CLI.md).
