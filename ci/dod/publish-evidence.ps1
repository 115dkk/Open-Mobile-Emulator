# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Source,
    [Parameter(Mandatory)][string]$Destination
)
$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path -LiteralPath $Source).Path
if (Test-Path -LiteralPath $Destination) { throw 'Publish destination must not exist' }
$files = @(Get-ChildItem -LiteralPath $sourceRoot -Recurse -Force)
foreach ($entry in $files) {
    $relative = [IO.Path]::GetRelativePath($sourceRoot, $entry.FullName)
    if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Evidence contains a reparse point' }
    if ($relative -match '(^|[\\/])(fixtures|artifacts|vm|\.git)([\\/]|$)' -or
        $entry.Extension -match '^\.(apk|xapk|apks|iso|img|qcow2|vdi|sfs|zip|gz|key)$') {
        throw 'Private fixture or guest storage found in evidence; publication refused'
    }
    if (!$entry.PSIsContainer -and $entry.Extension -notin @('.png', '.txt', '.md', '.json')) {
        throw 'Unexpected evidence file type; publication refused'
    }
    if (!$entry.PSIsContainer -and $entry.Extension -ne '.png') {
        $text = [IO.File]::ReadAllText($entry.FullName)
        if ($text -match 'BEGIN OPENSSH PRIVATE KEY|https?://[^\s"]+[?&](token|signature|sig)=') {
            throw 'Credential or signed download URL found in evidence'
        }
    }
}
[void][IO.Directory]::CreateDirectory($Destination)
# Explicit allowlist staging, never upload the run home or private fixture clone.
foreach ($entry in $files | Where-Object { !$_.PSIsContainer }) {
    $target = Join-Path $Destination ([IO.Path]::GetRelativePath($sourceRoot, $entry.FullName))
    [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target))
    Copy-Item -LiteralPath $entry.FullName -Destination $target
}
Write-Output "Evidence checked and staged: $(@($files | Where-Object { !$_.PSIsContainer }).Count) files"
