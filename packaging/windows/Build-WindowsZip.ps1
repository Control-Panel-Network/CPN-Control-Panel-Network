#Requires -Version 5.1
<#
.SYNOPSIS
  Build cpn-installer.exe and cpn.exe for x86_64-pc-windows-msvc and stage a zip.
#>
[CmdletBinding()]
param(
    [string]$OutDir = (Join-Path $PSScriptRoot '..\..\dist\windows'),
    [switch]$SkipUi
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..\..')
Set-Location $repoRoot

# openssl-sys (via webauthn-rs) needs OPENSSL_DIR / OPENSSL_LIB_DIR on MSVC.
# Reuse the developer discovery script when the caller did not set them.
if (-not $env:OPENSSL_LIB_DIR) {
    . (Join-Path $repoRoot 'scripts\windows-dev-env.ps1')
    if (-not $env:OPENSSL_LIB_DIR) {
        throw 'OpenSSL SDK not found. Install OpenSSL (see scripts/windows-dev-env.ps1) or set OPENSSL_DIR and OPENSSL_LIB_DIR.'
    }
}

if (-not $SkipUi) {
    Push-Location (Join-Path $repoRoot 'installer-ui')
    try {
        if (-not (Test-Path 'node_modules')) {
            npm ci
        }
        npm run build
    } finally {
        Pop-Location
    }
}

$target = 'x86_64-pc-windows-msvc'
rustup target add $target 2>$null | Out-Null
cargo build --release --locked --target $target

$stage = Join-Path $OutDir 'stage'
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item (Join-Path $repoRoot "target\$target\release\cpn-installer.exe") (Join-Path $stage 'cpn-installer.exe') -Force
Copy-Item (Join-Path $repoRoot "target\$target\release\cpn.exe") (Join-Path $stage 'cpn.exe') -Force
Copy-Item (Join-Path $repoRoot 'packaging\windows\Install-Cpn.ps1') (Join-Path $stage 'Install-Cpn.ps1') -Force
Copy-Item (Join-Path $repoRoot 'packaging\windows\Uninstall-Cpn.ps1') (Join-Path $stage 'Uninstall-Cpn.ps1') -Force

$zip = Join-Path $OutDir 'cpn-windows-x86_64.zip'
if (Test-Path $zip) {
    Remove-Item $zip -Force
}
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
Write-Host "Staged: $stage"
Write-Host "Zip:    $zip"
