#Requires -Version 5.1
<#
.SYNOPSIS
  Remove the CPN Windows service and installed binaries (keeps ProgramData by default).

.PARAMETER RemoveData
  Also delete C:\ProgramData\CPN (accounts, bootstrap, SMTP settings).
#>
[CmdletBinding()]
param(
    [switch]$RemoveData
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-IsAdministrator)) {
    throw 'Uninstall-Cpn.ps1 must run elevated (Run as administrator).'
}

$serviceName = 'CPNInstaller'
$existing = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
if ($existing) {
    if ($existing.Status -eq 'Running') {
        Stop-Service -Name $serviceName -Force -ErrorAction SilentlyContinue
    }
    sc.exe delete $serviceName | Out-Null
    Write-Host "Removed service $serviceName"
}

Get-NetFirewallRule -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like 'CPN Installer TCP *' } |
    ForEach-Object {
        Remove-NetFirewallRule -Name $_.Name -ErrorAction SilentlyContinue
        Write-Host "Removed firewall rule $($_.DisplayName)"
    }

$installRoot = Join-Path $env:ProgramFiles 'CPN'
if (Test-Path -LiteralPath $installRoot) {
    Remove-Item -LiteralPath $installRoot -Recurse -Force
    Write-Host "Removed $installRoot"
}

if ($RemoveData) {
    $dataRoot = Join-Path $env:ProgramData 'CPN'
    if (Test-Path -LiteralPath $dataRoot) {
        Remove-Item -LiteralPath $dataRoot -Recurse -Force
        Write-Host "Removed $dataRoot"
    }
} else {
    Write-Host "Kept data under $env:ProgramData\CPN (pass -RemoveData to delete)."
}

Write-Host 'Uninstall complete.'
