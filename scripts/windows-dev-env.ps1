#Requires -Version 5.1
<#
.SYNOPSIS
  Set the OpenSSL build variables CPN Rust builds need on Windows (developer use).

.DESCRIPTION
  webauthn-rs (passkeys) links OpenSSL through openssl-sys. On Windows MSVC the
  openssl-sys build script needs OPENSSL_DIR and, for the common Shining Light
  "OpenSSL-Win64" layout, OPENSSL_LIB_DIR pointing at lib\VC\x64\MD (the plain
  lib\ folder only holds import stubs, so OPENSSL_DIR alone fails with
  "OpenSSL libdir ... does not contain the required files").

  Dot-source the script so the variables land in the current PowerShell session:

      . .\scripts\windows-dev-env.ps1
      cargo check --locked

  The same discovery order is used by .github/workflows/release.yml for the
  Windows Phase A zip. Windows remains a Phase A target (installer UI, Windows
  service, account bootstrap); Linux is the production panel runtime.

  No OpenSSL SDK installed? Either install one:

      winget install --id ShiningLight.OpenSSL --exact
      # or: choco install openssl -y

  or build OpenSSL from source instead (needs Perl on PATH; several minutes):

      cargo build --features vendored-openssl

.PARAMETER OpenSslDir
  Explicit OpenSSL root. Default: probe OPENSSL_DIR and common install paths.

.PARAMETER Persist
  Also store OPENSSL_DIR / OPENSSL_LIB_DIR at user scope so new terminals inherit them.
#>
[CmdletBinding()]
param(
    [string]$OpenSslDir = '',
    [switch]$Persist
)

Set-StrictMode -Version Latest

$candidates = @()
if ($OpenSslDir) { $candidates += $OpenSslDir }
if ($env:OPENSSL_DIR) { $candidates += $env:OPENSSL_DIR }
$candidates += @(
    'C:\Program Files\OpenSSL-Win64',
    'C:\Program Files\OpenSSL',
    'C:\Program Files (x86)\OpenSSL-Win64',
    'C:\OpenSSL-Win64'
)

$dir = $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $dir) {
    Write-Warning 'OpenSSL install directory not found.'
    Write-Warning 'Install: winget install --id ShiningLight.OpenSSL --exact   (or: choco install openssl -y)'
    Write-Warning 'Or build from source: cargo build --features vendored-openssl   (needs Perl on PATH)'
    return
}

$libCandidates = @(
    (Join-Path $dir 'lib\VC\x64\MD'),
    (Join-Path $dir 'lib\VC\x64\MT'),
    (Join-Path $dir 'lib')
)
$libDir = $libCandidates | Where-Object { Test-Path (Join-Path $_ 'libssl.lib') } | Select-Object -First 1
if (-not $libDir) {
    $hit = Get-ChildItem -Path $dir -Recurse -Filter 'libssl.lib' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($hit) { $libDir = $hit.Directory.FullName }
}
if (-not $libDir) {
    Write-Warning "libssl.lib not found under $dir. The 'Light' OpenSSL installer has no SDK; install the full package."
    return
}

$env:OPENSSL_DIR = $dir
$env:OPENSSL_LIB_DIR = $libDir
if ($Persist) {
    [Environment]::SetEnvironmentVariable('OPENSSL_DIR', $dir, 'User')
    [Environment]::SetEnvironmentVariable('OPENSSL_LIB_DIR', $libDir, 'User')
    Write-Host 'Stored OPENSSL_DIR / OPENSSL_LIB_DIR at user scope (new terminals inherit them).'
}
Write-Host "OPENSSL_DIR=$env:OPENSSL_DIR"
Write-Host "OPENSSL_LIB_DIR=$env:OPENSSL_LIB_DIR"
Write-Host 'Ready: cargo check --locked / cargo test --locked'
