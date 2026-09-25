# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Tests GPL and allowed Apache SPDX-header handling.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

BeforeAll {
    $script:RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $script:SourceCheckScript = Join-Path $script:RepositoryRoot 'ci/Check-SpdxHeaders.ps1'
    $script:PowerShellPath = (Get-Process -Id $PID).Path

    function Invoke-SpdxCheck {
        param(
            [Parameter(Mandatory)][string]$Root,
            [Parameter(Mandatory)][string[]]$Paths
        )

        $wrapperPath = Join-Path $Root 'invoke-check.ps1'
        Set-Content -LiteralPath $wrapperPath -Encoding utf8NoBOM -Value @'
param(
    [string]$CheckScript,
    [string]$RepositoryRoot,
    [string]$PathsJson
)
Set-Location -LiteralPath $RepositoryRoot
$items = @(ConvertFrom-Json -InputObject $PathsJson)
& $CheckScript -Path $items
exit $LASTEXITCODE
'@
        $pathsJson = ConvertTo-Json -InputObject @($Paths) -Compress
        $output = @(
            & $script:PowerShellPath -NoProfile -File $wrapperPath `
                -CheckScript $script:CheckScript `
                -RepositoryRoot $Root `
                -PathsJson $pathsJson 2>&1
        ) | ForEach-Object { $_.ToString() }
        return [pscustomobject]@{
            ExitCode = $LASTEXITCODE
            Output = $output
            Text = $output -join "`n"
        }
    }
}

Describe 'Check-SpdxHeaders.ps1' {
    BeforeEach {
        $script:TempRepository = Join-Path $env:TEMP ("ome-spdx-{0}" -f [guid]::NewGuid().ToString('N'))
        [void][System.IO.Directory]::CreateDirectory((Join-Path $script:TempRepository 'ci'))
        [void][System.IO.Directory]::CreateDirectory((Join-Path $script:TempRepository 'guest/overlay'))
        [void][System.IO.Directory]::CreateDirectory((Join-Path $script:TempRepository 'src'))
        $script:CheckScript = Join-Path $script:TempRepository 'ci/Check-SpdxHeaders.ps1'
        Copy-Item -LiteralPath $script:SourceCheckScript -Destination $script:CheckScript
        & git -C $script:TempRepository init --quiet
        if ($LASTEXITCODE -ne 0) {
            throw 'git init failed.'
        }

        Set-Content -LiteralPath (Join-Path $script:TempRepository 'src/correct.ps1') -Encoding utf8NoBOM -Value @(
            '# SPDX-License-Identifier: GPL-2.0-or-later',
            '# Copyright (C) 2026 Open Mobile Emulator contributors',
            '#Requires -Version 7.0'
        )
        Set-Content -LiteralPath (Join-Path $script:TempRepository 'src/missing.py') -Encoding utf8NoBOM -Value @(
            '# no license here',
            'print("missing")'
        )
        Set-Content -LiteralPath (Join-Path $script:TempRepository 'src/apache.py') -Encoding utf8NoBOM -Value @(
            '# SPDX-License-Identifier: Apache-2.0',
            'print("wrong directory")'
        )
        Set-Content -LiteralPath (Join-Path $script:TempRepository 'guest/overlay/apache.py') -Encoding utf8NoBOM -Value @(
            '# SPDX-License-Identifier: Apache-2.0',
            'print("allowed")'
        )
    }

    AfterEach {
        if (Test-Path -LiteralPath $script:TempRepository) {
            Remove-Item -LiteralPath $script:TempRepository -Recurse -Force
        }
    }

    It 'accepts GPL and allowed Apache headers' {
        $result = Invoke-SpdxCheck $script:TempRepository @(
            'src/correct.ps1',
            'guest/overlay/apache.py'
        )

        $result.ExitCode | Should -Be 0
        $result.Text | Should -Match 'SUMMARY spdx findings=0 checked=2'
    }

    It 'reports missing and disallowed Apache headers' {
        $result = Invoke-SpdxCheck $script:TempRepository @(
            'src/correct.ps1',
            'src/missing.py',
            'src/apache.py',
            'guest/overlay/apache.py'
        )

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'SPDX src/missing\.py missing'
        $result.Text | Should -Match 'SPDX src/apache\.py wrong-license'
        $result.Text | Should -Not -Match 'SPDX guest/overlay/apache\.py'
        $result.Text | Should -Not -Match 'SPDX src/correct\.ps1'
    }

    It 'reads file content from the index with -Staged' {
        $stagedPath = Join-Path $script:TempRepository 'src/staged.ps1'
        Set-Content -LiteralPath $stagedPath -Encoding utf8NoBOM -Value '# missing in index'
        & git -C $script:TempRepository add -- 'src/staged.ps1'
        $LASTEXITCODE | Should -Be 0
        Set-Content -LiteralPath $stagedPath -Encoding utf8NoBOM -Value @(
            '# SPDX-License-Identifier: GPL-2.0-or-later',
            '# Copyright (C) 2026 Open Mobile Emulator contributors',
            '#Requires -Version 7.0'
        )

        $output = @(
            & $script:PowerShellPath -NoProfile -File $script:CheckScript -Staged 2>&1
        ) | ForEach-Object { $_.ToString() }
        $exitCode = $LASTEXITCODE
        $text = $output -join "`n"

        $exitCode | Should -Be 1
        $text | Should -Match 'SPDX src/staged\.ps1 missing'
    }
}
