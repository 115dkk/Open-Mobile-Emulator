# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Validates the files prepared for one GitHub release.

.DESCRIPTION
Checks the installer and its checksum, the QEMU binary and source bundles,
license notices, the complete SHA256SUMS file, and the flat allowlisted file
set. Every policy finding is reported before the script exits.

.PARAMETER Path
Specifies the flat directory that contains the prepared release files.

.PARAMETER Version
Specifies the release version as three dot-separated decimal integers.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Add-ReleaseFinding {
    param(
        [Parameter(Mandatory)]
        [AllowEmptyCollection()]
        [System.Collections.Generic.List[string]]$Findings,
        [Parameter(Mandatory)][string]$Subject,
        [Parameter(Mandatory)][string]$Description
    )

    [void]$Findings.Add("FINDING ${Subject}: $Description")
}

function Get-LowercaseSha256 {
    param([Parameter(Mandatory)][string]$LiteralPath)

    return (Get-FileHash -LiteralPath $LiteralPath -Algorithm SHA256).Hash.ToLowerInvariant()
}

$findings = [System.Collections.Generic.List[string]]::new()
$directory = [System.IO.Path]::GetFullPath($Path)
if (-not [System.IO.Directory]::Exists($directory)) {
    Add-ReleaseFinding $findings $Path 'release directory does not exist'
    foreach ($finding in $findings) {
        Write-Output $finding
    }
    Write-Output "SUMMARY release-assets findings=$($findings.Count) checked=0"
    exit 1
}
if ($Version -cnotmatch '^\d+\.\d+\.\d+$') {
    Add-ReleaseFinding $findings 'version' 'must match x.y.z with decimal integers'
}

$entries = @(Get-ChildItem -LiteralPath $directory -Force)
$files = @($entries | Where-Object { -not $_.PSIsContainer })
$directories = @($entries | Where-Object { $_.PSIsContainer })
foreach ($childDirectory in $directories) {
    Add-ReleaseFinding $findings $childDirectory.Name 'subdirectories are not allowed'
}

$installerName = "Open-Mobile-Emulator-$Version-x64-setup.exe"
$installerHashName = "$installerName.sha256"
$installer = @($files | Where-Object { $_.Name -ceq $installerName })
if ($installer.Count -ne 1) {
    Add-ReleaseFinding $findings $installerName "expected exactly one installer, found $($installer.Count)"
}
$executables = @($files | Where-Object { $_.Extension -ieq '.exe' })
if ($executables.Count -ne 1) {
    Add-ReleaseFinding $findings '*.exe' "expected only the release installer, found $($executables.Count)"
}
elseif ($executables[0].Name -cne $installerName) {
    Add-ReleaseFinding $findings $executables[0].Name "installer name must be $installerName"
}

$installerHashFiles = @($files | Where-Object { $_.Name -ceq $installerHashName })
if ($installerHashFiles.Count -ne 1) {
    Add-ReleaseFinding $findings $installerHashName "expected exactly one installer hash file, found $($installerHashFiles.Count)"
}
elseif ($installer.Count -eq 1) {
    $actualInstallerHash = Get-LowercaseSha256 $installer[0].FullName
    $hashTokens = @(
        ([System.IO.File]::ReadAllText($installerHashFiles[0].FullName) -split '\s+') |
            Where-Object { $_.Length -eq 64 -and $_ -cmatch '^[0-9a-f]{64}$' }
    )
    if ($actualInstallerHash -cnotin $hashTokens) {
        Add-ReleaseFinding $findings $installerHashName 'does not contain the lowercase installer SHA-256 token'
    }
}

$qemuBundles = @($files | Where-Object { $_.Name -clike 'qemu-ome-*-win64.zip' })
if ($qemuBundles.Count -ne 1) {
    Add-ReleaseFinding $findings 'qemu-ome-*-win64.zip' "expected exactly one QEMU bundle, found $($qemuBundles.Count)"
}
$sourceBundles = @($files | Where-Object { $_.Name -clike 'qemu-source-offer-*.tar.gz' })
if ($sourceBundles.Count -ne 1) {
    Add-ReleaseFinding $findings 'qemu-source-offer-*.tar.gz' "expected exactly one source bundle, found $($sourceBundles.Count)"
}
foreach ($requiredName in @('THIRD_PARTY.md', 'NOTICE', 'LICENSE')) {
    if (@($files | Where-Object { $_.Name -ceq $requiredName }).Count -ne 1) {
        Add-ReleaseFinding $findings $requiredName 'required release file is missing'
    }
}

$allowedNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
foreach ($name in @($installerName, $installerHashName, 'THIRD_PARTY.md', 'NOTICE', 'LICENSE', 'SHA256SUMS')) {
    [void]$allowedNames.Add($name)
}
foreach ($bundle in @($qemuBundles + $sourceBundles)) {
    [void]$allowedNames.Add($bundle.Name)
}
foreach ($file in $files) {
    if (-not $allowedNames.Contains($file.Name)) {
        Add-ReleaseFinding $findings $file.Name 'file is not allowed in the release directory'
    }
}

$fileNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
foreach ($file in $files) {
    [void]$fileNames.Add($file.Name)
}
$sumsFiles = @($files | Where-Object { $_.Name -ceq 'SHA256SUMS' })
if ($sumsFiles.Count -ne 1) {
    Add-ReleaseFinding $findings 'SHA256SUMS' "expected exactly one checksum manifest, found $($sumsFiles.Count)"
}
else {
    $listed = [System.Collections.Generic.Dictionary[string, string]]::new([System.StringComparer]::Ordinal)
    $lineNumber = 0
    foreach ($line in [System.IO.File]::ReadAllLines($sumsFiles[0].FullName)) {
        $lineNumber++
        if ($line -cnotmatch '^([0-9A-Fa-f]{64})  (.+)$') {
            Add-ReleaseFinding $findings "SHA256SUMS:$lineNumber" 'line must be a 64-character hexadecimal SHA-256, two spaces, and a file name'
            continue
        }
        $hash = $Matches[1].ToLowerInvariant()
        $name = $Matches[2]
        if ($name -ceq 'SHA256SUMS') {
            Add-ReleaseFinding $findings "SHA256SUMS:$lineNumber" 'must not list SHA256SUMS itself'
            continue
        }
        if ($listed.ContainsKey($name)) {
            Add-ReleaseFinding $findings $name 'is listed more than once in SHA256SUMS'
            continue
        }
        $listed.Add($name, $hash)
        if (-not $fileNames.Contains($name)) {
            Add-ReleaseFinding $findings $name 'SHA256SUMS refers to a file that is not present'
            continue
        }
        $target = Join-Path $directory $name
        if ((Get-LowercaseSha256 $target) -cne $hash) {
            Add-ReleaseFinding $findings $name 'SHA256SUMS hash does not match the file'
        }
    }
    foreach ($file in $files | Where-Object { $_.Name -cne 'SHA256SUMS' }) {
        if (-not $listed.ContainsKey($file.Name)) {
            Add-ReleaseFinding $findings $file.Name 'is missing from SHA256SUMS'
        }
    }
}

foreach ($finding in $findings) {
    Write-Output $finding
}
Write-Output "SUMMARY release-assets findings=$($findings.Count) checked=$($files.Count)"
if ($findings.Count -gt 0) {
    exit 1
}
exit 0
