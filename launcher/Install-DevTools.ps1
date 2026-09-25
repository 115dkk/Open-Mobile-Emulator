# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Installs QEMU and MSYS2 on a development host after explicit consent.
.DESCRIPTION
Checks winget package state and installs only missing development tools. This is
not part of the installed product and makes no change without Consent. WhatIf
shows the winget commands without running them.
.PARAMETER Consent
Authorizes winget to install missing packages.
.PARAMETER WhatIf
Prints commands instead of running winget.
.EXAMPLE
./Install-DevTools.ps1
.EXAMPLE
./Install-DevTools.ps1 -Consent -WhatIf
.EXAMPLE
./Install-DevTools.ps1 -Consent
#>
[CmdletBinding()]
param(
    [switch]$Consent,
    [switch]$WhatIf
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$packages = @(
    [pscustomobject]@{ Id = 'SoftwareFreedomConservancy.QEMU'; Name = 'QEMU' },
    [pscustomobject]@{ Id = 'MSYS2.MSYS2'; Name = 'MSYS2' }
)

Write-Host 'Development-host plan:'
foreach ($package in $packages) {
    Write-Host "- Check winget package $($package.Id); install it only when absent."
}
Write-Host '- Package and source agreements will be accepted for these explicit package IDs.'

if (-not $Consent) {
    Write-Host 'No change was made. Re-run with -Consent to continue.'
    exit 2
}

$winget = Get-Command 'winget.exe' -CommandType Application -ErrorAction Stop | Select-Object -First 1
foreach ($package in $packages) {
    $listOutput = @(& $winget.Source 'list' '--id' $package.Id '--exact' '--accept-source-agreements' 2>&1)
    $listExitCode = $LASTEXITCODE
    $installed = $listExitCode -eq 0 -and (($listOutput | ForEach-Object { [string]$_ }) -join "`n") -match [regex]::Escape($package.Id)
    if ($installed) {
        Write-Host "$($package.Name) is already installed; skipped."
        continue
    }

    $installArguments = @(
        'install',
        '--id', $package.Id,
        '--exact',
        '--accept-package-agreements',
        '--accept-source-agreements'
    )
    if ($WhatIf) {
        Write-Host "WHATIF: winget.exe $($installArguments -join ' ')"
        continue
    }

    & $winget.Source @installArguments
    if ($LASTEXITCODE -ne 0) {
        throw "winget failed to install $($package.Id) with exit code $LASTEXITCODE."
    }
    Write-Host "Installed $($package.Name)."
}
