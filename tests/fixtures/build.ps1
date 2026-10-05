# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Builds the self-made arm64 translator smoke-test fixtures.

.DESCRIPTION
Discovers the newest installed Android platform, build-tools, and NDK from
ANDROID_SDK_ROOT, ANDROID_HOME, or LOCALAPPDATA. Builds a standalone arm64
executable and an arm64-only APK, verifies the APK, and prints output hashes.

.PARAMETER Only
Builds only the hello executable or only the APK.

.PARAMETER Clean
Removes the generated tests/fixtures/build directory before building.
#>
[CmdletBinding()]
param(
    [string]$Only,
    [switch]$Clean
)

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
    $candidates = [System.Collections.Generic.List[string]]::new()
    foreach ($environmentName in @('ANDROID_SDK_ROOT', 'ANDROID_HOME')) {
        $value = [System.Environment]::GetEnvironmentVariable($environmentName)
        if (-not [string]::IsNullOrWhiteSpace($value)) {
            [void]$candidates.Add($value)
        }
    }
    if (-not [string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        [void]$candidates.Add((Join-Path $env:LOCALAPPDATA 'Android/Sdk'))
    }

    foreach ($candidate in $candidates) {
        $fullPath = [System.IO.Path]::GetFullPath($candidate)
        if (
            [System.IO.Directory]::Exists((Join-Path $fullPath 'ndk')) -and
            [System.IO.Directory]::Exists((Join-Path $fullPath 'build-tools')) -and
            [System.IO.Directory]::Exists((Join-Path $fullPath 'platforms'))
        ) {
            return $fullPath
        }
    }
    throw 'Android SDK not found. Set ANDROID_SDK_ROOT or ANDROID_HOME, or install it under LOCALAPPDATA/Android/Sdk.'
}

function Get-NewestVersionDirectory {
    param([Parameter(Mandatory)][string]$Parent)

    $versions = @(
        Get-ChildItem -LiteralPath $Parent -Directory |
            ForEach-Object {
                $parsedVersion = $null
                if ([System.Version]::TryParse($_.Name, [ref]$parsedVersion)) {
                    [pscustomobject]@{ Directory = $_.FullName; Version = $parsedVersion }
                }
            } |
            Sort-Object Version -Descending
    )
    if ($versions.Count -eq 0) {
        throw "No versioned directories found under $Parent."
    }
    return $versions[0].Directory
}

function Get-NewestPlatform {
    param([Parameter(Mandatory)][string]$PlatformsRoot)

    $platforms = @(
        Get-ChildItem -LiteralPath $PlatformsRoot -Directory |
            Where-Object { $_.Name -match '^android-(\d+)$' } |
            ForEach-Object {
                [pscustomobject]@{ Api = [int]$Matches[1]; Directory = $_.FullName }
            } |
            Sort-Object Api -Descending
    )
    if ($platforms.Count -eq 0) {
        throw "No Android platforms found under $PlatformsRoot."
    }
    return $platforms[0]
}

function Add-ZipEntry {
    param(
        [Parameter(Mandatory)][System.IO.Compression.ZipArchive]$Archive,
        [Parameter(Mandatory)][string]$SourcePath,
        [Parameter(Mandatory)][string]$EntryName
    )

    $existing = $Archive.GetEntry($EntryName)
    if ($null -ne $existing) {
        $existing.Delete()
    }
    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
        $Archive,
        $SourcePath,
        $EntryName,
        [System.IO.Compression.CompressionLevel]::Optimal
    )
}

