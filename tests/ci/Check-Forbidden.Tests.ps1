# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Tests the R1 forbidden-file checker in each selection mode.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

BeforeAll {
    $script:RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $script:CheckScript = Join-Path $script:RepositoryRoot 'ci/Check-Forbidden.ps1'
    $script:PowerShellPath = (Get-Process -Id $PID).Path

    function New-ForbiddenTestRepository {
        $root = Join-Path $env:TEMP ("ome-forbidden-{0}" -f [guid]::NewGuid().ToString('N'))
        [void][System.IO.Directory]::CreateDirectory((Join-Path $root 'ci'))
        [void][System.IO.Directory]::CreateDirectory((Join-Path $root 'img'))
        [void][System.IO.Directory]::CreateDirectory((Join-Path $root 'tests/fixtures'))
        Copy-Item -LiteralPath (Join-Path $script:RepositoryRoot 'ci/forbidden-patterns.txt') `
            -Destination (Join-Path $root 'ci/forbidden-patterns.txt')
        Set-Content -LiteralPath (Join-Path $root 'ci/allowlist.txt') `
            -Value 'tests/fixtures/allowed.apk # GPL-2.0-or-later, self-made test fixture' `
            -Encoding utf8NoBOM
        Set-Content -LiteralPath (Join-Path $root 'x.apk') -Value 'apk' -Encoding utf8NoBOM
        Set-Content -LiteralPath (Join-Path $root 'libndk_translation.so') -Value 'blob' -Encoding utf8NoBOM
        Set-Content -LiteralPath (Join-Path $root 'ok.txt') -Value 'ok' -Encoding utf8NoBOM
        Set-Content -LiteralPath (Join-Path $root 'img/readme.md') -Value 'docs' -Encoding utf8NoBOM
        Set-Content -LiteralPath (Join-Path $root '한글 파일.txt') -Value 'unicode' -Encoding utf8NoBOM
        Set-Content -LiteralPath (Join-Path $root 'tests/fixtures/allowed.apk') `
            -Value 'fixture' -Encoding utf8NoBOM

        & git -C $root init --quiet
        if ($LASTEXITCODE -ne 0) {
            throw 'git init failed.'
        }
        return $root
    }

    function Invoke-ForbiddenCheck {
        param(
            [Parameter(Mandatory)][string]$Root,
            [ValidateSet('Default', 'Staged', 'Path')]
            [string]$Mode = 'Default',
            [string[]]$Paths = @()
        )

        $wrapperPath = Join-Path $Root 'invoke-check.ps1'
        Set-Content -LiteralPath $wrapperPath -Encoding utf8NoBOM -Value @'
param(
    [string]$CheckScript,
    [string]$RepoRoot,
    [string]$Mode,
    [string]$PathsJson
)
if ($Mode -eq 'Staged') {
    & $CheckScript -RepoRoot $RepoRoot -Staged
}
elseif ($Mode -eq 'Path') {
    $items = @(ConvertFrom-Json -InputObject $PathsJson)
    & $CheckScript -RepoRoot $RepoRoot -Path $items
}
else {
    & $CheckScript -RepoRoot $RepoRoot
}
exit $LASTEXITCODE
'@
        $pathsJson = ConvertTo-Json -InputObject @($Paths) -Compress
        $output = @(
            & $script:PowerShellPath -NoProfile -File $wrapperPath `
                -CheckScript $script:CheckScript `
                -RepoRoot $Root `
                -Mode $Mode `
                -PathsJson $pathsJson 2>&1
        ) | ForEach-Object { $_.ToString() }
        return [pscustomobject]@{
            ExitCode = $LASTEXITCODE
            Output = $output
            Text = $output -join "`n"
        }
    }
}

Describe 'Check-Forbidden.ps1' {
    BeforeEach {
        $script:TempRepository = New-ForbiddenTestRepository
    }

    AfterEach {
        if (Test-Path -LiteralPath $script:TempRepository) {
            Remove-Item -LiteralPath $script:TempRepository -Recurse -Force
        }
    }

    It 'checks tracked and untracked files by default' {
        & git -C $script:TempRepository add -f -- 'ok.txt' 'img/readme.md' '한글 파일.txt'
        & git -C $script:TempRepository add -f -- 'tests/fixtures/allowed.apk'
        $LASTEXITCODE | Should -Be 0

        $result = Invoke-ForbiddenCheck -Root $script:TempRepository

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'FORBIDDEN \*\.apk x\.apk'
        $result.Text | Should -Match 'FORBIDDEN libndk_translation\* libndk_translation\.so'
        $result.Text | Should -Not -Match 'allowed\.apk'
        $result.Text | Should -Not -Match 'img/readme\.md'
    }

    It 'checks only staged ACMR paths with -Staged' {
        & git -C $script:TempRepository add -f -- `
            'x.apk' 'libndk_translation.so' 'ok.txt' 'img/readme.md' `
            '한글 파일.txt' 'tests/fixtures/allowed.apk'
        $LASTEXITCODE | Should -Be 0

        $result = Invoke-ForbiddenCheck -Root $script:TempRepository -Mode Staged

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'FORBIDDEN \*\.apk x\.apk'
        $result.Text | Should -Match 'FORBIDDEN libndk_translation\* libndk_translation\.so'
        $result.Text | Should -Not -Match 'allowed\.apk'
        $result.Text | Should -Not -Match 'img/readme\.md'
    }

    It 'checks only explicit paths with -Path' {
        $result = Invoke-ForbiddenCheck -Root $script:TempRepository -Mode Path -Paths @(
            'x.apk',
            'libndk_translation.so',
            'ok.txt',
            'img/readme.md',
            '한글 파일.txt',
            'tests/fixtures/allowed.apk'
        )

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'FORBIDDEN \*\.apk x\.apk'
        $result.Text | Should -Match 'FORBIDDEN libndk_translation\* libndk_translation\.so'
        $result.Text | Should -Not -Match 'allowed\.apk'
        $result.Text | Should -Not -Match 'img/readme\.md'
    }
}
