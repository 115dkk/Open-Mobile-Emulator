# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Tests Start-Guest dry-run behavior without QEMU.
.DESCRIPTION
Builds a temporary OME_HOME, fake QEMU directory, and firmware directory, then
checks that dry-run succeeds and keeps path values containing spaces in one
printed argument element.
.EXAMPLE
Invoke-Pester -Path ./tests/launcher/Start-Guest.Tests.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

BeforeAll {
    $script:RepoRoot = [IO.Directory]::GetParent([IO.Directory]::GetParent($PSScriptRoot).FullName).FullName
    $script:StartGuestPath = [IO.Path]::Combine($script:RepoRoot, 'launcher', 'Start-Guest.ps1')
}

Describe 'Start-Guest.ps1 -DryRun' {
    BeforeEach {
        $script:OldOmeHome = $env:OME_HOME
        $script:OldQemuDirectory = $env:OME_QEMU_DIR
        $script:OldFirmwareDirectory = $env:OME_FIRMWARE_DIR

        $script:OmeDataHome = [IO.Path]::Combine($TestDrive, 'OME Home')
        $script:QemuDirectory = [IO.Path]::Combine($TestDrive, 'QEMU Fake')
        $script:FirmwareDirectory = [IO.Path]::Combine($TestDrive, 'Firmware Files')
        $vmDirectory = [IO.Path]::Combine($script:OmeDataHome, 'vm', 'default')
        [void][IO.Directory]::CreateDirectory($vmDirectory)
        [void][IO.Directory]::CreateDirectory($script:QemuDirectory)
        [void][IO.Directory]::CreateDirectory($script:FirmwareDirectory)

        [IO.File]::WriteAllText([IO.Path]::Combine($vmDirectory, 'disk.qcow2'), 'dummy disk')
        [IO.File]::WriteAllText([IO.Path]::Combine($script:QemuDirectory, 'qemu-system-x86_64.exe'), 'dummy executable')
        [IO.File]::WriteAllText([IO.Path]::Combine($script:QemuDirectory, 'qemu-img.exe'), 'dummy executable')
        [IO.File]::WriteAllText([IO.Path]::Combine($script:FirmwareDirectory, 'edk2-x86_64-code.fd'), 'dummy code')
        [IO.File]::WriteAllText([IO.Path]::Combine($script:FirmwareDirectory, 'edk2-i386-vars.fd'), 'dummy vars')

        $env:OME_HOME = $script:OmeDataHome
        $env:OME_QEMU_DIR = $script:QemuDirectory
        $env:OME_FIRMWARE_DIR = $script:FirmwareDirectory
    }

    AfterEach {
        $env:OME_HOME = $script:OldOmeHome
        $env:OME_QEMU_DIR = $script:OldQemuDirectory
        $env:OME_FIRMWARE_DIR = $script:OldFirmwareDirectory
    }

    It 'succeeds with fake local files and prints one element per line' {
        $output = @(& pwsh -NoProfile -File $script:StartGuestPath -Accel tcg -Audio none -DryRun 2>&1)
        $exitCode = $LASTEXITCODE
        $text = ($output | ForEach-Object { [string]$_ }) -join "`n"

        $exitCode | Should -Be 0 -Because $text
        $text | Should -Match '(?m)^\[13\] if=pflash,format=raw,readonly=on,file=.*Firmware Files.*edk2-x86_64-code\.fd$'
        $text | Should -Match '(?m)^\[17\] file=.*OME Home.*disk\.qcow2,if=virtio,format=qcow2$'
        $text | Should -Not -Match '(?m)^\[\d+\] intel-hda$'
    }
}