function Write-ArtifactHash {
    param([Parameter(Mandatory)][string]$ArtifactPath)

    $item = Get-Item -LiteralPath $ArtifactPath
    $hash = (Get-FileHash -LiteralPath $ArtifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Output "OUTPUT $($item.FullName)"
    Write-Output "SIZE $($item.Length)"
    Write-Output "SHA256 $hash"
}

try {
    if (
        -not [string]::IsNullOrWhiteSpace($Only) -and
        $Only -cnotin @('hello', 'apk')
    ) {
        throw "Invalid -Only value '$Only'. Expected hello or apk."
    }

    $fixtureRoot = $PSScriptRoot
    $buildRoot = Join-Path $fixtureRoot 'build'
    if ($Clean -and [System.IO.Directory]::Exists($buildRoot)) {
        Remove-Item -LiteralPath $buildRoot -Recurse -Force
    }
    [void][System.IO.Directory]::CreateDirectory($buildRoot)

    $sdkRoot = Find-AndroidSdk
    $ndkRoot = Get-NewestVersionDirectory (Join-Path $sdkRoot 'ndk')
    $buildToolsRoot = Get-NewestVersionDirectory (Join-Path $sdkRoot 'build-tools')
    $platform = Get-NewestPlatform (Join-Path $sdkRoot 'platforms')
    $androidJar = Join-Path $platform.Directory 'android.jar'

    $clang = Join-Path $ndkRoot 'toolchains/llvm/prebuilt/windows-x86_64/bin/clang.exe'
    $arm64Api21Clang = Join-Path $ndkRoot 'toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android21-clang.cmd'
    $arm64Api26Clang = Join-Path $ndkRoot 'toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android26-clang.cmd'
    $aapt2 = Join-Path $buildToolsRoot 'aapt2.exe'
    $d8 = Join-Path $buildToolsRoot 'd8.bat'
    $zipalign = Join-Path $buildToolsRoot 'zipalign.exe'
    $apksigner = Join-Path $buildToolsRoot 'apksigner.bat'
    foreach ($requiredTool in @(
            $clang,
            $arm64Api21Clang,
            $arm64Api26Clang,
            $aapt2,
            $d8,
            $zipalign,
            $apksigner,
            $androidJar
        )) {
        if (-not [System.IO.File]::Exists($requiredTool)) {
            throw "Required Android tool is missing: $requiredTool"
        }
    }

    $javac = (Get-Command javac -ErrorAction Stop).Source
    $keytool = (Get-Command keytool -ErrorAction Stop).Source
    Write-Output "SDK $sdkRoot"
    Write-Output "NDK $ndkRoot"
    Write-Output "BUILD_TOOLS $buildToolsRoot"
    Write-Output "PLATFORM android-$($platform.Api)"

    if ([string]::IsNullOrWhiteSpace($Only) -or $Only -eq 'hello') {
        $helloSource = Join-Path $fixtureRoot 'hello_arm64/hello.c'
        $helloOutput = Join-Path $buildRoot 'hello_arm64'
        & $clang @(
            '--target=aarch64-linux-android21',
            '-static',
            '-O2',
            '-o',
            $helloOutput,
            $helloSource
        )
        if ($LASTEXITCODE -eq 0) {
            Write-Output 'HELLO_LINKAGE static'
        }
        else {
            Write-Warning 'Static arm64 executable link failed; building a dynamic PIE executable instead.'
            Invoke-ExternalTool $arm64Api21Clang @(
                '-fPIE',
                '-pie',
                '-O2',
                '-o',
                $helloOutput,
                $helloSource
            ) 'Dynamic arm64 hello build'
            Write-Output 'HELLO_LINKAGE dynamic-pie'
        }
        Write-ArtifactHash $helloOutput
    }

    if ([string]::IsNullOrWhiteSpace($Only) -or $Only -eq 'apk') {
        $probeRoot = Join-Path $fixtureRoot 'arm64-probe'
        $intermediateRoot = Join-Path $buildRoot 'arm64-probe-intermediate'
        if ([System.IO.Directory]::Exists($intermediateRoot)) {
            Remove-Item -LiteralPath $intermediateRoot -Recurse -Force
        }
        $classesRoot = Join-Path $intermediateRoot 'classes'
        $dexRoot = Join-Path $intermediateRoot 'dex'
        $nativeRoot = Join-Path $intermediateRoot 'lib/arm64-v8a'
        foreach ($directory in @($intermediateRoot, $classesRoot, $dexRoot, $nativeRoot)) {
            [void][System.IO.Directory]::CreateDirectory($directory)
        }

        $nativeLibrary = Join-Path $nativeRoot 'libprobe.so'
        Invoke-ExternalTool $arm64Api26Clang @(
            '-shared',
            '-landroid',
            '-ldl',
            '-fPIC',
            '-O2',
            '-Wl,-soname,libprobe.so',
            '-o',
            $nativeLibrary,
            (Join-Path $probeRoot 'jni/probe.c')
        ) 'arm64 JNI library build'

        $secondLibrary = Join-Path $nativeRoot 'libprobe_second.so'
        Invoke-ExternalTool $arm64Api26Clang @(
            '-shared', '-fPIC', '-O2', '-Wl,-soname,libprobe_second.so',
            '-o', $secondLibrary, (Join-Path $probeRoot 'jni/probe_second.c')
        ) 'arm64 dlopen JNI library build'

        $javaSource = Join-Path $probeRoot 'java/org/ome/arm64probe/MainActivity.java'
        Invoke-ExternalTool $javac @(
            '-encoding',
            'UTF-8',
            '-classpath',
            $androidJar,
            '-source',
            '11',
            '-target',
            '11',
            '-d',
            $classesRoot,
            $javaSource
        ) 'Java compilation'

        $classFile = Join-Path $classesRoot 'org/ome/arm64probe/MainActivity.class'
        Invoke-ExternalTool $d8 @(
            '--release',
            '--min-api',
            '26',
            '--lib',
            $androidJar,
            '--output',
            $dexRoot,
            $classFile
        ) 'DEX compilation'

        $compiledResources = Join-Path $intermediateRoot 'resources.zip'
        Invoke-ExternalTool $aapt2 @(
            'compile',
            '--dir',
            (Join-Path $probeRoot 'res'),
            '-o',
            $compiledResources
        ) 'Android resource compilation'

        $unsignedApk = Join-Path $intermediateRoot 'arm64-probe-unsigned.apk'
        Invoke-ExternalTool $aapt2 @(
            'link',
            '-o',
            $unsignedApk,
            '--manifest',
            (Join-Path $probeRoot 'AndroidManifest.xml'),
            '-I',
            $androidJar,
            '--min-sdk-version',
            '26',
            '--target-sdk-version',
            $platform.Api.ToString(),
            '--version-code',
            '1',
            '--version-name',
            '1.0',
            $compiledResources
        ) 'APK resource link'

        $archive = [System.IO.Compression.ZipFile]::Open(
            $unsignedApk,
            [System.IO.Compression.ZipArchiveMode]::Update
        )
        try {
            Add-ZipEntry $archive (Join-Path $dexRoot 'classes.dex') 'classes.dex'
            Add-ZipEntry $archive $nativeLibrary 'lib/arm64-v8a/libprobe.so'
            Add-ZipEntry $archive $secondLibrary 'lib/arm64-v8a/libprobe_second.so'
            $fixedTimestamp = [System.DateTimeOffset]::new(
                1980,
                1,
                1,
                0,
                0,
                0,
                [System.TimeSpan]::Zero
            )
            foreach ($entry in $archive.Entries) {
                $entry.LastWriteTime = $fixedTimestamp
            }
        }
        finally {
            $archive.Dispose()
        }

        $alignedApk = Join-Path $intermediateRoot 'arm64-probe-aligned.apk'
        Invoke-ExternalTool $zipalign @(
            '-f',
            '-p',
            '4',
            $unsignedApk,
            $alignedApk
        ) 'APK alignment'

        $keystore = Join-Path $buildRoot 'debug.keystore'
        if (-not [System.IO.File]::Exists($keystore)) {
            Invoke-ExternalTool $keytool @(
                '-genkeypair',
                '-keystore',
                $keystore,
                '-storepass',
                'android',
                '-alias',
                'androiddebugkey',
                '-keypass',
                'android',
                '-dname',
                'CN=Android Debug,O=Android,C=US',
                '-keyalg',
                'RSA',
                '-keysize',
                '2048',
                '-validity',
                '10000',
                '-noprompt'
            ) 'Debug keystore generation'
        }

        $apkOutput = Join-Path $buildRoot 'arm64-probe.apk'
        if ([System.IO.File]::Exists($apkOutput)) {
            Remove-Item -LiteralPath $apkOutput -Force
        }
        Invoke-ExternalTool $apksigner @(
            'sign',
            '--ks',
            $keystore,
            '--ks-key-alias',
            'androiddebugkey',
            '--ks-pass',
            'pass:android',
            '--key-pass',
            'pass:android',
            '--out',
            $apkOutput,
            $alignedApk
        ) 'APK signing'

        Write-Output 'APKSIGNER_VERIFY_BEGIN'
        Invoke-ExternalTool $apksigner @(
            'verify',
            '--verbose',
            '--print-certs',
            $apkOutput
        ) 'APK signature verification'
        Write-Output 'APKSIGNER_VERIFY_END'

        $signedArchive = [System.IO.Compression.ZipFile]::OpenRead($apkOutput)
        try {
            $nativeEntries = @(
                $signedArchive.Entries |
                    Where-Object { $_.FullName.StartsWith('lib/', [System.StringComparison]::Ordinal) } |
                    ForEach-Object { $_.FullName }
            )
        }
        finally {
            $signedArchive.Dispose()
        }
        if ($nativeEntries.Count -eq 0) {
            throw 'APK contains no native libraries.'
        }
        foreach ($entry in $nativeEntries) {
            if ($entry -notmatch '^lib/arm64-v8a/[^/]+$') {
                throw "APK contains a non-arm64-v8a native-library entry: $entry"
            }
        }
        Write-Output 'APK_LIBS_BEGIN'
        foreach ($entry in $nativeEntries) {
            Write-Output $entry
        }
        Write-Output 'APK_LIBS_END'
        Write-ArtifactHash $apkOutput
    }

    exit 0
}
catch {
    [System.Console]::Error.WriteLine("ERROR fixture build: $($_.Exception.Message)")
    exit 2
}
