# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Measures QMP multitouch delivery on an already booted developer guest.
.DESCRIPTION
Discovers input nodes, captures getevent traces and screenshots, measures 20
warm QMP round trips, and disables pointer_location in finally. Does not start
or stop QEMU. Use the launcher before and after this script. The guest must be
unlocked on an idle home screen. Use adb root beforehand for getevent, then
adb unroot afterwards. No application is installed or launched. GL screendump
failures are recorded and adb fallback screenshots are explicitly labelled.
.PARAMETER Serial
adb serial of the already booted guest.
.PARAMETER QmpPort
Loopback QMP port.
.PARAMETER OutDir
Directory for PNG screenshots and TXT evidence only.
.EXAMPLE
./Invoke-OmeMultitouchSpike.ps1
#>
[CmdletBinding()]
param(
    [string]$Serial = '127.0.0.1:5555',
    [ValidateRange(1, 65535)]
    [int]$QmpPort = 4444,
    [string]$OutDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force
if ([string]::IsNullOrWhiteSpace($OutDir)) {
    $OutDir = [IO.Path]::Combine((Get-OmeRepoRoot), 'docs', 'evidence', 'M2', 'multitouch')
}
$OutDir = [IO.Path]::GetFullPath($OutDir)
[void][IO.Directory]::CreateDirectory($OutDir)
$adb = Find-OmeAdb
if ($null -eq $adb) { throw 'adb.exe was not found.' }
$captures = [Collections.Generic.List[object]]::new()
$activeSlots = @{}
$pointerEnabled = $false
$mouseDown = $false
$qmpScreenshot = $true

function Invoke-AdbText {
    param([string[]]$Arguments)
    $text = @(& $adb -s $Serial @Arguments 2>&1) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw "adb exit ${LASTEXITCODE}: $text" }
    return $text
}

function Send-Events {
    param([string]$Step, [object[]]$Events)
    $request = [ordered]@{ execute = 'input-send-event'; arguments = @{ events = $Events } }
    "$([DateTimeOffset]::Now.ToString('o')) $Step $(ConvertTo-Json -InputObject $request -Depth 20 -Compress)" |
        Add-Content -LiteralPath "$OutDir/commands.txt"
    $result = Invoke-OmeQmp -Command input-send-event -Arguments @{ events = $Events } -Port $QmpPort
    "return: $(ConvertTo-Json -InputObject $result -Compress)" | Add-Content -LiteralPath "$OutDir/commands.txt"
}

function Get-Touch {
    param([string]$Type, [int]$Slot, [int]$X = 8192, [int]$Y = 8192)
    ConvertTo-OmeQmpTouchEvent -Type $Type -Slot $Slot -TrackingId ($Slot + 1) -X $X -Y $Y
}

function Get-Button {
    param([string]$Button, [bool]$Down)
    [ordered]@{ type = 'btn'; data = [ordered]@{ button = $Button; down = $Down } }
}

function Start-Capture {
    param([string]$Name, [string]$Node)
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $adb
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in @('-s', $Serial, 'shell', 'timeout', '8', 'getevent', '-lt', $Node)) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    [void]$process.Start()
    $capture = [pscustomobject]@{
        Name = $Name; Process = $process
        StdOut = $process.StandardOutput.ReadToEndAsync()
        StdErr = $process.StandardError.ReadToEndAsync()
    }
    $captures.Add($capture)
}

function Complete-Captures {
    foreach ($capture in $captures) {
        try {
            if (-not $capture.Process.WaitForExit(12000)) { $capture.Process.Kill($true); $capture.Process.WaitForExit() }
            $capture.StdOut.GetAwaiter().GetResult() | Set-Content -LiteralPath "$OutDir/$($capture.Name).txt"
            $capture.StdErr.GetAwaiter().GetResult() | Set-Content -LiteralPath "$OutDir/$($capture.Name)-stderr.txt"
            "$($capture.Name): exit $($capture.Process.ExitCode) (timeout intentionally ends capture)" |
                Add-Content -LiteralPath "$OutDir/captures.txt"
        }
        finally { $capture.Process.Dispose() }
    }
    $captures.Clear()
}

