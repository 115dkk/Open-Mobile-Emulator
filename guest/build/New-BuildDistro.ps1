# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
<#
.SYNOPSIS
Creates the WSL 2 build distro for the self-built guest image on a Windows development PC.

.DESCRIPTION
Downloads the Ubuntu 22.04 WSL rootfs pinned in pins.env (URL and SHA-256), imports it as the
distro OME-Build under the given folder, and runs setup-build-host.sh inside it as root. Idempotent:
an existing download is reused after its checksum is verified, an existing distro is not re-imported.
The distro's disk grows on the drive that holds -InstallPath; the build needs about 250 GB there.

.EXAMPLE
pwsh -File guest/build/New-BuildDistro.ps1 -InstallPath C:\WSL\OME-Build
#>
[CmdletBinding()]
param(
    [string]$Name = 'OME-Build',
    [string]$InstallPath = 'C:\WSL\OME-Build',
    [string]$DownloadDir = 'C:\WSL'
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$pins = @{}
Get-Content (Join-Path $here 'pins.env') | Where-Object { $_ -match '^[A-Z0-9_]+=' } | ForEach-Object {
    $k, $v = $_ -split '=', 2
    $pins[$k] = $v.Trim('"')
}
$url = $pins['BUILD_HOST_ROOTFS_URL']
$sha = $pins['BUILD_HOST_ROOTFS_SHA256']
$tar = Join-Path $DownloadDir (Split-Path -Leaf $url)

New-Item -ItemType Directory -Force -Path $DownloadDir | Out-Null
if (-not (Test-Path $tar)) {
    Write-Host "==> downloading $url"
    curl.exe -fL --retry 3 -o $tar $url
}
$actual = (Get-FileHash $tar -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $sha) { throw "rootfs checksum mismatch: expected $sha, got $actual ($tar)" }
Write-Host "==> rootfs verified ($tar)"

$existing = (wsl.exe --list --quiet 2>$null) -replace "`0", '' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
if ($existing -contains $Name) {
    Write-Host "==> distro $Name already exists; skipping import"
} else {
    Write-Host "==> importing $Name into $InstallPath"
    New-Item -ItemType Directory -Force -Path $InstallPath | Out-Null
    wsl.exe --import $Name $InstallPath $tar --version 2
    if ($LASTEXITCODE -ne 0) { throw "wsl --import failed ($LASTEXITCODE)" }
}

$repoRoot = (Resolve-Path (Join-Path $here '..\..')).Path
$drive = $repoRoot.Substring(0, 1).ToLower()
$wslRepo = '/mnt/' + $drive + ($repoRoot.Substring(2) -replace '\\', '/')
Write-Host "==> running setup-build-host.sh as root inside $Name"
wsl.exe -d $Name -u root -- bash -c "cp '$wslRepo/guest/build/pins.env' '$wslRepo/guest/build/setup-build-host.sh' /tmp/ && sed -i 's/\r$//' /tmp/pins.env /tmp/setup-build-host.sh && bash /tmp/setup-build-host.sh"
if ($LASTEXITCODE -ne 0) { throw "setup-build-host.sh failed ($LASTEXITCODE)" }
Write-Host "==> restarting $Name so wsl.conf (systemd, default user) applies"
wsl.exe --terminate $Name
wsl.exe -d $Name -- bash -c 'echo "user: $(id -un), cpus: $(nproc), ram: $(free -g | awk ''/Mem:/ {print $2}'') GB, disk: $(df -h / | tail -1)"'
Write-Host "==> next: wsl -d $Name -- bash -lc 'OME_REPO_ROOT=`"$wslRepo`" `"$wslRepo/guest/build/build-image.sh`" all'"
