# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Builds and signs the OME guest input method without Gradle.

.PARAMETER KeyStore
Existing keystore containing the ome-ime alias. Without this parameter a temporary
RSA key is generated and removed in finally. Existing keystore passwords are read
from OME_IME_STORE_PASSWORD and OME_IME_KEY_PASSWORD (defaults to store password).
#>
[CmdletBinding()]
param([string]$KeyStore)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-ExternalTool {
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [Parameter(Mandatory)][string[]]$ArgumentList,
        [Parameter(Mandatory)][string]$Description
    )
    & $FilePath @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

function Find-AndroidSdk {
    $candidates = @($env:ANDROID_HOME, $env:ANDROID_SDK_ROOT)
    if ($env:LOCALAPPDATA) {
        $candidates += Join-Path $env:LOCALAPPDATA 'Android/Sdk'
    }
    foreach ($candidate in $candidates) {
        if ([string]::IsNullOrWhiteSpace($candidate)) { continue }
        if ((Test-Path -LiteralPath (Join-Path $candidate 'build-tools')) -and
            (Test-Path -LiteralPath (Join-Path $candidate 'platforms'))) {
            return [System.IO.Path]::GetFullPath($candidate)
        }
    }
    throw 'Android SDK not found. Set ANDROID_HOME or ANDROID_SDK_ROOT.'
}

function Find-JavaTool {
    param([Parameter(Mandatory)][string]$Name)
    if ($env:JAVA_HOME) {
        $tool = Join-Path $env:JAVA_HOME "bin/$Name.exe"
        if (-not (Test-Path -LiteralPath $tool)) { throw "Missing Java tool: $tool" }
        return $tool
    }
    return (Get-Command $Name -ErrorAction Stop).Source
}

