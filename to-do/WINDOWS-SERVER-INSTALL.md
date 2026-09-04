# Windows Server install (CPN Phase A)

Date: 04/09/2026

## Feasibility summary

| Windows Server | CurrentBuild (approx) | CPN status | Reason |
|---|---|---|---|
| 2012 | 9200 | **Not supported** | Modern Rust (edition 2024) and current MSVC toolchains do not target this OS. No Phase A binary path. |
| 2012 R2 | 9600 | **Not supported** | Same toolchain / runtime gap as 2012. Prefer Hyper-V Linux guests for full recipes. |
| 2016 | 14393+ | **Partial (Phase A)** | Installer UI, Windows service, account bootstrap under `C:\ProgramData\CPN`. |
| 2019 | 17763+ | **Partial (Phase A)** | Same as 2016. |
| 2022 / newer | 20348+ | **Partial (Phase A)** | Same as 2016. |
| Windows 10/11 client | any | **Unsupported** | Server SKUs only. |

**Fully supported Linux guests** (dnf/apt recipes) remain the primary path. Windows Phase A is intentional and limited.

## What Phase A ships

1. Native `cpn-installer.exe` and `cpn.exe` for `x86_64-pc-windows-msvc`
2. Data directory: `C:\ProgramData\CPN` (override with `CPN_DATA_DIR`)
3. Default listen port **2087** (same as Linux)
4. `packaging/windows/Install-Cpn.ps1`: copy binaries, create `CPNInstaller` service, optional firewall rule
5. Clear refusal of Linux dnf/apt web and mail recipes in the UI/API

## What Phase A does **not** ship

- Automatic Nginx / Caddy / OpenLiteSpeed install (Linux only)
- Automatic PHP webmail (SnappyMail / Roundcube) stack on Windows
- Full panel site wiring parity with AlmaLinux recipes
- MSI (zip + PowerShell install is the packaging path for now)

## Install steps

1. Build on a Windows machine with Rust + Node, or download CI artifacts:

```powershell
.\packaging\windows\Build-WindowsZip.ps1
```

2. On the target Windows Server 2016+ host (elevated PowerShell):

```powershell
Expand-Archive cpn-windows-x86_64.zip -DestinationPath C:\Temp\cpn
cd C:\Temp\cpn
.\Install-Cpn.ps1 -Port 2087
# LAN access (HTTP without TLS; lab/operator opt-in):
# .\Install-Cpn.ps1 -Port 2087 -AllowRemote
```

3. Open `http://127.0.0.1:2087/?token=...` (token is printed by the service logs / first console run). Complete first-account bootstrap.

4. Uninstall:

```powershell
.\Uninstall-Cpn.ps1
# Also wipe accounts / SMTP secrets:
# .\Uninstall-Cpn.ps1 -RemoveData
```

## Phase B (planned): IIS / reverse proxy notes

Not automated yet. Operators can place a reverse proxy in front of the CPN installer/panel on port 2087:

- **IIS** with URL Rewrite + ARR, or Http.Sys reserved URL, terminating TLS on 443
- Document root / site creation helpers may land later; do not expect Linux vhost writers to run on Windows
- Site JSON records via `cpn site` still work under `C:\ProgramData\CPN\sites`

## Phase C (later): mail / webmail

PHP webmail on Windows is limited (IIS + PHP FastCGI, or containers). Local IMAP/SMTP backends (Postfix/Dovecot-style) are Linux-oriented. Prefer:

- External SMTP for password reset (already supported via `smtp.json` in the data dir)
- Linux guest for full mail recipes

## Firewall

`Install-Cpn.ps1` creates an inbound TCP rule for the chosen port unless `-SkipFirewall` is set. Confirm with:

```powershell
Get-NetFirewallRule -DisplayName 'CPN Installer TCP *'
```

## Related files

- `src/os_support.rs`: Windows detection and 2012 vs 2016+ tiers
- `src/paths.rs`: `C:\ProgramData\CPN` default
- `packaging/windows/*.ps1`: build, install, uninstall
- `to-do/OS-SUPPORT-MATRIX.md`: matrix row
