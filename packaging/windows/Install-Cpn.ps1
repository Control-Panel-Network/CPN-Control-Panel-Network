#Requires -Version 5.1
<#
.SYNOPSIS
  Install CPN installer binaries and register a Windows service (Phase A).

.DESCRIPTION
  Copies cpn-installer.exe and cpn.exe into Program Files\CPN, creates
  C:\ProgramData\CPN, opens TCP 2087 in Windows Firewall (optional), and
  registers a delayed-auto service that runs the installer UI.

  Supported: Windows Server 2016 and later (build >= 14393).
  Not supported: Windows Server 2012 / 2012 R2 (modern Rust toolchain).

.PARAMETER SourceDir
  Directory containing cpn-installer.exe and cpn.exe (default: script folder).

.PARAMETER Port
  Listen port (default 2087).

.PARAMETER AllowRemote
  Bind 0.0.0.0 instead of 127.0.0.1.

.PARAMETER SkipFirewall
  Do not create a Windows Firewall inbound rule.
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
Use a Linux guest under Hyper-V for full dnf/apt recipes, or upgrade the host OS.
See to-do/WINDOWS-SERVER-INSTALL.md
"@
}

$installerSrc = Join-Path $SourceDir 'cpn-installer.exe'
$cliSrc = Join-Path $SourceDir 'cpn.exe'
if (-not (Test-Path -LiteralPath $installerSrc)) {
    throw "Missing binary: $installerSrc"
}
if (-not (Test-Path -LiteralPath $cliSrc)) {
    Write-Warning "cpn.exe not found in $SourceDir; installing installer binary only."
}

$installRoot = Join-Path $env:ProgramFiles 'CPN'
$dataRoot = Join-Path $env:ProgramData 'CPN'
New-Item -ItemType Directory -Force -Path $installRoot | Out-Null
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null

Copy-Item -LiteralPath $installerSrc -Destination (Join-Path $installRoot 'cpn-installer.exe') -Force
if (Test-Path -LiteralPath $cliSrc) {
    Copy-Item -LiteralPath $cliSrc -Destination (Join-Path $installRoot 'cpn.exe') -Force
}

$portFile = Join-Path $dataRoot 'listen_port'
Set-Content -LiteralPath $portFile -Value "$Port" -Encoding ascii

$exe = Join-Path $installRoot 'cpn-installer.exe'
$args = @("--port", "$Port")
if ($AllowRemote) {
    $args += '--allow-remote'
}
# sc.exe Create binary path must be quoted when it contains spaces.
$binPath = "`"$exe`" $($args -join ' ')"

$serviceName = 'CPNInstaller'
$existing = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "Stopping existing service $serviceName..."
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

if (-not $SkipFirewall) {
    $ruleName = "CPN Installer TCP $Port"
    Get-NetFirewallRule -DisplayName $ruleName -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction SilentlyContinue
    New-NetFirewallRule -DisplayName $ruleName -Direction Inbound -Action Allow -Protocol TCP -LocalPort $Port | Out-Null
    Write-Host "Firewall rule added: $ruleName"
}

Start-Service -Name $serviceName
Write-Host ""
Write-Host "CPN Phase A installed."
Write-Host "  Binaries: $installRoot"
Write-Host "  Data:     $dataRoot"
Write-Host "  Service:  $serviceName (port $Port)"
if ($AllowRemote) {
    Write-Host "  Bind:     0.0.0.0 (AllowRemote)"
} else {
    Write-Host "  Bind:     127.0.0.1 (use -AllowRemote for LAN access)"
}
Write-Host ""
Write-Host "Open the installer UI, complete account bootstrap, then see to-do/WINDOWS-SERVER-INSTALL.md"
Write-Host "for IIS notes. Linux web/mail recipes are not available on Windows."
