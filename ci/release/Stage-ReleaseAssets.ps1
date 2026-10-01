# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0
<#
.SYNOPSIS
Gathers the files of a product release into one flat directory with fixed names.

.DESCRIPTION
Takes the NSIS installer that `tauri build` produced, the verified QEMU release files that
Get-QemuBundle.ps1 downloaded, and the repository's THIRD_PARTY.md, NOTICE and LICENSE, and lays
them out as release.yml publishes them:

  Open-Mobile-Emulator-<version>-x64-setup.exe          the installer (the only .exe; the product's
                                                        update check accepts one installer asset)
  Open-Mobile-Emulator-<version>-x64-setup.exe.sha256   its SHA-256, read by the update check
  qemu-ome-<tag>-<commit>-win64.zip                     the bundled QEMU runtime, as released by CI
  qemu-source-offer-<tag>-<commit>.tar.gz               the corresponding source (R4)
  THIRD_PARTY.md  NOTICE  LICENSE                       notices (M3 item 3)
  SHA256SUMS                                            SHA-256 of every file above

ci/Check-ReleaseAssets.ps1 then checks the directory before anything is uploaded. The installer's
file name drops the spaces of the product name because GitHub rewrites spaces in asset names.

.PARAMETER Version
The product version, which must equal the version in host/app/tauri.conf.json.
.PARAMETER Installer
Path of the installer `tauri build` wrote under host/target/release/bundle/nsis/.
.PARAMETER QemuFiles
Directory holding the downloaded QEMU release files (Get-QemuBundle.ps1 -Destination).
.PARAMETER Destination
Directory that receives the staged files (must be absent or empty).
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$QemuFiles,
    [Parameter(Mandatory)][string]$Destination
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version must be x.y.z: $Version" }
$configured = (Get-Content -LiteralPath (Join-Path $repo 'host\app\tauri.conf.json') -Raw | ConvertFrom-Json).version
if ($configured -ne $Version) { throw "tauri.conf.json says $configured, not $Version" }
if (-not (Test-Path -LiteralPath $Installer -PathType Leaf)) { throw "Installer not found: $Installer" }
if ((Test-Path -LiteralPath $Destination) -and (Get-ChildItem -LiteralPath $Destination -Force | Select-Object -First 1)) {
    throw "Destination is not empty: $Destination"
}
[void][IO.Directory]::CreateDirectory($Destination)

$installerName = "Open-Mobile-Emulator-$Version-x64-setup.exe"
Copy-Item -LiteralPath $Installer -Destination (Join-Path $Destination $installerName)
$installerHash = (Get-FileHash -LiteralPath (Join-Path $Destination $installerName) -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText((Join-Path $Destination "$installerName.sha256"), "$installerHash  $installerName`n", [Text.UTF8Encoding]::new($false))

$zip = @(Get-ChildItem -LiteralPath $QemuFiles -Filter 'qemu-ome-*-win64.zip')
$offer = @(Get-ChildItem -LiteralPath $QemuFiles -Filter 'qemu-source-offer-*.tar.gz')
if ($zip.Count -ne 1) { throw "Expected one QEMU runtime zip in $QemuFiles, found $($zip.Count)" }
if ($offer.Count -ne 1) { throw "Expected one QEMU source offer in $QemuFiles, found $($offer.Count)" }
Copy-Item -LiteralPath $zip[0].FullName -Destination $Destination
Copy-Item -LiteralPath $offer[0].FullName -Destination $Destination
foreach ($notice in 'THIRD_PARTY.md', 'NOTICE', 'LICENSE') {
    Copy-Item -LiteralPath (Join-Path $repo $notice) -Destination (Join-Path $Destination $notice)
}

$sums = foreach ($file in Get-ChildItem -LiteralPath $Destination -File | Sort-Object Name) {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $file.Name
}
[IO.File]::WriteAllText((Join-Path $Destination 'SHA256SUMS'), (($sums -join "`n") + "`n"), [Text.UTF8Encoding]::new($false))
Get-ChildItem -LiteralPath $Destination -File | Sort-Object Name | ForEach-Object { '{0,14:N0}  {1}' -f $_.Length, $_.Name }
