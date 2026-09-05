# Security Policy

## Supported versions

Security fixes are applied on the default branch (`main`) and released when practical. Prefer the latest release or `main` when testing.

| Version | Supported |
|---|---|
| Latest release / `main` | Yes |
| Older releases | Best effort |

CPN is experimental and not ready for production servers. Treat security reports seriously, but expect limited support windows while the project is unfinished.

## Reporting a vulnerability

**Do not** open a public GitHub issue or public pull request for security-sensitive findings.

Prefer one of these private channels:

1. **GitHub Private Vulnerability Reporting** on this repository (Security → Advisories → Report a vulnerability), when enabled.
2. Contact a maintainer via GitHub:
   - [@master3395](https://github.com/master3395)
   - [@KraoESPfan1n](https://github.com/KraoESPfan1n)

Please include:

- Affected version, commit, or package.
- How CPN was run (RPM, DEB, raw binary, container, or Windows Phase A).
- Steps to reproduce.
- Impact and required attacker access.

You can expect an initial response within **7 days** when possible. Fixes may take longer depending on severity and available maintainer time.

## Threat model (summary)

CPN is a privileged web installer that can install and configure system packages and services. Linux package paths currently cover supported/partial Enterprise Linux 8–10 targets, Ubuntu 22.04/24.04, and Debian 12/13; the exact support tiers are maintained in [README.md](README.md). Windows Server 2016+ is a limited Phase A path and does not have Linux package parity.

A single Rust process serves the installer UI, streams progress over WebSockets, and performs privileged installation actions. By default it listens on `127.0.0.1:2087` and prints a temporary access token in the console URL. Use SSH port forwarding for remote access. `--allow-remote` / `CPN_ALLOW_REMOTE=1` binds to `0.0.0.0` and is an explicit operator opt-in to HTTP exposure without TLS.

### Trust assumptions

- The operator already has root or equivalent administrative access on the install host.
- CPN should be used on a dedicated test machine while the project remains alpha.
- Anyone who can reach the installer and obtain a valid temporary token can drive privileged install actions.

### In scope

- Leakage or hard-coding of installer tokens, credentials, signing material, or other secrets.
- Missing/weak authentication on installer actions that should require the temporary token.
- Unsafe download, package, repository, signature, or shell handling that expands remote-code-execution risk beyond the operator's intent.
- Package/update behavior that silently replaces operator-owned configuration or security controls.
- Misleading firewall/network exposure of the installer or managed services.

### Out of scope (typical)

- Treating pre-existing local root access as a remote exploit by itself.
- A vulnerability entirely inside an unmodified third-party package after installation.
- Production deployment contrary to the project's explicit alpha warning.

## Operator guidance

- Download end-user packages from [GitHub Releases](https://github.com/Control-Panel-Network/CPN-Control-Panel-Network/releases), not from untrusted mirrors.
- Do **not** publish the temporary token shown in the installer URL.
- Prefer SSH port forwarding to `--allow-remote`.
- Stop the installer when the session is finished and restrict port `2087`.
- Review existing web/database/mail configuration before using CPN on a non-empty host.
- Verify `SHA256SUMS` and its signature before installing a release artifact as root.

Official release fingerprint:

```text
FE70B9718F63B10BB70A6F70BECBB7488AE5C3E5
```

The matching public key is stored at `packaging/RPM-GPG-KEY-CPN` and included in tagged release artifacts.

## Incident response

1. **Triage:** confirm severity and affected versions; keep sensitive details private.
2. **Mitigate:** revoke/rotate leaked credentials or tokens and shut down exposed installer instances.
3. **Fix:** prepare and review the smallest safe patch.
4. **Release:** publish a fixed version with verification artifacts.
5. **Disclose:** publish/update the advisory after users have a reasonable upgrade path.

## Maintainer notes

- Require MFA on GitHub accounts with write access.
- Keep protected branches and required checks enabled when collaborators are added.
- Never commit installer tokens, private keys, production credentials, or signing secrets.
- Prefer HTTPS repositories and signed native packages; do not introduce `curl | bash` install paths for managed server software when a verifiable repository configuration can be used instead.
