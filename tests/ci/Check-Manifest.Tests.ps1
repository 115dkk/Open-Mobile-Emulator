# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Tests valid and invalid R2 artifact manifests.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

BeforeAll {
    $script:RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $script:CheckScript = Join-Path $script:RepositoryRoot 'ci/Check-Manifest.ps1'
    $script:RealManifest = Join-Path $script:RepositoryRoot 'manifests/artifacts.json'
    $script:PowerShellPath = (Get-Process -Id $PID).Path

    function Invoke-ManifestCheck {
        param([Parameter(Mandatory)][string]$ManifestPath)

        $output = @(
            & $script:PowerShellPath -NoProfile -File $script:CheckScript `
                -Path $ManifestPath 2>&1
        ) | ForEach-Object { $_.ToString() }
        return [pscustomobject]@{
            ExitCode = $LASTEXITCODE
            Output = $output
            Text = $output -join "`n"
        }
    }

    function Write-ManifestVariant {
        param(
            [Parameter(Mandatory)][scriptblock]$Change
        )

        $path = Join-Path $script:TempDirectory ("manifest-{0}.json" -f [guid]::NewGuid().ToString('N'))
        $manifest = Get-Content -LiteralPath $script:RealManifest -Raw | ConvertFrom-Json
        & $Change $manifest
        $manifest | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
        return $path
    }
}

Describe 'Check-Manifest.ps1' {
    BeforeEach {
        $script:TempDirectory = Join-Path $env:TEMP ("ome-manifest-{0}" -f [guid]::NewGuid().ToString('N'))
        [void][System.IO.Directory]::CreateDirectory($script:TempDirectory)
    }

    AfterEach {
        if (Test-Path -LiteralPath $script:TempDirectory) {
            Remove-Item -LiteralPath $script:TempDirectory -Recurse -Force
        }
    }

    It 'accepts a copy of the real manifest' {
        $copyPath = Join-Path $script:TempDirectory 'artifacts.json'
        Copy-Item -LiteralPath $script:RealManifest -Destination $copyPath

        $result = Invoke-ManifestCheck $copyPath

        $result.ExitCode | Should -Be 0
        $result.Text | Should -Match 'SUMMARY manifest findings=0'
    }

    It 'rejects an empty sha256' {
        $path = Write-ManifestVariant { param($manifest) $manifest.artifacts[0].sha256 = '' }
        $result = Invoke-ManifestCheck $path

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'MANIFEST .* sha256 must be a non-empty string'
    }

    It 'rejects a 63-character sha256' {
        $path = Write-ManifestVariant { param($manifest) $manifest.artifacts[0].sha256 = 'a' * 63 }
        $result = Invoke-ManifestCheck $path

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'MANIFEST .* sha256 must be exactly 64 lowercase hexadecimal characters'
    }

    It 'rejects an HTTP URL' {
        $path = Write-ManifestVariant { param($manifest) $manifest.artifacts[0].url = 'http://sourceforge.net/file.iso' }
        $result = Invoke-ManifestCheck $path

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'MANIFEST .* url must be an absolute HTTPS URL'
    }

    It 'rejects a URL on an unknown host' {
        $path = Write-ManifestVariant { param($manifest) $manifest.artifacts[0].url = 'https://example.invalid/file.iso' }
        $result = Invoke-ManifestCheck $path

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'MANIFEST .* url host is not allowed: example\.invalid'
    }

    It 'rejects an unknown fetched_by value' {
        $path = Write-ManifestVariant { param($manifest) $manifest.artifacts[0].fetched_by = 'server' }
        $result = Invoke-ManifestCheck $path

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'MANIFEST .* fetched_by must be installer or builder'
    }

    It 'rejects duplicate artifact names' {
        $path = Write-ManifestVariant {
            param($manifest)
            $duplicate = $manifest.artifacts[0].PSObject.Copy()
            $manifest.artifacts = @($manifest.artifacts[0], $duplicate)
        }
        $result = Invoke-ManifestCheck $path

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'MANIFEST .* name must be unique'
    }
}
