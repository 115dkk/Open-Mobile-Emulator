# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Collects M0 guest compatibility and performance evidence.
.DESCRIPTION
Waits for adb boot, optionally installs one or more APK splits, launches the
selected package, and captures bridge properties, CPU details, a binary-safe
screenshot, Android diagnostics, bridge-related log errors, and a host sample.
Every sample records its own failure and later samples still run.
.PARAMETER Name
Guest name used in the evidence summary.
.PARAMETER Serial
adb serial for the guest.
.PARAMETER Apk
One or more local APK paths. Multiple paths use adb install-multiple.
.PARAMETER Package
Android package to launch and inspect. Defaults to the compatibility record.
.PARAMETER OutDir
Evidence output directory.
.PARAMETER BootTimeoutSec
Maximum time to wait for Android boot completion.
.PARAMETER SkipInstall
Does not run adb install even when Apk paths are supplied.
.PARAMETER SkipLaunch
Does not launch the package; samples whatever is already running. Use this to
sample a session in progress: relaunching a running game through monkey ended
the game process on 2026-09-26 (docs/evidence/M0/findings-20260926.md).
.PARAMETER SettleSec
Seconds to wait after launching the app before sampling.
.EXAMPLE
./Invoke-GuestTest.ps1 -SkipInstall
.EXAMPLE
./Invoke-GuestTest.ps1 -Apk base.apk,split_config.arm64_v8a.apk -SettleSec 90
#>
[CmdletBinding()]
param(
    [string]$Name = 'default',
    [string]$Serial = '127.0.0.1:5555',
    [string[]]$Apk,
    [string]$Package,
    [string]$OutDir,

    [ValidateRange(1, 86400)]
    [int]$BootTimeoutSec = 600,

    [switch]$SkipInstall,
    [switch]$SkipLaunch,

    [ValidateRange(0, 86400)]
    [int]$SettleSec = 90
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force

$repoRoot = Get-OmeRepoRoot
$compatPath = [IO.Path]::Combine($repoRoot, 'compat', 'com.epidgames.trickcalrevive.json')
if ([string]::IsNullOrWhiteSpace($Package)) {
    $compat = Get-Content -LiteralPath $compatPath -Raw | ConvertFrom-Json
    $Package = [string]$compat.package
}
if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = [IO.Path]::Combine($repoRoot, 'docs', 'evidence', 'M0', "run-$(Get-Date -Format 'yyyyMMdd-HHmmss')")
}
$OutDir = [IO.Path]::GetFullPath($OutDir)
[void][IO.Directory]::CreateDirectory($OutDir)
$adb = Find-OmeAdb
if ($null -eq $adb) {
    throw 'adb.exe was not found. Install Android SDK platform-tools or set OME_ADB.'
}

$results = [Collections.Generic.List[object]]::new()

function Add-TestResult {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Step,

        [Parameter(Mandatory)]
        [ValidateSet('Pass', 'Fail', 'Skipped')]
        [string]$Status,

        [Parameter(Mandatory)]
        [string]$File,

        [string]$Details = ''
    )

    $results.Add([pscustomobject]@{ Step = $Step; Status = $Status; File = $File; Details = $Details })
}

function Invoke-AdbText {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string[]]$Arguments,

        [ValidateRange(1, 3600000)]
        [int]$TimeoutMs = 120000
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $adb
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    try {
        if (-not $process.Start()) {
            throw 'adb did not start.'
        }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMs)) {
            $process.Kill($true)
            throw "adb timed out after $TimeoutMs ms."
        }
        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            StdOut = $stdoutTask.GetAwaiter().GetResult()
            StdErr = $stderrTask.GetAwaiter().GetResult()
        }
    }
    finally {
        $process.Dispose()
    }
}

