# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Enables the repository-managed Git hooks.

.DESCRIPTION
Sets the local core.hooksPath value to .githooks and verifies the stored value.
Running the script repeatedly leaves the same configuration in place.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

try {
    $repoRoot = Split-Path -Parent $PSScriptRoot
    & git -C $repoRoot rev-parse --is-inside-work-tree *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "Not a Git work tree: $repoRoot"
    }

    & git -C $repoRoot config core.hooksPath .githooks
    if ($LASTEXITCODE -ne 0) {
        throw 'git config failed.'
    }

    $configuredPath = (& git -C $repoRoot config --get core.hooksPath).Trim()
    if ($LASTEXITCODE -ne 0 -or $configuredPath -cne '.githooks') {
        throw "core.hooksPath verification failed; current value is '$configuredPath'."
    }

    Write-Output 'Git hooks enabled: core.hooksPath=.githooks'
    exit 0
}
catch {
    [System.Console]::Error.WriteLine("ERROR Install-Hooks: $($_.Exception.Message)")
    exit 2
}
