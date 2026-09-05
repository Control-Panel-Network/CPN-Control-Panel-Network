#Requires -Version 5.1
<#
.SYNOPSIS
  Install CPN installer binaries and register a Windows service (Phase A).

.DESCRIPTION
  Copies cpn-installer.exe and cpn.exe into Program Files\CPN, creates
  C:\ProgramData\CPN, and registers a delayed-auto service that runs the
  installer UI.

  A Windows Firewall inbound rule is created only when -AllowRemote is used.
  The default loopback-only install does not need an inbound firewall rule.

  Supported: Windows Server 2016 and later (build >= 14393), Phase A only.
  Not supported: Windows Server 2012 / 2012 R2 (modern Rust toolchain).

.PARAMETER SourceDir
  Directory containing cpn-installer.exe and cpn.exe (default: script folder).

.PARAMETER Port
  Listen port (default 2087).

.PARAMETER AllowRemote
  Bind 0.0.0.0 instead of 127.0.0.1. This exposes the HTTP installer to the network.

.PARAMETER SkipFirewall
  Do not create the CPN Windows Firewall inbound rule even with -AllowRemote.
#>
[CmdletBinding()]
param(
    [string]$SourceDir = $PSScriptRoot,
    [ValidateRange(1, 65535)]
    [int]$Port = 2087,
    [switch]$AllowRemote,
    [switch]$SkipFirewall
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-WindowsBuild {
    try {
        return [int](Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -Name CurrentBuild).CurrentBuild
    } catch {
        return 0
    }
}

function Get-ProductName {
    try {
        return [string](Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -Name ProductName).ProductName
    } catch {
        return 'Windows'
    }
}

if (-not (Test-IsAdministrator)) {
    throw 'Install-Cpn.ps1 must run elevated (Run as administrator).'
}

$build = Get-WindowsBuild
$product = Get-ProductName
Write-Host "Detected: $product (build $build)"

if ($product -notmatch 'Server') {
    throw "CPN Windows install targets Windows Server only. Detected: $product"
}

if ($build -gt 0 -and $build -lt 14393) {
    throw @"
Windows Server 2012 / 2012 R2 (build $build) is not supported.
CPN Phase A requires Windows Server 2016 or later (CurrentBuild >= 14393).
Use a supported Linux guest under Hyper-V for the full web/mail package path, or upgrade the host OS.
See README.md for the current support tiers.
"@
}

$installerSrc = Join-Path $SourceDir 'cpn-installer.exe'
$cliSrc = Join-Path $SourceDir 'cpn.exe'
if (-not (Test-Path -LiteralPath $installerSrc)) {
    throw "Missing binary: $installerSrc"
}
if (-not (Test-Path -LiteralPath $cliSrc)) {
    throw "Missing binary: $cliSrc"
}

$installRoot = Join-Path $env:ProgramFiles 'CPN'
$dataRoot = Join-Path $env:ProgramData 'CPN'
New-Item -ItemType Directory -Force -Path $installRoot | Out-Null
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null

Copy-Item -LiteralPath $installerSrc -Destination (Join-Path $installRoot 'cpn-installer.exe') -Force
Copy-Item -LiteralPath $cliSrc -Destination (Join-Path $installRoot 'cpn.exe') -Force

$portFile = Join-Path $dataRoot 'listen_port'
Set-Content -LiteralPath $portFile -Value "$Port" -Encoding ascii

$exe = Join-Path $installRoot 'cpn-installer.exe'
$args = @('--port', "$Port")
if ($AllowRemote) {
    $args += '--allow-remote'
}
# sc.exe create binary path must quote paths containing spaces.
$binPath = "`"$exe`" $($args -join ' ')"

$serviceName = 'CPNInstaller'
$existing = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "Updating existing service $serviceName..."
    if ($existing.Status -eq 'Running') {
        Stop-Service -Name $serviceName -Force -ErrorAction SilentlyContinue
    }
    sc.exe delete $serviceName | Out-Null
    Start-Sleep -Seconds 2
}

Write-Host "Creating service $serviceName..."
sc.exe create $serviceName binPath= $binPath start= delayed-auto DisplayName= "CPN Installer" | Out-Null
sc.exe description $serviceName "CPN Control Panel Network installer UI (Phase A)" | Out-Null
sc.exe failure $serviceName reset= 86400 actions= restart/60000/restart/60000/restart/60000 | Out-Null

$ruleName = "CPN Installer TCP $Port"
$cpnRules = Get-NetFirewallRule -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like 'CPN Installer TCP *' }

# Remove only CPN-named rules left by a previous CPN Windows install. A loopback
# install should not leave a host-wide inbound exception behind.
$cpnRules | ForEach-Object {
    Remove-NetFirewallRule -Name $_.Name -ErrorAction SilentlyContinue
}

if ($AllowRemote -and -not $SkipFirewall) {
    New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Action Allow -Protocol TCP -LocalPort $Port | Out-Null
    Write-Host "Firewall rule added for remote mode: $ruleName"
} elseif ($AllowRemote -and $SkipFirewall) {
    Write-Warning 'Remote bind enabled but CPN did not create a Windows Firewall rule (-SkipFirewall).'
} else {
    Write-Host 'Loopback-only mode: no inbound Windows Firewall rule is required.'
}

Start-Service -Name $serviceName
Write-Host ''
Write-Host 'CPN Phase A installed.'
Write-Host "  Binaries: $installRoot"
Write-Host "  Data:     $dataRoot"
Write-Host "  Service:  $serviceName (port $Port)"
if ($AllowRemote) {
    Write-Host '  Bind:     0.0.0.0 (HTTP; restrict this port to trusted networks)'
} else {
    Write-Host '  Bind:     127.0.0.1 (recommended; use SSH/tunnel equivalent for remote access)'
}
Write-Host ''
Write-Host 'Complete account bootstrap in the installer UI.'
Write-Host 'Windows support is Phase A only; Linux web/mail package recipes are not available on Windows.'
