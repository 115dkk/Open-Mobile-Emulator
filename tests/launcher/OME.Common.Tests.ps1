# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Tests the shared Open Mobile Emulator launcher module.
.DESCRIPTION
Checks QEMU argument construction, URL host validation, environment-directed
QEMU discovery, and binary PPM-to-PNG conversion without external tools,
network access, or administrator permission.
.EXAMPLE
Invoke-Pester -Path ./tests/launcher/OME.Common.Tests.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

BeforeAll {
    $script:RepoRoot = [IO.Directory]::GetParent([IO.Directory]::GetParent($PSScriptRoot).FullName).FullName
    $script:ModulePath = [IO.Path]::Combine($script:RepoRoot, 'launcher', 'OME.Common.psm1')
    Import-Module $script:ModulePath -Force
}

Describe 'Get-OmeQemuArguments' {
    BeforeEach {
        $script:BaseArguments = @{
            Name = 'default'
            DiskPath = 'C:\OME Data\vm\default\disk.qcow2'
            FirmwareCode = 'C:\Program Files\qemu\share\edk2-x86_64-code.fd'
            FirmwareVars = 'C:\OME Data\vm\default\efivars.fd'
        }
    }

    It 'builds the standard VGA WHPX configuration as discrete elements' {
        $arguments = Get-OmeQemuArguments @script:BaseArguments -Gpu std -Accel whpx

        $arguments | Should -Contain 'OME default'
        $arguments | Should -Contain 'whpx,kernel-irqchip=off'
        $arguments | Should -Contain 'Skylake-Client-v4'
        $arguments | Should -Not -Contain 'max'
        $arguments | Should -Contain 'reboot=shutdown'
        $arguments | Should -Contain 'VGA'
        $arguments | Should -Contain 'sdl,show-cursor=on'
        $arguments | Should -Contain 'dsound,id=snd0'
        $arguments | Should -Contain 'if=pflash,format=raw,readonly=on,file=C:\Program Files\qemu\share\edk2-x86_64-code.fd'
        $arguments | Should -Contain 'file=C:\OME Data\vm\default\disk.qcow2,if=virtio,format=qcow2'
    }

    It 'builds virgl TCG and installer CD-ROM configuration' {
        $isoPath = 'C:\OME Data\artifacts\guest.iso'
        $arguments = Get-OmeQemuArguments @script:BaseArguments -Gpu virgl -Accel tcg -Cdrom -IsoPath $isoPath

        $arguments | Should -Contain 'tcg,thread=multi'
        $arguments | Should -Contain "file=$isoPath,media=cdrom,if=none,id=cd0"
        $arguments | Should -Contain 'ide-cd,drive=cd0,bootindex=0,bus=ide.0'
        $arguments | Should -Contain 'virtio-vga-gl,edid=off'
        $arguments | Should -Contain 'sdl,show-cursor=on,gl=on'
    }

    It 'omits all audio arguments when audio is none' {
        $arguments = Get-OmeQemuArguments @script:BaseArguments -Audio none

        $arguments | Should -Not -Contain '-audiodev'
        $arguments | Should -Not -Contain 'intel-hda'
        $arguments | Should -Not -Contain 'hda-duplex,audiodev=snd0'
    }

    It 'uses the GTK display element' {
        $arguments = Get-OmeQemuArguments @script:BaseArguments -Display gtk

        $arguments | Should -Contain 'gtk,show-cursor=on'
        $arguments | Should -Not -Contain 'sdl,show-cursor=on'
    }

    It 'appends extra arguments after the RTC value' {
        $arguments = Get-OmeQemuArguments @script:BaseArguments -ExtraArgs @('-nodefaults', '-no-reboot')

        $arguments[-4..-1] | Should -Be @('-rtc', 'base=utc', '-nodefaults', '-no-reboot')
    }
}

Describe 'Test-OmeAllowedUrl' {
    It 'accepts the primary SourceForge download host' {
        Test-OmeAllowedUrl -Url 'https://downloads.sourceforge.net/x' | Should -BeTrue
    }

    It 'accepts a SourceForge mirror subdomain' {
        Test-OmeAllowedUrl -Url 'https://phoenixnap.dl.sourceforge.net/x' | Should -BeTrue
    }

    It 'rejects an unrelated host' {
        Test-OmeAllowedUrl -Url 'https://evil.example.com/x' | Should -BeFalse
    }

    It 'rejects a deceptive suffix' {
        Test-OmeAllowedUrl -Url 'https://sourceforge.net.evil.com/x' | Should -BeFalse
    }
}

Describe 'Find-OmeQemu' {
    BeforeEach {
        $script:OriginalQemuDirectory = $env:OME_QEMU_DIR
        $script:DummyQemuDirectory = [IO.Path]::Combine($TestDrive, 'dummy-qemu')
        [void][IO.Directory]::CreateDirectory($script:DummyQemuDirectory)
        [IO.File]::WriteAllText([IO.Path]::Combine($script:DummyQemuDirectory, 'qemu-system-x86_64.exe'), 'not executable')
        [IO.File]::WriteAllText([IO.Path]::Combine($script:DummyQemuDirectory, 'qemu-img.exe'), 'not executable')
        $env:OME_QEMU_DIR = $script:DummyQemuDirectory
    }

    AfterEach {
        $env:OME_QEMU_DIR = $script:OriginalQemuDirectory
    }

    It 'honours OME_QEMU_DIR without executing a dummy file' {
        $qemu = Find-OmeQemu

        $qemu.Source | Should -Be 'env'
        $qemu.SystemExe | Should -Be ([IO.Path]::Combine($script:DummyQemuDirectory, 'qemu-system-x86_64.exe'))
        $qemu.ImgExe | Should -Be ([IO.Path]::Combine($script:DummyQemuDirectory, 'qemu-img.exe'))
        $qemu.Version | Should -Be 'unknown'
    }
}

Describe 'Convert-OmePpmToPng' {
    It 'round-trips a tiny P6 image with all pixel colours intact' {
        $ppmPath = [IO.Path]::Combine($TestDrive, 'tiny.ppm')
        $pngPath = [IO.Path]::Combine($TestDrive, 'tiny.png')
        $header = [Text.Encoding]::ASCII.GetBytes("P6`n2 2`n255`n")
        $pixels = [byte[]]@(
            255, 0, 0,
            0, 255, 0,
            0, 0, 255,
            255, 255, 255
        )
        $bytes = [byte[]]::new($header.Length + $pixels.Length)
        [Array]::Copy($header, 0, $bytes, 0, $header.Length)
        [Array]::Copy($pixels, 0, $bytes, $header.Length, $pixels.Length)
        [IO.File]::WriteAllBytes($ppmPath, $bytes)

        Convert-OmePpmToPng -PpmPath $ppmPath -PngPath $pngPath

        Add-Type -AssemblyName System.Drawing
        $bitmap = [Drawing.Bitmap]::new($pngPath)
        try {
            $bitmap.Width | Should -Be 2
            $bitmap.Height | Should -Be 2
            $bitmap.GetPixel(0, 0).ToArgb() | Should -Be ([Drawing.Color]::Red.ToArgb())
            $bitmap.GetPixel(1, 0).ToArgb() | Should -Be ([Drawing.Color]::Lime.ToArgb())
            $bitmap.GetPixel(0, 1).ToArgb() | Should -Be ([Drawing.Color]::Blue.ToArgb())
            $bitmap.GetPixel(1, 1).ToArgb() | Should -Be ([Drawing.Color]::White.ToArgb())
        }
        finally {
            $bitmap.Dispose()
        }
    }
}