function Save-TextSample {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Step,

        [Parameter(Mandatory)]
        [string]$FileName,

        [Parameter(Mandatory)]
        [scriptblock]$Action
    )

    $path = [IO.Path]::Combine($OutDir, $FileName)
    try {
        $content = & $Action
        Set-Content -LiteralPath $path -Value ([string]$content) -Encoding utf8
        Add-TestResult -Step $Step -Status Pass -File $FileName
    }
    catch {
        Set-Content -LiteralPath $path -Value "ERROR: $($_.Exception.Message)" -Encoding utf8
        Add-TestResult -Step $Step -Status Fail -File $FileName -Details $_.Exception.Message
    }
}

Save-TextSample -Step 'adb boot' -FileName 'boot.txt' -Action {
    [void](Wait-OmeAdbBoot -Serial $Serial -TimeoutSec $BootTimeoutSec)
    "sys.boot_completed=1`nserial=$Serial"
}

Save-TextSample -Step 'native bridge properties' -FileName 'bridge-props.txt' -Action {
    $properties = @(
        'ro.dalvik.vm.native.bridge',
        'ro.product.cpu.abilist',
        'ro.product.cpu.abilist64',
        'ro.enable.native.bridge.exec',
        'ro.build.version.release',
        'ro.build.version.sdk',
        'ro.product.model'
    )
    $lines = [Collections.Generic.List[string]]::new()
    foreach ($property in $properties) {
        $response = Invoke-AdbText -Arguments @('-s', $Serial, 'shell', 'getprop', $property)
        if ($response.ExitCode -ne 0) {
            throw "getprop $property failed: $($response.StdErr.Trim())"
        }
        $lines.Add("$property=$($response.StdOut.Trim())")
    }
    $lines -join "`n"
}

Save-TextSample -Step 'guest CPU flags' -FileName 'cpuinfo.txt' -Action {
    $response = Invoke-AdbText -Arguments @('-s', $Serial, 'shell', 'cat', '/proc/cpuinfo')
    if ($response.ExitCode -ne 0) {
        throw "cat /proc/cpuinfo failed: $($response.StdErr.Trim())"
    }
    $hasSse42 = [regex]::IsMatch($response.StdOut, '(?im)^flags\s*:.*\bsse4_2\b')
    $hasPopcnt = [regex]::IsMatch($response.StdOut, '(?im)^flags\s*:.*\bpopcnt\b')
    "$($response.StdOut.TrimEnd())`n`nOME flag check: sse4_2=$($hasSse42.ToString().ToLowerInvariant()); popcnt=$($hasPopcnt.ToString().ToLowerInvariant())"
}

$installFile = 'install.txt'
$installPath = [IO.Path]::Combine($OutDir, $installFile)
if ($SkipInstall) {
    Set-Content -LiteralPath $installPath -Value 'Skipped by -SkipInstall.' -Encoding utf8
    Add-TestResult -Step 'APK install' -Status Skipped -File $installFile -Details 'SkipInstall selected'
}
elseif ($null -eq $Apk -or $Apk.Count -eq 0) {
    Set-Content -LiteralPath $installPath -Value 'No APK paths were supplied.' -Encoding utf8
    Add-TestResult -Step 'APK install' -Status Skipped -File $installFile -Details 'No APK paths supplied'
}
else {
    try {
        $apkPaths = @($Apk | ForEach-Object {
            $fullPath = [IO.Path]::GetFullPath($_)
            if (-not [IO.File]::Exists($fullPath)) {
                throw "APK does not exist: $fullPath"
            }
            $fullPath
        })
        $installArguments = [Collections.Generic.List[string]]::new()
        $installArguments.Add('-s')
        $installArguments.Add($Serial)
        if ($apkPaths.Count -gt 1) {
            $installArguments.Add('install-multiple')
        }
        else {
            $installArguments.Add('install')
        }
        $installArguments.Add('-r')
        foreach ($apkPath in $apkPaths) {
            $installArguments.Add($apkPath)
        }
        $response = Invoke-AdbText -Arguments $installArguments.ToArray() -TimeoutMs 600000
        $text = "Exit code: $($response.ExitCode)`nSTDOUT:`n$($response.StdOut)`nSTDERR:`n$($response.StdErr)"
        Set-Content -LiteralPath $installPath -Value $text -Encoding utf8
        if ($response.ExitCode -ne 0) {
            throw "adb install failed with exit code $($response.ExitCode)."
        }
        Add-TestResult -Step 'APK install' -Status Pass -File $installFile
    }
    catch {
        if (-not [IO.File]::Exists($installPath)) {
            Set-Content -LiteralPath $installPath -Value "ERROR: $($_.Exception.Message)" -Encoding utf8
        }
        Add-TestResult -Step 'APK install' -Status Fail -File $installFile -Details $_.Exception.Message
    }
}

