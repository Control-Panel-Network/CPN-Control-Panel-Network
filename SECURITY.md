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

- Affected version, commit, or package (for example RPM build)
- How CPN was run (binary, RPM, container)
- Steps to reproduce
- Impact (what an attacker could do)

You can expect an initial response within **7 days** when possible. Fixes may take longer depending on severity and available maintainer time.

## Threat model (summary)

CPN is a web installer that prepares server components on AlmaLinux 9 and AlmaLinux 10 (native RPM or privileged Docker/Podman). A single Rust process serves an HTTP UI, streams progress over WebSockets, and can install system packages and related services. The installer listens on `0.0.0.0:8787` and prints a temporary access token in the console URL.

### Trust assumptions

- The operator has root (or equivalent) on the machine where CPN runs.
- CPN is intended for a dedicated test machine during this development stage.
- Anyone who can reach the installer URL **and** hold a valid temporary token can drive install actions.

### In scope

- Leakage or hard-coding of the temporary installer token in logs, commits, docs, or public issues
- Unauthenticated or weakly authenticated access to installer actions when a token is required
- Unsafe handling of download URLs, package install steps, or scripts that could lead to unexpected remote code execution beyond the operator’s intent
- Secrets or credentials committed to the repository
- Misleading firewall or network exposure of port `8787` without clear operator control

### Out of scope (typical)

- Treating local root (or already-privileged) access on the install host as a remote exploit by itself
- Misconfiguration of third-party packages after install when the issue is entirely upstream of CPN
- Running CPN on production hosts contrary to the project’s experimental warning

## Operator guidance

- Do **not** publish the temporary token that appears in the installer URL.
- Prefer a dedicated test VPS or VM while CPN is unfinished.
- Close or restrict port `8787` when you are done with an install session.
- Stop the installer process when finished so the temporary UI is no longer reachable.

## Incident response (lightweight checklist)

1. **Triage:** confirm severity and affected versions; keep discussion private until a fix is ready.
2. **Mitigate:** revoke or rotate leaked tokens; shut down exposed installer instances; pin or bump vulnerable dependencies if needed.
3. **Fix:** develop on a private fork or private advisory branch when available.
4. **Release:** publish a fixed version or tag; note the issue in release notes without unnecessary exploit detail.
5. **Disclose:** publish or update the advisory after users have a reasonable path to upgrade.

## Maintainer notes

- Enable MFA on GitHub accounts with write access.
- Keep the default branch protected with required checks when collaborators are added.
- Never commit installer tokens, private keys, or production credentials.
