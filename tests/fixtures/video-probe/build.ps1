# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0
<#
.SYNOPSIS
Builds the Java-only local video diagnostic with a temporary signing key.
.PARAMETER OutputDirectory
Build output directory. Defaults to the ignored out directory beside this script.
#>
[CmdletBinding()]
param([string]$OutputDirectory = (Join-Path $PSScriptRoot 'out'))
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-ExternalTool {
    param([string]$FilePath, [string[]]$ArgumentList, [string]$Description)
    & $FilePath @ArgumentList
    if ($LASTEXITCODE -ne 0) { throw "$Description failed with exit code $LASTEXITCODE." }
}

$work = $null
$oldPassword = $env:OME_VIDEO_KEY_PASSWORD
$oldJavaHome = $env:JAVA_HOME
try {
    $sdk = @($env:ANDROID_HOME, $env:ANDROID_SDK_ROOT,
        (Join-Path $env:LOCALAPPDATA 'Android/Sdk')) | Where-Object {
        $_ -and (Test-Path -LiteralPath (Join-Path $_ 'build-tools')) -and
        (Test-Path -LiteralPath (Join-Path $_ 'platforms'))
    } | Select-Object -First 1
    if (-not $sdk) { throw 'Set ANDROID_HOME or ANDROID_SDK_ROOT to a complete SDK.' }
    $tools = @(Get-ChildItem (Join-Path $sdk 'build-tools') -Directory | ForEach-Object {
        $version = $null
        if ([version]::TryParse($_.Name, [ref]$version)) {
            [pscustomobject]@{ Path = $_.FullName; Version = $version }
        }
    } | Sort-Object Version -Descending)[0].Path
    $platform = @(Get-ChildItem (Join-Path $sdk 'platforms') -Directory | Where-Object {
        $_.Name -match '^android-\d+$'
    } | Sort-Object { [int]($_.Name -replace 'android-', '') } -Descending)[0]
    $jar = Join-Path $platform.FullName 'android.jar'
    $javac = if ($env:JAVA_HOME) { Join-Path $env:JAVA_HOME 'bin/javac.exe' } else { (Get-Command javac).Source }
    $keytool = Join-Path (Split-Path $javac) 'keytool.exe'
    $env:JAVA_HOME = Split-Path (Split-Path $javac)
    if (-not (Test-Path (Join-Path $PSScriptRoot 'res/raw/probe.mp4'))) { throw 'Generate res/raw/probe.mp4 first.' }
    [void][IO.Directory]::CreateDirectory($OutputDirectory)
    $work = Join-Path $OutputDirectory ('work-' + [guid]::NewGuid().ToString('N'))
    foreach ($name in @('', 'java', 'classes', 'dex')) { [void][IO.Directory]::CreateDirectory((Join-Path $work $name)) }
    $aapt = Join-Path $tools 'aapt2.exe'
    Invoke-ExternalTool $aapt @('compile', '--dir', (Join-Path $PSScriptRoot 'res'), '-o', "$work/resources.zip") 'Resource compilation'
    Invoke-ExternalTool $aapt @('link', '-o', "$work/unsigned.apk", '--manifest',
        (Join-Path $PSScriptRoot 'AndroidManifest.xml'), '-I', $jar, '--java', "$work/java",
        '--min-sdk-version', '26', '--target-sdk-version', '35', '--version-code', '1',
        '--version-name', '1.0', '-0', 'mp4', "$work/resources.zip") 'Resource link'
    $sources = @(Get-ChildItem (Join-Path $PSScriptRoot 'java'), "$work/java" -Recurse -Filter '*.java' | ForEach-Object FullName)
    Invoke-ExternalTool $javac (@('--release', '11', '-encoding', 'UTF-8', '-classpath', $jar,
        '-d', "$work/classes") + $sources) 'Java compilation'
    $classes = @(Get-ChildItem "$work/classes" -Recurse -Filter '*.class' | ForEach-Object FullName)
    Invoke-ExternalTool (Join-Path $tools 'd8.bat') (@('--release', '--min-api', '26', '--lib', $jar,
        '--output', "$work/dex") + $classes) 'DEX compilation'
    $archive = [IO.Compression.ZipFile]::Open("$work/unsigned.apk", [IO.Compression.ZipArchiveMode]::Update)
    try {
        [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, "$work/dex/classes.dex", 'classes.dex')
    } finally { $archive.Dispose() }
    Invoke-ExternalTool (Join-Path $tools 'zipalign.exe') @('-p', '4', "$work/unsigned.apk", "$work/aligned.apk") 'APK alignment'
    $env:OME_VIDEO_KEY_PASSWORD = [Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
    Invoke-ExternalTool $keytool @('-genkeypair', '-keystore', "$work/key.p12", '-storetype', 'PKCS12',
        '-storepass:env', 'OME_VIDEO_KEY_PASSWORD', '-keypass:env', 'OME_VIDEO_KEY_PASSWORD',
        '-alias', 'ome-video', '-dname', 'CN=OME Video Probe', '-keyalg', 'RSA', '-keysize', '2048',
        '-validity', '30', '-noprompt') 'Temporary key generation'
    $apk = Join-Path $OutputDirectory 'video-probe.apk'
    Invoke-ExternalTool (Join-Path $tools 'apksigner.bat') @('sign', '--ks', "$work/key.p12",
        '--ks-key-alias', 'ome-video', '--ks-pass', 'env:OME_VIDEO_KEY_PASSWORD', '--v4-signing-enabled',
        'false', '--out', $apk, "$work/aligned.apk") 'APK signing'
    Invoke-ExternalTool (Join-Path $tools 'apksigner.bat') @('verify', '--verbose', $apk) 'Signature verification'
    Invoke-ExternalTool (Join-Path $tools 'zipalign.exe') @('-c', '-p', '4', $apk) 'Alignment verification'
    Write-Output "OUTPUT $apk"
    Write-Output "SHA256 $((Get-FileHash $apk -Algorithm SHA256).Hash.ToLowerInvariant())"
}
finally {
    if ($work -and [IO.Directory]::Exists($work)) { [IO.Directory]::Delete($work, $true) }
    $env:OME_VIDEO_KEY_PASSWORD = $oldPassword
    $env:JAVA_HOME = $oldJavaHome
}