if ($SkipLaunch) {
    Set-Content -LiteralPath ([IO.Path]::Combine($OutDir, 'launch.txt')) -Value 'Skipped by -SkipLaunch.' -Encoding utf8
    Add-TestResult -Step 'app launch' -Status Skipped -File 'launch.txt' -Details 'SkipLaunch selected'
}
else {
    Save-TextSample -Step 'app launch' -FileName 'launch.txt' -Action {
        $response = Invoke-AdbText -Arguments @('-s', $Serial, 'shell', 'monkey', '-p', $Package, '-c', 'android.intent.category.LAUNCHER', '1')
        $text = "Exit code: $($response.ExitCode)`nSTDOUT:`n$($response.StdOut)`nSTDERR:`n$($response.StdErr)"
        if ($response.ExitCode -ne 0) {
            throw "monkey failed with exit code $($response.ExitCode): $($response.StdErr.Trim())"
        }
        $text
    }
}

if ($SettleSec -gt 0) {
    Start-Sleep -Seconds $SettleSec
}

$screenshotName = 'screenshot-1.png'
$screenshotPath = [IO.Path]::Combine($OutDir, $screenshotName)
try {
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $adb
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in @('-s', $Serial, 'exec-out', 'screencap', '-p')) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    try {
        if (-not $process.Start()) {
            throw 'adb screencap did not start.'
        }
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $fileStream = [IO.File]::Open($screenshotPath, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::None)
        try {
            $process.StandardOutput.BaseStream.CopyTo($fileStream)
        }
        finally {
            $fileStream.Dispose()
        }
        if (-not $process.WaitForExit(120000)) {
            $process.Kill($true)
            throw 'adb screencap timed out.'
        }
        $stderr = $stderrTask.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) {
            throw "adb screencap failed: $stderr"
        }
        if ((Get-Item -LiteralPath $screenshotPath).Length -eq 0) {
            throw 'adb screencap returned an empty file.'
        }
    }
    finally {
        $process.Dispose()
    }
    Add-TestResult -Step 'screenshot' -Status Pass -File $screenshotName
}
catch {
    if ([IO.File]::Exists($screenshotPath)) {
        Remove-Item -LiteralPath $screenshotPath -Force
    }
    Set-Content -LiteralPath ([IO.Path]::Combine($OutDir, 'screenshot-error.txt')) -Value "ERROR: $($_.Exception.Message)" -Encoding utf8
    Add-TestResult -Step 'screenshot' -Status Fail -File 'screenshot-error.txt' -Details $_.Exception.Message
}

Save-TextSample -Step 'top sample' -FileName 'top.txt' -Action {
    $response = Invoke-AdbText -Arguments @('-s', $Serial, 'shell', 'top', '-m', '10', '-n', '3', '-b') -TimeoutMs 180000
    if ($response.ExitCode -ne 0) {
        throw "top failed: $($response.StdErr.Trim())"
    }
    $response.StdOut
}

Save-TextSample -Step 'gfxinfo sample' -FileName 'gfxinfo.txt' -Action {
    $response = Invoke-AdbText -Arguments @('-s', $Serial, 'shell', 'dumpsys', 'gfxinfo', $Package) -TimeoutMs 180000
    if ($response.ExitCode -ne 0) {
        throw "dumpsys gfxinfo failed: $($response.StdErr.Trim())"
    }
    $response.StdOut
}

