# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Creates persistent storage and EFI variables for an OME guest.
.DESCRIPTION
Creates a qcow2 disk with qemu-img, copies the discovered firmware variable
template, and records guest metadata. Existing files are preserved unless Force
is supplied.
.PARAMETER Name
Guest directory name below OME_HOME\vm.
.PARAMETER SizeGB
Virtual qcow2 capacity in GiB.
.PARAMETER Force
Overwrites disk, variable store, and metadata without prompting.
.PARAMETER BootInstaller
Prints manual installer steps and starts the guest with the manifest ISO.
.EXAMPLE
./New-GuestDisk.ps1 -Name default -SizeGB 32
.EXAMPLE
./New-GuestDisk.ps1 -Name default -BootInstaller
#>
[CmdletBinding()]
param(
    [string]$Name = 'default',

    [ValidateRange(1, 2048)]
    [int]$SizeGB = 32,

    [switch]$Force,
    [switch]$BootInstaller
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force

$qemu = Find-OmeQemu
if ($null -eq $qemu -or $null -eq $qemu.ImgExe) {
    throw 'qemu-img.exe was not found. Install QEMU or set OME_QEMU_DIR.'
}
$firmware = Find-OmeFirmware -Qemu $qemu
if ($null -eq $firmware) {
    throw 'EDK2/OVMF firmware was not found. Install QEMU firmware or set OME_FIRMWARE_DIR.'
}

$vmDirectory = Get-OmePath -Kind vm -Name $Name
$diskPath = [IO.Path]::Combine($vmDirectory, 'disk.qcow2')
$varsPath = [IO.Path]::Combine($vmDirectory, 'efivars.fd')
$metadataPath = [IO.Path]::Combine($vmDirectory, 'vm.json')
$artifact = @((Get-OmeManifest).artifacts | Where-Object { $_.fetched_by -ceq 'installer' }) | Select-Object -First 1
if ($null -eq $artifact) {
    throw 'No installer artifact is declared in manifests/artifacts.json.'
}

if ($Force) {
    foreach ($path in @($diskPath, $varsPath, $metadataPath)) {
        if ([IO.File]::Exists($path)) {
            Remove-Item -LiteralPath $path -Force
        }
    }
}

if (-not [IO.File]::Exists($diskPath)) {
    & $qemu.ImgExe 'create' '-f' 'qcow2' $diskPath "${SizeGB}G"
    if ($LASTEXITCODE -ne 0) {
        throw "qemu-img failed with exit code $LASTEXITCODE."
    }
    Write-Host "Created disk: $diskPath"
}
else {
    Write-Host "Disk already exists; skipped: $diskPath"
}

if (-not [IO.File]::Exists($varsPath)) {
    Copy-Item -LiteralPath $firmware.VarsTemplate -Destination $varsPath
    Write-Host "Created EFI variable store: $varsPath"
}
else {
    Write-Host "EFI variable store already exists; skipped: $varsPath"
}

if (-not [IO.File]::Exists($metadataPath)) {
    $metadata = [ordered]@{
        name = $Name
        sizeGB = $SizeGB
        created = [DateTimeOffset]::Now.ToString('o')
        isoArtifactName = [string]$artifact.name
        notes = 'Persistent Bliss OS guest disk. Installation is performed manually from the manifest ISO.'
    }
    $metadata | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $metadataPath -Encoding utf8
    Write-Host "Created metadata: $metadataPath"
}
else {
    Write-Host "Metadata already exists; skipped: $metadataPath"
}

if ($BootInstaller) {
    Write-Host ''
    Write-Host 'Bliss installer steps:'
    Write-Host '1. Choose Install from the boot menu.'
    Write-Host '2. Create a GPT partition table.'
    Write-Host '3. Create an EFI system partition and one ext4 partition using the remaining disk.'
    Write-Host '4. Select the ext4 partition and format it as ext4.'
    Write-Host '5. Install GRUB EFI when prompted.'
    Write-Host '6. Skip making /system read-write unless a later test specifically requires it.'
    Write-Host ''
    & ([IO.Path]::Combine($PSScriptRoot, 'Start-Guest.ps1')) -Name $Name -Cdrom
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
