# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Downloads and verifies external artifacts declared in the OME manifest.
.DESCRIPTION
Refuses unapproved hosts, resumes downloads with curl.exe, and verifies file
size and SHA-256 before accepting an artifact. Existing verified files are left
unchanged.
.PARAMETER Name
One or more manifest artifact names. All artifacts are selected when omitted.
.PARAMETER Force
Removes an existing unverified file before downloading it again.
.PARAMETER VerifyOnly
Checks local files without making any network request.
.EXAMPLE
./Get-Artifacts.ps1
.EXAMPLE
./Get-Artifacts.ps1 -Name bliss-os-16.9.7-gapps-x86_64-iso -VerifyOnly
#>
[CmdletBinding()]
param(
    [string[]]$Name,
    [switch]$Force,
    [switch]$VerifyOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force

function Test-ArtifactFile {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [object]$Artifact,

        [Parameter(Mandatory)]
        [string]$Path
    )

    if (-not [IO.File]::Exists($Path)) {
        return [pscustomobject]@{ Valid = $false; Hash = $null; Reason = 'missing' }
    }

    $actualSize = (Get-Item -LiteralPath $Path).Length
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    $expected = ([string]$Artifact.sha256).Trim().ToLowerInvariant()
    $reasons = [Collections.Generic.List[string]]::new()
    if ($Artifact.PSObject.Properties.Name -contains 'size_bytes' -and $null -ne $Artifact.size_bytes -and $actualSize -ne [long]$Artifact.size_bytes) {
        $reasons.Add("size $actualSize, expected $($Artifact.size_bytes)")
    }
    if ($hash -cne $expected) {
        $reasons.Add("SHA-256 mismatch; expected $expected")
    }
    if ($reasons.Count -gt 0) {
        return [pscustomobject]@{ Valid = $false; Hash = $hash; Reason = ($reasons -join '; ') }
    }
    return [pscustomobject]@{ Valid = $true; Hash = $hash; Reason = 'verified' }
}

$manifest = Get-OmeManifest
$artifacts = if ($null -eq $Name -or $Name.Count -eq 0) {
    @($manifest.artifacts)
}
else {
    @($Name | ForEach-Object { Get-OmeArtifact -Name $_ })
}
$artifactDirectory = Get-OmePath -Kind artifacts
$curl = if ($VerifyOnly) { $null } else { Get-Command 'curl.exe' -CommandType Application -ErrorAction Stop | Select-Object -First 1 }
$verificationFailures = 0

foreach ($artifact in $artifacts) {
    if ([string]::IsNullOrWhiteSpace([string]$artifact.sha256)) {
        throw "Artifact has no SHA-256: $($artifact.name)"
    }
    if (-not (Test-OmeAllowedUrl -Url ([string]$artifact.url))) {
        throw "Artifact URL host is not allowed by the manifest: $($artifact.url)"
    }

    $fileName = [IO.Path]::GetFileName([string]$artifact.filename)
    if ($fileName -cne [string]$artifact.filename) {
        throw "Artifact filename must not contain a directory: $($artifact.filename)"
    }
    $target = [IO.Path]::Combine($artifactDirectory, $fileName)
    $result = Test-ArtifactFile -Artifact $artifact -Path $target
    if ($result.Valid) {
        Write-Host "$($artifact.name): verified"
        Write-Host "SHA-256: $($result.Hash)"
        continue
    }

    if ($VerifyOnly) {
        Write-Host "$($artifact.name): verification failed ($($result.Reason))" -ForegroundColor Red
        if ($null -ne $result.Hash) {
            Write-Host "SHA-256: $($result.Hash)"
        }
        $verificationFailures++
        continue
    }

    if ($Force -and [IO.File]::Exists($target)) {
        Remove-Item -LiteralPath $target -Force
    }
    Write-Host "$($artifact.name): downloading from approved host"
    $curlArguments = @(
        '-L',
        '--fail',
        '--retry', '5',
        '--retry-delay', '5',
        '-C', '-',
        '--progress-bar',
        '-o', $target,
        [string]$artifact.url
    )
    & $curl.Source @curlArguments
    if ($LASTEXITCODE -ne 0) {
        throw "curl.exe failed with exit code $LASTEXITCODE for $($artifact.name)."
    }

    $result = Test-ArtifactFile -Artifact $artifact -Path $target
    if (-not $result.Valid) {
        if ([IO.File]::Exists($target)) {
            Remove-Item -LiteralPath $target -Force
        }
        if ($null -ne $result.Hash) {
            Write-Host "SHA-256: $($result.Hash)"
        }
        throw "Downloaded artifact failed verification: $($result.Reason). The file was deleted."
    }
    Write-Host "$($artifact.name): verified"
    Write-Host "SHA-256: $($result.Hash)"
}

if ($VerifyOnly -and $verificationFailures -gt 0) {
    throw "$verificationFailures artifact verification check(s) failed."
}
