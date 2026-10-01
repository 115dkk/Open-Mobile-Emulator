# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Tests valid and invalid prepared release directories.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

BeforeAll {
    $script:RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $script:CheckScript = Join-Path $script:RepositoryRoot 'ci/Check-ReleaseAssets.ps1'
    $script:PowerShellPath = (Get-Process -Id $PID).Path
    $script:Version = '0.1.0'
    $script:InstallerName = "Open-Mobile-Emulator-$($script:Version)-x64-setup.exe"

    function Get-TestSha256 {
        param([Parameter(Mandatory)][string]$Path)

        return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    }

    function Write-TestFile {
        param(
            [Parameter(Mandatory)][string]$Directory,
            [Parameter(Mandatory)][string]$Name,
            [Parameter(Mandatory)][string]$Value
        )

        [System.IO.File]::WriteAllText(
            (Join-Path $Directory $Name),
            $Value,
            [System.Text.UTF8Encoding]::new($false)
        )
    }

    function Write-TestSums {
        param(
            [Parameter(Mandatory)][string]$Directory,
            [string[]]$Omit = @()
        )

        $lines = foreach ($file in Get-ChildItem -LiteralPath $Directory -File | Sort-Object Name) {
            if ($file.Name -ceq 'SHA256SUMS' -or $file.Name -cin $Omit) {
                continue
            }
            "$(Get-TestSha256 $file.FullName)  $($file.Name)"
        }
        [System.IO.File]::WriteAllLines(
            (Join-Path $Directory 'SHA256SUMS'),
            $lines,
            [System.Text.UTF8Encoding]::new($false)
        )
    }

    function New-ValidReleaseDirectory {
        $directory = Join-Path $TestDrive ([guid]::NewGuid().ToString('N'))
        [void][System.IO.Directory]::CreateDirectory($directory)
        Write-TestFile $directory $script:InstallerName 'installer'
        $installerHash = Get-TestSha256 (Join-Path $directory $script:InstallerName)
        Write-TestFile $directory "$($script:InstallerName).sha256" "$installerHash  $($script:InstallerName)`n"
        Write-TestFile $directory 'qemu-ome-11.1.0-win64.zip' 'qemu'
        Write-TestFile $directory 'qemu-source-offer-11.1.0.tar.gz' 'source'
        Write-TestFile $directory 'THIRD_PARTY.md' 'third party'
        Write-TestFile $directory 'NOTICE' 'notice'
        Write-TestFile $directory 'LICENSE' 'license'
        Write-TestSums $directory
        return $directory
    }

    function Invoke-ReleaseCheck {
        param([Parameter(Mandatory)][string]$Directory)

        $output = @(
            & $script:PowerShellPath -NoProfile -File $script:CheckScript `
                -Path $Directory -Version $script:Version 2>&1
        ) | ForEach-Object { $_.ToString() }
        return [pscustomobject]@{
            ExitCode = $LASTEXITCODE
            Output = $output
            Text = $output -join "`n"
        }
    }
}

Describe 'Check-ReleaseAssets.ps1' {
    It 'accepts a complete flat release directory' {
        $directory = New-ValidReleaseDirectory

        $result = Invoke-ReleaseCheck $directory

        $result.ExitCode | Should -Be 0
        $result.Text | Should -Match 'SUMMARY release-assets findings=0 checked=8'
    }

    It 'rejects an incorrectly named installer' {
        $directory = New-ValidReleaseDirectory
        Rename-Item -LiteralPath (Join-Path $directory $script:InstallerName) `
            -NewName 'Open-Mobile-Emulator-wrong-x64-setup.exe'
        Write-TestSums $directory

        $result = Invoke-ReleaseCheck $directory

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'FINDING Open-Mobile-Emulator-0\.1\.0-x64-setup\.exe: expected exactly one installer'
        $result.Text | Should -Match 'FINDING Open-Mobile-Emulator-wrong-x64-setup\.exe: installer name must be'
    }

    It 'rejects an installer hash file with a different token' {
        $directory = New-ValidReleaseDirectory
        Write-TestFile $directory "$($script:InstallerName).sha256" "$('0' * 64)  $($script:InstallerName)`n"
        Write-TestSums $directory

        $result = Invoke-ReleaseCheck $directory

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'FINDING .*\.exe\.sha256: does not contain the lowercase installer SHA-256 token'
    }

    It 'rejects a missing source offer' {
        $directory = New-ValidReleaseDirectory
        Remove-Item -LiteralPath (Join-Path $directory 'qemu-source-offer-11.1.0.tar.gz')
        Write-TestSums $directory

        $result = Invoke-ReleaseCheck $directory

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'FINDING qemu-source-offer-\*\.tar\.gz: expected exactly one source bundle, found 0'
    }

    It 'rejects a file omitted from SHA256SUMS' {
        $directory = New-ValidReleaseDirectory
        Write-TestSums $directory -Omit 'NOTICE'

        $result = Invoke-ReleaseCheck $directory

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'FINDING NOTICE: is missing from SHA256SUMS'
    }

    It 'rejects a second executable' {
        $directory = New-ValidReleaseDirectory
        Write-TestFile $directory 'extra.exe' 'extra executable'
        Write-TestSums $directory

        $result = Invoke-ReleaseCheck $directory

        $result.ExitCode | Should -Be 1
        $result.Text | Should -Match 'FINDING \*\.exe: expected only the release installer, found 2'
        $result.Text | Should -Match 'FINDING extra\.exe: file is not allowed in the release directory'
    }
}
