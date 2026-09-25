# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Enables the Windows Hypervisor Platform after explicit consent.
.DESCRIPTION
Shows the effect and exits unless Consent is supplied. With consent, the script
relaunches itself as administrator when needed, records before and after state,
and enables only HypervisorPlatform without rebooting. If the PowerShell DISM
cmdlet is unavailable or fails, dism.exe is used for the same operation.
.PARAMETER Consent
Confirms that the user accepts the feature change, reboot requirement, and
possible kernel anti-cheat incompatibility.
.PARAMETER Elevated
Internal marker used by the elevated child process.
.EXAMPLE
./Enable-Whpx.ps1
.EXAMPLE
./Enable-Whpx.ps1 -Consent
#>
[CmdletBinding()]
param(
    [switch]$Consent,
    [switch]$Elevated
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force

function Get-FeatureState {
    [CmdletBinding()]
    param()

    try {
        $feature = Get-CimInstance -ClassName Win32_OptionalFeature -Filter "Name='HypervisorPlatform'" -ErrorAction Stop
        if ($null -eq $feature) {
            return 'Not found'
        }
        return $(switch ([int]$feature.InstallState) {
            1 { 'Enabled' }
            2 { 'Disabled' }
            3 { 'Absent' }
            default { "Unknown ($($feature.InstallState))" }
        })
    }
    catch {
        return "Unknown: $($_.Exception.Message)"
    }
}

function Test-IsAdministrator {
    [CmdletBinding()]
    param()

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

$planLines = @(
    'This script would enable the Windows optional feature HypervisorPlatform.',
    'Administrator permission is required.',
    'A reboot is required before WHPX can be used.',
    'Side effect: kernel anti-cheat games may refuse to run while Hyper-V/WHPX is on.',
    'The script never reboots the computer.'
)
if (-not $Consent) {
    $planLines | ForEach-Object { Write-Host $_ }
    exit 2
}

$logPath = [IO.Path]::Combine((Get-OmePath -Kind logs), 'whpx-enable.log')
if (-not (Test-IsAdministrator)) {
    if ($Elevated) {
        throw 'Elevation was requested, but the process is not running as administrator.'
    }
    $argumentList = @(
        '-NoProfile',
        '-File',
        ('"' + $PSCommandPath.Replace('"', '\"') + '"'),
        '-Consent',
        '-Elevated'
    )
    $child = Start-Process -FilePath 'pwsh.exe' -Verb RunAs -ArgumentList $argumentList -Wait -PassThru
    if ([IO.File]::Exists($logPath)) {
        Get-Content -LiteralPath $logPath | ForEach-Object { Write-Host $_ }
    }
    exit $child.ExitCode
}

$timestamp = [DateTimeOffset]::Now.ToString('o')
$before = Get-FeatureState
Add-Content -LiteralPath $logPath -Value "[$timestamp] CONSENT: User requested enabling HypervisorPlatform without automatic reboot." -Encoding utf8
Add-Content -LiteralPath $logPath -Value "[$timestamp] Before: $before" -Encoding utf8

if ($before -ceq 'Enabled') {
    Add-Content -LiteralPath $logPath -Value "[$timestamp] Action: skipped; feature already enabled." -Encoding utf8
    Add-Content -LiteralPath $logPath -Value "[$timestamp] After: Enabled" -Encoding utf8
    Write-Host 'HypervisorPlatform is already enabled.'
    Write-Host "Log: $logPath"
    exit 0
}

$restartNeeded = $false
$method = 'Enable-WindowsOptionalFeature'
try {
    $result = Enable-WindowsOptionalFeature -Online -FeatureName 'HypervisorPlatform' -NoRestart -ErrorAction Stop
    $restartNeeded = [bool]$result.RestartNeeded
    Add-Content -LiteralPath $logPath -Value "[$timestamp] $method result: RestartNeeded=$restartNeeded; State=$($result.State)" -Encoding utf8
}
catch {
    Add-Content -LiteralPath $logPath -Value "[$timestamp] $method failed: $($_.Exception.Message)" -Encoding utf8
    $method = 'dism.exe'
    $dismPath = [IO.Path]::Combine($env:SystemRoot, 'System32', 'dism.exe')
    $dismOutput = @(& $dismPath '/Online' '/Enable-Feature' '/FeatureName:HypervisorPlatform' '/NoRestart' 2>&1)
    $dismExitCode = $LASTEXITCODE
    Add-Content -LiteralPath $logPath -Value "[$timestamp] $method exit code: $dismExitCode" -Encoding utf8
    Add-Content -LiteralPath $logPath -Value ($dismOutput | ForEach-Object { [string]$_ }) -Encoding utf8
    if ($dismExitCode -notin @(0, 3010)) {
        throw "dism.exe failed with exit code $dismExitCode. See $logPath"
    }
    $restartNeeded = $dismExitCode -eq 3010
}

$after = Get-FeatureState
Add-Content -LiteralPath $logPath -Value "[$([DateTimeOffset]::Now.ToString('o'))] After: $after; method=$method" -Encoding utf8
Write-Host "HypervisorPlatform state before: $before"
Write-Host "HypervisorPlatform state after: $after"
Write-Host "Enable method: $method"
if ($restartNeeded -or $before -cne 'Enabled') {
    Write-Host 'Reboot required'
}
Write-Host 'The script did not reboot the computer.'
Write-Host "Log: $logPath"
