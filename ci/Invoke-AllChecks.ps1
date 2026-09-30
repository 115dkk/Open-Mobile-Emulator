# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Runs all repository policy checks and Pester tests.

.DESCRIPTION
Runs the R1 forbidden-file check, R2 manifest check, R5 SPDX check, and all
Pester tests. Missing Pester 5 is reported as a warning and does not fail the
other policy checks.

.PARAMETER Staged
Passes staged-file selection to the R1 and R5 checks.
#>
[CmdletBinding()]
param(
    [switch]$Staged
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-PolicyCheck {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$ScriptPath,
        [string[]]$ArgumentList = @()
    )

    $checkOutput = @(
        & $script:PowerShellPath -NoProfile -File $ScriptPath @ArgumentList 2>&1
    )
    $exitCode = $LASTEXITCODE
    foreach ($line in $checkOutput) {
        Write-Host $line
    }
    $status = if ($exitCode -eq 0) { 'PASS' } else { "FAIL($exitCode)" }
    Write-Host "RESULT $Name $status"
    return $exitCode
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$script:PowerShellPath = (Get-Process -Id $PID).Path
$failed = $false
$selectionArguments = if ($Staged) { @('-Staged') } else { @() }

$forbiddenResult = Invoke-PolicyCheck `
    -Name 'Check-Forbidden' `
    -ScriptPath (Join-Path $PSScriptRoot 'Check-Forbidden.ps1') `
    -ArgumentList $selectionArguments
if ($forbiddenResult -ne 0) {
    $failed = $true
}

$manifestResult = Invoke-PolicyCheck `
    -Name 'Check-Manifest' `
    -ScriptPath (Join-Path $PSScriptRoot 'Check-Manifest.ps1')
if ($manifestResult -ne 0) {
    $failed = $true
}

$spdxResult = Invoke-PolicyCheck `
    -Name 'Check-SpdxHeaders' `
    -ScriptPath (Join-Path $PSScriptRoot 'Check-SpdxHeaders.ps1') `
    -ArgumentList $selectionArguments
if ($spdxResult -ne 0) {
    $failed = $true
}

$unsafeScopeResult = Invoke-PolicyCheck `
    -Name 'Check-UnsafeScope' `
    -ScriptPath (Join-Path $PSScriptRoot 'Check-UnsafeScope.ps1')
if ($unsafeScopeResult -ne 0) {
    $failed = $true
}

$syntaxResult = Invoke-PolicyCheck `
    -Name 'Check-ScriptSyntax' `
    -ScriptPath (Join-Path $PSScriptRoot 'Check-ScriptSyntax.ps1')
if ($syntaxResult -ne 0) {
    $failed = $true
}

$pesterModule = Get-Module -ListAvailable Pester |
    Where-Object { $_.Version.Major -eq 5 } |
    Sort-Object Version -Descending |
    Select-Object -First 1
if ($null -eq $pesterModule) {
    Write-Warning 'Pester 5 is not installed; Pester tests were skipped.'
    Write-Output 'RESULT Pester SKIP'
}
else {
    $pesterWorkingDirectory = Join-Path `
        ([System.IO.Path]::GetTempPath()) `
        ("ome-pester-{0}" -f [guid]::NewGuid().ToString('N'))
    try {
        Import-Module $pesterModule.Path -Force
        [void][System.IO.Directory]::CreateDirectory($pesterWorkingDirectory)
        Push-Location -LiteralPath $pesterWorkingDirectory
        try {
            $pesterResult = Invoke-Pester -Path (Join-Path $repoRoot 'tests') -CI -PassThru
        }
        finally {
            Pop-Location
        }
        if ($pesterResult.FailedCount -gt 0) {
            $failed = $true
            Write-Output "RESULT Pester FAIL($($pesterResult.FailedCount))"
        }
        else {
            Write-Output 'RESULT Pester PASS'
        }
    }
    catch {
        [System.Console]::Error.WriteLine("ERROR Invoke-AllChecks Pester: $($_.Exception.Message)")
        $failed = $true
        Write-Output 'RESULT Pester FAIL(2)'
    }
    finally {
        if ([System.IO.Directory]::Exists($pesterWorkingDirectory)) {
            Remove-Item -LiteralPath $pesterWorkingDirectory -Recurse -Force
        }
    }
}

if ($failed) {
    exit 1
}
exit 0
