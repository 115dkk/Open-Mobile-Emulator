# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Parses every PowerShell script and checks every driver module for syntax errors.

.DESCRIPTION
Runs the PowerShell parser over all .ps1 and .psm1 files under ci, launcher,
translator and qemu-build, and `node --check` over ci/dod/*.mjs when node is on
PATH. A script that does not parse never reaches the runner: two completion runs
(docs/evidence/M2/dod-ci.md, 33회 and 34회) were lost to errors this check finds
in a second.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$failed = 0
$scripts = @(Get-ChildItem -LiteralPath (@('ci', 'launcher', 'translator', 'qemu-build') |
    ForEach-Object { Join-Path $repoRoot $_ } | Where-Object { Test-Path -LiteralPath $_ }) -Recurse -File -Include '*.ps1', '*.psm1')
foreach ($script in $scripts) {
    $tokens = $null
    $errors = $null
    [void][System.Management.Automation.Language.Parser]::ParseFile($script.FullName, [ref]$tokens, [ref]$errors)
    foreach ($parseError in $errors) {
        $failed++
        Write-Output ("FAIL {0}:{1}: {2}" -f $script.FullName, $parseError.Extent.StartLineNumber, $parseError.Message)
    }
}
Write-Output ("Parsed {0} PowerShell scripts." -f $scripts.Count)

$node = Get-Command node -ErrorAction SilentlyContinue
$modules = @(Get-ChildItem -LiteralPath (Join-Path $repoRoot 'ci/dod') -File -Filter '*.mjs')
if ($null -eq $node) {
    Write-Warning ("node is not on PATH; {0} driver modules were not checked." -f $modules.Count)
}
else {
    foreach ($module in $modules) {
        $output = & $node.Source --check $module.FullName 2>&1
        if ($LASTEXITCODE -ne 0) {
            $failed++
            Write-Output ("FAIL {0}: {1}" -f $module.FullName, ($output -join ' '))
        }
    }
    Write-Output ("Checked {0} driver modules with node." -f $modules.Count)
}

if ($failed -gt 0) {
    Write-Output ("RESULT Check-ScriptSyntax FAIL({0})" -f $failed)
    exit 1
}
Write-Output 'RESULT Check-ScriptSyntax PASS'
exit 0