function Save-Screen {
    param([string]$Name)
    if ($script:qmpScreenshot) {
        try {
            [void](Save-OmeQmpScreenshot -OutFile "$OutDir/$Name.png" -Port $QmpPort)
            "$Name.png: QMP screendump" | Add-Content -LiteralPath "$OutDir/screenshots.txt"
            return
        }
        catch {
            $_.Exception.Message | Add-Content -LiteralPath "$OutDir/screenshots.txt"
            $script:qmpScreenshot = $false
        }
    }
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $adb
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in @('-s', $Serial, 'exec-out', 'screencap', '-p')) { [void]$startInfo.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $file = [IO.File]::Create("$OutDir/$Name.png")
    try {
        [void]$process.Start()
        $copy = $process.StandardOutput.BaseStream.CopyToAsync($file)
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(10000)) { $process.Kill($true); throw 'adb screencap timed out.' }
        [void]$copy.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) { throw $stderr.GetAwaiter().GetResult() }
    }
    finally { $file.Dispose(); $process.Dispose() }
    "$Name.png: adb exec-out screencap -p (QMP unavailable)" | Add-Content -LiteralPath "$OutDir/screenshots.txt"
}

try {
    [void](Wait-OmeAdbBoot -Serial $Serial -TimeoutSec 30)
    $devices = Invoke-AdbText @('shell', 'getevent', '-pl')
    $devices | Set-Content -LiteralPath "$OutDir/getevent-devices.txt"
    $touchNode = $null
    $tabletNode = $null
    foreach ($block in [regex]::Split($devices, '(?m)(?=^add device )')) {
        if ($block -match '^add device \d+: (/dev/input/event\d+)') {
            $node = $Matches[1]
            if ($block.Contains('"QEMU Virtio MultiTouch"')) { $touchNode = $node }
            if ($block.Contains('"QEMU QEMU USB Tablet"')) { $tabletNode = $node }
        }
    }
    if ($null -eq $touchNode -or $null -eq $tabletNode) { throw 'Multitouch/tablet node not found; getevent may require adb root.' }
    "touch=$touchNode tablet=$tabletNode" | Set-Content -LiteralPath "$OutDir/nodes.txt"
    Invoke-AdbText @('shell', 'settings', 'get', 'system', 'pointer_location') | Set-Content -LiteralPath "$OutDir/pointer-before.txt"
    $pointerEnabled = $true
    [void](Invoke-AdbText @('shell', 'settings', 'put', 'system', 'pointer_location', '1'))
    Start-Sleep -Milliseconds 300
    Save-Screen 'baseline'

    Start-Capture 'single-getevent' $touchNode
    Start-Sleep -Milliseconds 300
    $activeSlots[0] = $true
    Send-Events 'single-begin' (@(Get-Touch begin 0) + @(Get-Button touch $true))
    Start-Sleep -Milliseconds 100
    Save-Screen 'single-down'
    Send-Events 'single-end' (@(Get-Touch end 0) + @(Get-Button touch $false))
    $activeSlots.Remove(0)
    Complete-Captures

    Start-Capture 'two-getevent' $touchNode
    Start-Sleep -Milliseconds 300
    $activeSlots[0] = $true
    $activeSlots[1] = $true
    Send-Events 'two-begin' (@(Get-Touch begin 0) + @(Get-Touch begin 1 24575 12288) + @(Get-Button touch $true))
    Start-Sleep -Milliseconds 100
    Save-Screen 'two-down'
    Send-Events 'two-update' @(Get-Touch update 0 11468 9830)
    Start-Sleep -Milliseconds 100
    Save-Screen 'two-drag'
    Send-Events 'two-end' (@(Get-Touch end 0) + @(Get-Touch end 1) + @(Get-Button touch $false))
    $activeSlots.Clear()
    Complete-Captures

    # Move away first so a repeated run still produces ABS_X/Y changes;
    # Linux input suppresses unchanged absolute values in getevent output.
    Send-Events 'mouse-prepare' @(
        [ordered]@{ type = 'abs'; data = [ordered]@{ axis = 'x'; value = 20480 } },
        [ordered]@{ type = 'abs'; data = [ordered]@{ axis = 'y'; value = 8192 } }
    )
    Start-Capture 'mouse-touch-getevent' $touchNode
    Start-Capture 'mouse-tablet-getevent' $tabletNode
    Start-Sleep -Milliseconds 300
    $activeSlots[0] = $true
    Send-Events 'mouse-touch-begin' (@(Get-Touch begin 0) + @(Get-Button touch $true))
    $mouseDown = $true
    Send-Events 'mouse-move-press' @(
        [ordered]@{ type = 'abs'; data = [ordered]@{ axis = 'x'; value = 16384 } },
        [ordered]@{ type = 'abs'; data = [ordered]@{ axis = 'y'; value = 12288 } },
        (Get-Button left $true)
    )
    Start-Sleep -Milliseconds 100
    Save-Screen 'mouse-down'
    Send-Events 'mouse-release' @(Get-Button left $false)
    $mouseDown = $false
    Send-Events 'mouse-touch-update' @(Get-Touch update 0 11468 9830)
    Start-Sleep -Milliseconds 100
    Save-Screen 'mouse-touch-after'
    Invoke-AdbText @('shell', 'dumpsys', 'input') | Set-Content -LiteralPath "$OutDir/input-mouse-held.txt"
    Send-Events 'mouse-touch-end' (@(Get-Touch end 0) + @(Get-Button touch $false))
    $activeSlots.Clear()
    Complete-Captures

    # Like Measure-OmeQmpLatency, reuse a negotiated connection and exclude
    # connection/capability setup. Reuse its private framing helpers in scope.
    $events = @(Get-Touch update 0 8192 8192)
    $activeSlots[0] = $true
    Send-Events 'latency-begin' (@(Get-Touch begin 0) + @(Get-Button touch $true))
    $samples = & (Get-Module OME.Common) {
        param($Port, $Events)
        $client = [Net.Sockets.TcpClient]::new()
        try {
            $task = $client.ConnectAsync('127.0.0.1', $Port)
            if (-not $task.Wait(5000)) { throw 'QMP connect timed out.' }
            [void]$task.GetAwaiter().GetResult()
            $stream = $client.GetStream()
            $stream.ReadTimeout = 5000
            $stream.WriteTimeout = 5000
            $encoding = [Text.UTF8Encoding]::new($false)
            $reader = [IO.StreamReader]::new($stream, $encoding, $false, 4096, $true)
            $writer = [IO.StreamWriter]::new($stream, $encoding, 4096, $true)
            $writer.NewLine = "`r`n"
            try {
                [void](Read-OmeQmpMessage -Reader $reader -Greeting)
                Send-OmeQmpObject -Writer $writer -Object @{ execute = 'qmp_capabilities'; id = 'caps' }
                [void](Read-OmeQmpMessage -Reader $reader -ExpectedId caps)
                for ($index = 0; $index -lt 20; $index++) {
                    $Events[1].data.value = 8192 + $index
                    $watch = [Diagnostics.Stopwatch]::StartNew()
                    Send-OmeQmpObject -Writer $writer -Object @{ execute = 'input-send-event'; id = "timing-$index"; arguments = @{ events = $Events } }
                    [void](Read-OmeQmpMessage -Reader $reader -ExpectedId "timing-$index")
                    $watch.Stop()
                    $watch.Elapsed.TotalMilliseconds
                }
            }
            finally { $writer.Dispose(); $reader.Dispose() }
        }
        finally { $client.Dispose() }
    } $QmpPort $events
    Send-Events 'latency-end' (@(Get-Touch end 0) + @(Get-Button touch $false))
    $activeSlots.Clear()
    $samples | Set-Content -LiteralPath "$OutDir/latency-samples-ms.txt"
    $sorted = @($samples | Sort-Object)
    "count=$($samples.Count) min_ms=$($sorted[0]) median_ms=$(($sorted[9] + $sorted[10]) / 2) max_ms=$($sorted[-1])" |
        Set-Content -LiteralPath "$OutDir/latency.txt"
    Invoke-AdbText @('shell', 'dumpsys', 'input') | Set-Content -LiteralPath "$OutDir/input-after.txt"
}
catch {
    $_ | Out-String | Add-Content -LiteralPath "$OutDir/errors.txt"
    throw
}
finally {
    try {
        $release = @()
        foreach ($slot in @($activeSlots.Keys)) { $release += @(Get-Touch cancel $slot) }
        if ($activeSlots.Count -gt 0) { $release += @(Get-Button touch $false) }
        if ($mouseDown) { $release += @(Get-Button left $false) }
        if ($release.Count -gt 0) { Send-Events 'cleanup-release' $release }
    }
    catch { $_.Exception.Message | Add-Content -LiteralPath "$OutDir/cleanup-errors.txt" }
    try {
        if ($pointerEnabled) {
            [void](Invoke-AdbText @('shell', 'settings', 'put', 'system', 'pointer_location', '0'))
            Invoke-AdbText @('shell', 'settings', 'get', 'system', 'pointer_location') | Set-Content -LiteralPath "$OutDir/pointer-after.txt"
        }
    }
    finally { Complete-Captures }
}
