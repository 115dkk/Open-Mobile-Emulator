# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Tests the ADR-0002 unsafe-scope policy gate with temporary repositories.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

BeforeAll {
    $script:RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $script:SourceCheckScript = Join-Path $script:RepositoryRoot 'ci/Check-UnsafeScope.ps1'
    $script:PowerShellPath = (Get-Process -Id $PID).Path

    function Set-FixtureContent {
        param(
            [Parameter(Mandatory)][string]$Path,
            [Parameter(Mandatory)][string]$Value
        )
        [void][System.IO.Directory]::CreateDirectory((Split-Path -Parent $Path))
        Set-Content -LiteralPath $Path -Encoding utf8NoBOM -Value $Value
    }

    function New-UnsafeFixture {
        param([Parameter(Mandatory)][string]$Root)

        [void][System.IO.Directory]::CreateDirectory((Join-Path $Root 'ci'))
        Copy-Item -LiteralPath $script:SourceCheckScript -Destination (Join-Path $Root 'ci/Check-UnsafeScope.ps1')
        Set-FixtureContent (Join-Path $Root 'host/Cargo.toml') @'
# SPDX-License-Identifier: GPL-2.0-or-later
[workspace]
members = ["crates/ordinary", "crates/ome-platform-win"]
[workspace.lints.rust]
unsafe_code = "forbid"
'@
        Set-FixtureContent (Join-Path $Root 'host/crates/ordinary/src/lib.rs') @'
// SPDX-License-Identifier: GPL-2.0-or-later
#![forbid(unsafe_code)]
// The restricted token in a line comment is allowed.
/* The restricted token in a block comment is allowed. */
pub fn value() -> u32 { 1 }
'@
        Set-FixtureContent (Join-Path $Root 'host/crates/ome-platform-win/Cargo.toml') @'
# SPDX-License-Identifier: GPL-2.0-or-later
[package]
name = "ome-platform-win"
version = "0.1.0"
[lints.rust]
unsafe_code = "deny"
'@
        Set-FixtureContent (Join-Path $Root 'host/crates/ome-platform-win/src/lib.rs') @'
// SPDX-License-Identifier: GPL-2.0-or-later
#![deny(unsafe_code)]
#![deny(clippy::undocumented_unsafe_blocks)]
mod ffi;
'@
        Set-FixtureContent (Join-Path $Root 'host/crates/ome-platform-win/src/ffi/mod.rs') @'
// SPDX-License-Identifier: GPL-2.0-or-later
#![allow(unsafe_code)]
pub unsafe fn raw() {}
'@
        & git -C $Root init --quiet
        if ($LASTEXITCODE -ne 0) {
            throw 'git init failed.'
        }
    }

    function Invoke-UnsafeCheck {
        param([Parameter(Mandatory)][string]$Root)

        $output = @(
            & $script:PowerShellPath -NoProfile -File (Join-Path $Root 'ci/Check-UnsafeScope.ps1') `
                -RepositoryRoot $Root 2>&1
        ) | ForEach-Object { $_.ToString() }
        return [pscustomobject]@{
            ExitCode = $LASTEXITCODE
            Text = $output -join "`n"
        }
    }
}

Describe 'Check-UnsafeScope.ps1' {
    BeforeEach {
        $script:TempRepository = Join-Path $env:TEMP ("ome-unsafe-{0}" -f [guid]::NewGuid().ToString('N'))
        New-UnsafeFixture $script:TempRepository
    }

    AfterEach {
        if (Test-Path -LiteralPath $script:TempRepository) {
            Remove-Item -LiteralPath $script:TempRepository -Recurse -Force
        }
    }

    It 'accepts a passing tree with unsafe confined to ffi' {
        $result = Invoke-UnsafeCheck $script:TempRepository

        $result.ExitCode | Should -Be 0
        $result.Text | Should -Match 'SUMMARY unsafe-scope findings=0 checked=3'
    }

    It 'reports unsafe outside the ffi directory, including after a lifetime' {
        Set-FixtureContent (Join-Path $script:TempRepository 'host/crates/ordinary/src/extra.rs') @'
// SPDX-License-Identifier: GPL-2.0-or-later
pub fn borrow<'a>(value: &'a str) -> &'a str { value }
pub unsafe fn escaped() {}
'@

        $result = Invoke-UnsafeCheck $script:TempRepository

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'UNSAFE host/crates/ordinary/src/extra\.rs outside-ffi'
    }

    It 'reports a missing forbid line' {
        Set-FixtureContent (Join-Path $script:TempRepository 'host/crates/ordinary/src/lib.rs') @'
// SPDX-License-Identifier: GPL-2.0-or-later
pub fn value() -> u32 { 1 }
'@

        $result = Invoke-UnsafeCheck $script:TempRepository

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'UNSAFE host/crates/ordinary/src/lib\.rs missing-forbid'
    }

    It 'reports a wrong workspace lint' {
        Set-FixtureContent (Join-Path $script:TempRepository 'host/Cargo.toml') @'
# SPDX-License-Identifier: GPL-2.0-or-later
[workspace]
members = ["crates/ordinary", "crates/ome-platform-win"]
[workspace.lints.rust]
unsafe_code = "warn"
'@

        $result = Invoke-UnsafeCheck $script:TempRepository

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'UNSAFE host/Cargo\.toml workspace-lint'
    }
}