$temporaryKey = $null
$intermediate = $null
$oldStorePassword = $env:OME_IME_STORE_PASSWORD
$oldKeyPassword = $env:OME_IME_KEY_PASSWORD
$oldJavaHome = $env:JAVA_HOME
try {
    $versionText = (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'version.txt') -Raw).Trim()
    $versionCode = 0
    if ($versionText -notmatch '^[1-9][0-9]*$' -or
        -not [int]::TryParse($versionText, [ref]$versionCode)) {
        throw 'version.txt must contain one positive versionCode.'
    }
    $versionName = "$versionCode.0"
    $sdk = Find-AndroidSdk
    $versions = @(Get-ChildItem -LiteralPath (Join-Path $sdk 'build-tools') -Directory |
        ForEach-Object {
            $version = $null
            if ([version]::TryParse($_.Name, [ref]$version)) {
                [pscustomobject]@{ Directory = $_.FullName; Version = $version }
            }
        } | Sort-Object Version -Descending)
    if ($versions.Count -eq 0) { throw 'No Android build-tools found.' }
    $tools = $versions[0].Directory
    $platforms = @(Get-ChildItem -LiteralPath (Join-Path $sdk 'platforms') -Directory |
        ForEach-Object {
            if ($_.Name -match '^android-(\d+)$' -and [int]$Matches[1] -ge 33 -and
                (Test-Path -LiteralPath (Join-Path $_.FullName 'android.jar'))) {
                [pscustomobject]@{ Api = [int]$Matches[1]; Directory = $_.FullName }
            }
        } | Sort-Object @{ Expression = { $_.Api -eq 35 }; Descending = $true },
            @{ Expression = { $_.Api }; Descending = $true })
    if ($platforms.Count -eq 0) { throw 'Android platform API 33 or newer is required.' }
    $androidJar = Join-Path $platforms[0].Directory 'android.jar'
    $aapt2 = Join-Path $tools 'aapt2.exe'
    $d8 = Join-Path $tools 'd8.bat'
    $zipalign = Join-Path $tools 'zipalign.exe'
    $apksigner = Join-Path $tools 'apksigner.bat'
    foreach ($tool in @($aapt2, $d8, $zipalign, $apksigner)) {
        if (-not (Test-Path -LiteralPath $tool)) { throw "Missing Android tool: $tool" }
    }
    $javac = Find-JavaTool 'javac'
    $keytool = Find-JavaTool 'keytool'
    # The Android .bat launchers also use JAVA_HOME.
    $env:JAVA_HOME = Split-Path -Parent (Split-Path -Parent $javac)
    $out = Join-Path $PSScriptRoot 'out'
    $intermediate = Join-Path $out ("work-{0}" -f [guid]::NewGuid().ToString('N'))
    $classes = Join-Path $intermediate 'classes'
    $dex = Join-Path $intermediate 'dex'
    foreach ($directory in @($out, $intermediate, $classes, $dex)) {
        [void][System.IO.Directory]::CreateDirectory($directory)
    }
    Write-Output "SDK $sdk"
    Write-Output "BUILD_TOOLS $tools"
    Write-Output "PLATFORM android-$($platforms[0].Api)"
    Write-Output "VERSION $versionCode ($versionName) minSdk=33 targetSdk=35"

    $resources = Join-Path $intermediate 'resources.zip'
    $unsigned = Join-Path $intermediate 'unsigned.apk'
    Invoke-ExternalTool $aapt2 @('compile', '--dir', (Join-Path $PSScriptRoot 'res'),
        '-o', $resources) 'Resource compilation'
    Invoke-ExternalTool $aapt2 @('link', '-o', $unsigned, '--manifest',
        (Join-Path $PSScriptRoot 'AndroidManifest.xml'), '-I', $androidJar,
        '--min-sdk-version', '33', '--target-sdk-version', '35',
        '--version-code', $versionText, '--version-name', $versionName, $resources) 'Resource link'
    $sources = @(Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'java') -Recurse -Filter '*.java' |
        Sort-Object FullName | ForEach-Object FullName)
    Invoke-ExternalTool $javac (@('--release', '11', '-encoding', 'UTF-8', '-classpath',
        $androidJar, '-d', $classes) + $sources) 'Java compilation'
    $classFiles = @(Get-ChildItem -LiteralPath $classes -Recurse -Filter '*.class' |
        Sort-Object FullName | ForEach-Object FullName)
    Invoke-ExternalTool $d8 (@('--release', '--min-api', '33', '--lib', $androidJar,
        '--output', $dex) + $classFiles) 'DEX compilation'
    $archive = [System.IO.Compression.ZipFile]::Open($unsigned, [System.IO.Compression.ZipArchiveMode]::Update)
    try {
        [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive,
            (Join-Path $dex 'classes.dex'), 'classes.dex', [System.IO.Compression.CompressionLevel]::Optimal)
        foreach ($entry in $archive.Entries) {
            $entry.LastWriteTime = [System.DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
        }
    }
    finally { $archive.Dispose() }
    $aligned = Join-Path $intermediate 'aligned.apk'
    Invoke-ExternalTool $zipalign @('-p', '4', $unsigned, $aligned) 'APK alignment'

    if ([string]::IsNullOrWhiteSpace($KeyStore)) {
        $temporaryKey = Join-Path $intermediate 'temporary.p12'
        $KeyStore = $temporaryKey
        $env:OME_IME_STORE_PASSWORD = [Convert]::ToBase64String(
            [System.Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
        $env:OME_IME_KEY_PASSWORD = $env:OME_IME_STORE_PASSWORD
        Invoke-ExternalTool $keytool @('-genkeypair', '-keystore', $KeyStore, '-storetype', 'PKCS12',
            '-storepass:env', 'OME_IME_STORE_PASSWORD', '-keypass:env', 'OME_IME_KEY_PASSWORD',
            '-alias', 'ome-ime', '-dname', 'CN=Open Mobile Emulator', '-keyalg', 'RSA',
            '-keysize', '3072', '-validity', '10000', '-noprompt') 'Temporary signing key generation'
    }
    else {
        $KeyStore = (Resolve-Path -LiteralPath $KeyStore).Path
        if ([string]::IsNullOrEmpty($env:OME_IME_STORE_PASSWORD)) {
            throw 'Set OME_IME_STORE_PASSWORD for the supplied keystore.'
        }
        if ([string]::IsNullOrEmpty($env:OME_IME_KEY_PASSWORD)) {
            $env:OME_IME_KEY_PASSWORD = $env:OME_IME_STORE_PASSWORD
        }
    }
    $signed = Join-Path $intermediate 'signed.apk'
    Invoke-ExternalTool $apksigner @('sign', '--ks', $KeyStore, '--ks-key-alias', 'ome-ime',
        '--ks-pass', 'env:OME_IME_STORE_PASSWORD', '--key-pass', 'env:OME_IME_KEY_PASSWORD',
        '--v4-signing-enabled', 'false', '--out', $signed, $aligned) 'APK signing'
    Invoke-ExternalTool $apksigner @('verify', '--verbose', $signed) 'APK signature verification'
    Invoke-ExternalTool $zipalign @('-c', '-p', '4', $signed) 'APK alignment verification'
    $apk = Join-Path $out 'ome-ime.apk'
    Copy-Item -LiteralPath $signed -Destination $apk -Force
    $hash = (Get-FileHash -LiteralPath $apk -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$apk.sha256" -Encoding utf8NoBOM -Value "$hash  ome-ime.apk"
    [ordered]@{ versionCode = $versionCode; versionName = $versionName; sha256 = $hash } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'ome-ime.json') -Encoding utf8NoBOM
    Write-Output "OUTPUT $apk"
    Write-Output "SHA256 $hash"
}
finally {
    if ($temporaryKey -and [System.IO.File]::Exists($temporaryKey)) {
        [System.IO.File]::Delete($temporaryKey)
    }
    if ($intermediate -and [System.IO.Directory]::Exists($intermediate)) {
        [System.IO.Directory]::Delete($intermediate, $true)
    }
    $env:OME_IME_STORE_PASSWORD = $oldStorePassword
    $env:OME_IME_KEY_PASSWORD = $oldKeyPassword
    $env:JAVA_HOME = $oldJavaHome
}