Save-TextSample -Step 'meminfo sample' -FileName 'meminfo.txt' -Action {
    $response = Invoke-AdbText -Arguments @('-s', $Serial, 'shell', 'dumpsys', 'meminfo', $Package) -TimeoutMs 180000
    if ($response.ExitCode -ne 0) {
        throw "dumpsys meminfo failed: $($response.StdErr.Trim())"
    }
    $response.StdOut
}

$logcatText = $null
Save-TextSample -Step 'logcat tail' -FileName 'logcat-tail.txt' -Action {
    $response = Invoke-AdbText -Arguments @('-s', $Serial, 'logcat', '-d', '-t', '2000') -TimeoutMs 180000
    if ($response.ExitCode -ne 0) {
        throw "logcat failed: $($response.StdErr.Trim())"
    }
    $script:logcatText = $response.StdOut
    $response.StdOut
}

Save-TextSample -Step 'native bridge error scan' -FileName 'logcat-bridge-errors.txt' -Action {
    if ($null -eq $script:logcatText) {
        $logPath = [IO.Path]::Combine($OutDir, 'logcat-tail.txt')
        if (-not [IO.File]::Exists($logPath)) {
            throw 'logcat-tail.txt is unavailable.'
        }
        $script:logcatText = Get-Content -LiteralPath $logPath -Raw
    }
    $matchingLines = @($script:logcatText -split "`r?`n" | Where-Object { $_ -match '(?i)dlopen failed|SIGILL|ndk_translation|native bridge' })
    if ($matchingLines.Count -eq 0) {
        'No matching bridge error lines found.'
    }
    else {
        $matchingLines -join "`n"
    }
}

Save-TextSample -Step 'host CPU and memory sample' -FileName 'host-sample.txt' -Action {
    $counter = Get-Counter '\Processor(_Total)\% Processor Time' -SampleInterval 1 -MaxSamples 3
    $cpuValues = @($counter.CounterSamples | ForEach-Object { [Math]::Round($_.CookedValue, 2) })
    $operatingSystem = Get-CimInstance -ClassName Win32_OperatingSystem
    $totalGb = [Math]::Round([double]$operatingSystem.TotalVisibleMemorySize / 1MB, 2)
    $freeGb = [Math]::Round([double]$operatingSystem.FreePhysicalMemory / 1MB, 2)
    @(
        "CPU counter: \\Processor(_Total)\\% Processor Time",
        "CPU samples percent: $($cpuValues -join ', ')",
        "CPU average percent: $([Math]::Round(($cpuValues | Measure-Object -Average).Average, 2))",
        "Physical memory total GB: $totalGb",
        "Physical memory free GB: $freeGb",
        "Physical memory used GB: $([Math]::Round($totalGb - $freeGb, 2))"
    ) -join "`n"
}

$summaryPath = [IO.Path]::Combine($OutDir, 'summary.md')
$summaryLines = [Collections.Generic.List[string]]::new()
$summaryLines.Add('# Guest test summary')
$summaryLines.Add('')
$summaryLines.Add("- Guest: ``$Name``")
$summaryLines.Add("- Serial: ``$Serial``")
$summaryLines.Add("- Package: ``$Package``")
$summaryLines.Add("- Recorded: $([DateTimeOffset]::Now.ToString('o'))")
$summaryLines.Add('')
$summaryLines.Add('| Step | Status | Evidence | Details |')
$summaryLines.Add('|---|---|---|---|')
foreach ($result in $results) {
    $details = ([string]$result.Details).Replace('|', '\|').Replace("`r", ' ').Replace("`n", ' ')
    $summaryLines.Add("| $($result.Step) | $($result.Status) | ``$($result.File)`` | $details |")
}
Set-Content -LiteralPath $summaryPath -Value ($summaryLines -join "`n") -Encoding utf8
Write-Host "Evidence written to: $OutDir"
Write-Host "Passed: $(@($results | Where-Object Status -ceq 'Pass').Count); Failed: $(@($results | Where-Object Status -ceq 'Fail').Count); Skipped: $(@($results | Where-Object Status -ceq 'Skipped').Count)"
