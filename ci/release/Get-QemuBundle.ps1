# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0
<#
.SYNOPSIS
Downloads the QEMU release that manifests/qemu-release.json names, checks every file and unpacks the
runtime for the product build.

.DESCRIPTION
The product release (release.yml) does not build QEMU itself; it takes the runtime zip and the
corresponding-source archive from the QEMU release that qemu-release.yml built and attested
(ADR-0007). This script fetches those two files plus the release's SHA256SUMS with gh, compares each
SHA-256 with the manifest, verifies the build provenance attestation of each file against the
qemu-release.yml workflow of this repository, and unpacks the zip into the staging directory that
ci/release/tauri.release.conf.json bundles as qemu/. A mismatch of any kind stops with exit 1 and
nothing is unpacked.

Needs gh authenticated for the repository (GH_TOKEN on a runner). Nothing here is product
behaviour; the product's own update check lives in the runtime.

.PARAMETER Manifest
Path of manifests/qemu-release.json.
.PARAMETER Destination
Directory that receives the downloaded files (created; must be empty or absent).
.PARAMETER ExtractTo
Directory that receives the unpacked runtime (bin/, share/); replaced when it exists.
.PARAMETER SkipAttestation
Skips the provenance check. Only for a local trial against a release that has no attestation; the
release workflow never passes it.
#>
[CmdletBinding()]
param(
    [string]$Manifest = (Join-Path $PSScriptRoot '..\..\manifests\qemu-release.json'),
    [Parameter(Mandatory)][string]$Destination,
    [string]$ExtractTo = '',
    [switch]$SkipAttestation
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$pin = Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json
foreach ($name in 'repository', 'tag', 'runtime_zip', 'runtime_zip_sha256', 'source_offer', 'source_offer_sha256', 'signer_workflow') {
    if (-not ($pin.PSObject.Properties.Name -contains $name) -or [string]::IsNullOrWhiteSpace($pin.$name)) {
        throw "manifests/qemu-release.json lacks '$name'"
    }
}
foreach ($name in 'runtime_zip_sha256', 'source_offer_sha256') {
    if ($pin.$name -notmatch '^[0-9a-f]{64}$') { throw "'$name' is not a lower-case SHA-256" }
}
if ((Test-Path -LiteralPath $Destination) -and (Get-ChildItem -LiteralPath $Destination -Force | Select-Object -First 1)) {
    throw "Destination is not empty: $Destination"
}
[void][IO.Directory]::CreateDirectory($Destination)

$files = @(
    @{ name = $pin.runtime_zip; sha256 = $pin.runtime_zip_sha256 },
    @{ name = $pin.source_offer; sha256 = $pin.source_offer_sha256 }
)
foreach ($file in $files + @(@{ name = 'SHA256SUMS'; sha256 = '' })) {
    gh release download $pin.tag --repo $pin.repository --pattern $file.name --dir $Destination
    if ($LASTEXITCODE) { throw "gh release download failed for $($file.name) (exit $LASTEXITCODE)" }
}
foreach ($file in $files) {
    $path = Join-Path $Destination $file.name
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing after download: $($file.name)" }
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $file.sha256) { throw "SHA-256 mismatch for $($file.name): manifest $($file.sha256), file $actual" }
    "verified SHA-256 $($file.name)"
    if (-not $SkipAttestation) {
        gh attestation verify $path --repo $pin.repository --signer-workflow $pin.signer_workflow
        if ($LASTEXITCODE) { throw "Provenance attestation check failed for $($file.name) (exit $LASTEXITCODE)" }
        "verified provenance $($file.name)"
    }
}
$sums = Get-Content -LiteralPath (Join-Path $Destination 'SHA256SUMS')
foreach ($file in $files) {
    $line = $sums | Where-Object { $_ -match "^[0-9a-f]{64}\s+$([regex]::Escape($file.name))$" }
    if (-not $line -or ($line -split '\s+')[0] -ne $file.sha256) { throw "The release's SHA256SUMS does not list $($file.name) with the manifest's digest" }
}
"the release's SHA256SUMS agrees with the manifest"

if ($ExtractTo) {
    if (Test-Path -LiteralPath $ExtractTo) { [IO.Directory]::Delete($ExtractTo, $true) }
    Expand-Archive -LiteralPath (Join-Path $Destination $pin.runtime_zip) -DestinationPath $ExtractTo
    foreach ($required in 'bin\qemu-system-x86_64.exe', 'bin\qemu-img.exe', 'share\qemu\edk2-x86_64-code.fd', 'share\qemu\edk2-i386-vars.fd') {
        if (-not (Test-Path -LiteralPath (Join-Path $ExtractTo $required))) { throw "The runtime zip lacks $required" }
    }
    $count = @(Get-ChildItem -LiteralPath $ExtractTo -Recurse -File).Count
    "unpacked $count files to $ExtractTo"
}
