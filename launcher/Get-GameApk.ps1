# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Pulls an installed application from the user's own Android device.
.DESCRIPTION
Selects one connected adb device, reads all package paths, pulls the base and
split APKs into OME_HOME, records hashes and phone provenance, and checks the
base APK signer against the compatibility record when apksigner is available.
.PARAMETER Package
Android application package identifier.
.PARAMETER DeviceSerial
Specific adb device serial. Required when more than one device is connected.
.PARAMETER OutDir
Destination directory. Defaults to OME_HOME\apks\Package.
.PARAMETER Compat
Compatibility JSON path. Defaults to compat\Package.json.
.EXAMPLE
./Get-GameApk.ps1 -Package com.example.game
.EXAMPLE
./Get-GameApk.ps1 -Package com.example.game -DeviceSerial R123456
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^[A-Za-z][A-Za-z0-9_]*(?:\.[A-Za-z][A-Za-z0-9_]*)+$')]
    [string]$Package,

    [string]$DeviceSerial,
    [string]$OutDir,
    [string]$Compat
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force

$adb = Find-OmeAdb
if ($null -eq $adb) {
    throw 'adb.exe was not found. Install Android SDK platform-tools or set OME_ADB.'
}
$repoRoot = Get-OmeRepoRoot
if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = Get-OmePath -Kind apks -Name $Package
}
else {
    $OutDir = [IO.Path]::GetFullPath($OutDir)
    [void][IO.Directory]::CreateDirectory($OutDir)
}
if ([string]::IsNullOrWhiteSpace($Compat)) {
    $Compat = [IO.Path]::Combine($repoRoot, 'compat', "$Package.json")
}
else {
    $Compat = [IO.Path]::GetFullPath($Compat)
}

function Invoke-Adb {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string[]]$Arguments
    )

    $output = @(& $adb @Arguments 2>&1)
    return [pscustomobject]@{
        ExitCode = $LASTEXITCODE
        Text = ($output | ForEach-Object { [string]$_ }) -join "`n"
    }
}

$deviceResult = Invoke-Adb -Arguments @('devices', '-l')
if ($deviceResult.ExitCode -ne 0) {
    throw "adb devices failed: $($deviceResult.Text)"
}
Write-Host $deviceResult.Text
$devices = @(
    $deviceResult.Text -split "`r?`n" |
        Where-Object { $_ -match '^([^\s]+)\s+device(?:\s|$)' } |
        ForEach-Object {
            [pscustomobject]@{ Serial = [regex]::Match($_, '^([^\s]+)').Groups[1].Value; Description = $_ }
        }
)
if ([string]::IsNullOrWhiteSpace($DeviceSerial)) {
    if ($devices.Count -gt 1) {
        Write-Host 'More than one adb device is connected. Re-run with -DeviceSerial:' -ForegroundColor Yellow
        foreach ($device in $devices) {
            Write-Host "  $($device.Description)"
        }
        exit 2
    }
    if ($devices.Count -eq 0) {
        Write-Host 'No authorized adb device is connected.' -ForegroundColor Red
        exit 2
    }
    $DeviceSerial = $devices[0].Serial
}
elseif ($DeviceSerial -notin @($devices.Serial)) {
    Write-Host "Device '$DeviceSerial' is not connected and authorized." -ForegroundColor Red
    exit 2
}

$pathResult = Invoke-Adb -Arguments @('-s', $DeviceSerial, 'shell', 'pm', 'path', $Package)
if ($pathResult.ExitCode -ne 0) {
    throw "Could not list APK paths for ${Package}: $($pathResult.Text)"
}
$remotePaths = @(
    $pathResult.Text -split "`r?`n" |
        Where-Object { $_ -match '^package:(.+)$' } |
        ForEach-Object { [regex]::Match($_, '^package:(.+)$').Groups[1].Value.Trim() }
)
if ($remotePaths.Count -eq 0) {
    throw "Package is not installed on device '$DeviceSerial': $Package"
}

$existingNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$pulledPaths = [Collections.Generic.List[string]]::new()
$remoteToLocal = [Collections.Generic.List[object]]::new()
$recordPath = [IO.Path]::Combine($OutDir, 'pulled.json')
try {
    foreach ($remotePath in $remotePaths) {
        $fileName = [IO.Path]::GetFileName($remotePath)
        if ([string]::IsNullOrWhiteSpace($fileName)) {
            throw "Could not determine a filename for remote APK: $remotePath"
        }
        $candidateName = $fileName
        $suffix = 1
        while (-not $existingNames.Add($candidateName)) {
            $candidateName = "$([IO.Path]::GetFileNameWithoutExtension($fileName))-$suffix$([IO.Path]::GetExtension($fileName))"
            $suffix++
        }
        $localPath = [IO.Path]::Combine($OutDir, $candidateName)
        $pullResult = Invoke-Adb -Arguments @('-s', $DeviceSerial, 'pull', $remotePath, $localPath)
        if ($pullResult.ExitCode -ne 0 -or -not [IO.File]::Exists($localPath)) {
            throw "adb pull failed for ${remotePath}: $($pullResult.Text)"
        }
        $pulledPaths.Add($localPath)
        $remoteToLocal.Add([pscustomobject]@{ Remote = $remotePath; Local = $localPath })
        Write-Host "Pulled: $remotePath -> $localPath"
    }

    $fingerprintResult = Invoke-Adb -Arguments @('-s', $DeviceSerial, 'shell', 'getprop', 'ro.build.fingerprint')
    $fingerprint = if ($fingerprintResult.ExitCode -eq 0) { $fingerprintResult.Text.Trim() } else { "unavailable: $($fingerprintResult.Text)" }

    $baseMapping = $remoteToLocal | Where-Object { [IO.Path]::GetFileName($_.Remote) -ceq 'base.apk' } | Select-Object -First 1
    if ($null -eq $baseMapping) {
        $baseMapping = $remoteToLocal | Select-Object -First 1
    }
    $signerVerified = $false
    $signerActual = $null
    $signerExpected = $null
    $signer = Find-OmeApkSigner
    if ([IO.File]::Exists($Compat)) {
        $compatibility = Get-Content -LiteralPath $Compat -Raw | ConvertFrom-Json
        if ($compatibility.PSObject.Properties.Name -contains 'signer_sha1' -and -not [string]::IsNullOrWhiteSpace([string]$compatibility.signer_sha1)) {
            $signerExpected = ([string]$compatibility.signer_sha1 -replace '[:\s]', '').ToLowerInvariant()
        }
    }

    if ($null -ne $signerExpected -and $null -ne $signer) {
        $signerOutput = @(& $signer 'verify' '--print-certs' $baseMapping.Local 2>&1)
        $signerExitCode = $LASTEXITCODE
        $signerText = ($signerOutput | ForEach-Object { [string]$_ }) -join "`n"
        if ($signerExitCode -ne 0) {
            throw "apksigner verification failed: $signerText"
        }
        $match = [regex]::Match($signerText, '(?im)certificate SHA-1 digest:\s*([0-9a-f: ]+)')
        if (-not $match.Success) {
            throw 'apksigner did not report a certificate SHA-1 digest.'
        }
        $signerActual = ($match.Groups[1].Value -replace '[:\s]', '').ToLowerInvariant()
        if ($signerActual -cne $signerExpected) {
            foreach ($path in $pulledPaths) {
                Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
            }
            Remove-Item -LiteralPath $recordPath -Force -ErrorAction SilentlyContinue
            Write-Host "Signer mismatch for $Package. Expected $signerExpected, got $signerActual. Pulled files were deleted." -ForegroundColor Red
            exit 1
        }
        $signerVerified = $true
        Write-Host 'signer verified'
    }
    elseif ($null -ne $signerExpected -and $null -eq $signer) {
        Write-Warning 'apksigner was not found. Files were kept, but signer_verified is false.'
    }
    elseif ($null -eq $signerExpected) {
        Write-Warning 'No signer_sha1 was found in the compatibility file. Files were kept, but signer_verified is false.'
    }

    $fileRecords = @(
        foreach ($mapping in $remoteToLocal) {
            $item = Get-Item -LiteralPath $mapping.Local
            [ordered]@{
                file = $item.Name
                remote_path = $mapping.Remote
                size = $item.Length
                sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            }
        }
    )
    $record = [ordered]@{
        package = $Package
        files = $fileRecords
        'pulled-at' = [DateTimeOffset]::Now.ToString('o')
        device_serial = $DeviceSerial
        device_fingerprint = $fingerprint
        signer_expected_sha1 = $signerExpected
        signer_actual_sha1 = $signerActual
        signer_verified = $signerVerified
    }
    $record | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $recordPath -Encoding utf8
    Write-Host "Provenance record: $recordPath"
}
catch {
    foreach ($path in $pulledPaths) {
        Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $recordPath -Force -ErrorAction SilentlyContinue
    throw
}
