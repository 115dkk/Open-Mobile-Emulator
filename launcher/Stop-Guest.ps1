# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Stops an Open Mobile Emulator guest.
.DESCRIPTION
Requests an ACPI powerdown through QMP, waits for the recorded QEMU process, and
forces termination only after the timeout. Repeated calls are safe.
.PARAMETER Name
Guest name below OME_HOME\vm.
.PARAMETER TimeoutSec
Seconds to wait after the power-off request before forcing termination.
.PARAMETER AdbSerial
adb serial of the guest for the graceful power-off (adb reboot -p). Empty skips adb.
.EXAMPLE
./Stop-Guest.ps1 -Name default
.EXAMPLE
./Stop-Guest.ps1 -Name default -TimeoutSec 60
#>
[CmdletBinding()]
param(
    [string]$Name = 'default',

    [ValidateRange(0, 3600)]
    [int]$TimeoutSec = 30,

    [string]$AdbSerial = '127.0.0.1:5555'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force

$vmDirectory = Get-OmePath -Kind vm -Name $Name
$pidPath = [IO.Path]::Combine($vmDirectory, 'qemu.pid')
if (-not [IO.File]::Exists($pidPath)) {
    Write-Host "Guest '$Name' is not running; no PID file exists."
    exit 0
}

$pidText = (Get-Content -LiteralPath $pidPath -Raw).Trim()
$processId = 0
if (-not [int]::TryParse($pidText, [ref]$processId) -or $processId -le 0) {
    Remove-Item -LiteralPath $pidPath -Force
    Write-Host "Removed invalid PID file for guest '$Name'."
    exit 0
}

$process = Get-Process -Id $processId -ErrorAction SilentlyContinue
if ($null -eq $process) {
    Remove-Item -LiteralPath $pidPath -Force
    Write-Host "Guest '$Name' was already stopped."
    exit 0
}
if ($process.ProcessName -notmatch '^qemu-system-') {
    Remove-Item -LiteralPath $pidPath -Force
    Write-Warning "PID $processId belongs to '$($process.ProcessName)', not QEMU. Removed the stale PID file without stopping that process."
    exit 0
}

$qmpPort = 4444
try {
    $commandLogs = @(Get-ChildItem -LiteralPath (Get-OmePath -Kind logs) -Filter "qemu-$Name-*.log" -File | Where-Object { $_.Name -notmatch '\.(?:stdout|stderr)\.log$' } | Sort-Object LastWriteTime -Descending)
    if ($commandLogs.Count -gt 0) {
        $commandLine = Get-Content -LiteralPath $commandLogs[0].FullName -Raw
        if ($commandLine -match 'tcp:127\.0\.0\.1:(\d+),server=on,wait=off') {
            $qmpPort = [int]$Matches[1]
        }
    }
}
catch {
    Write-Warning "Could not determine the recorded QMP port; using 4444. $($_.Exception.Message)"
}

# Android treats the ACPI power button as a key press (screen off or power menu),
# not as a shutdown request, so an adb power-off comes first when the guest is
# reachable. QEMU exits on the guest's power-off by itself (2026-09-25 evidence).
$adbRequested = $false
$adb = Find-OmeAdb
if ($null -ne $adb -and -not [string]::IsNullOrWhiteSpace($AdbSerial)) {
    try {
        $state = (& $adb -s $AdbSerial get-state 2>$null | Out-String).Trim()
        if ($state -ceq 'device') {
            & $adb -s $AdbSerial reboot -p 2>$null | Out-Null
            $adbRequested = $true
            Write-Host "Requested power-off through adb ($AdbSerial)."
        }
    }
    catch {
        Write-Warning "adb power-off request failed; falling back to ACPI. $($_.Exception.Message)"
    }
}

if (-not $adbRequested) {
    try {
        [void](Invoke-OmeQmp -Command 'system_powerdown' -Port $qmpPort)
        Write-Host "Sent system_powerdown to guest '$Name'."
    }
    catch {
        Write-Warning "QMP powerdown request failed; waiting for PID $processId before forcing it. $($_.Exception.Message)"
    }
}

$deadline = [DateTimeOffset]::Now.AddSeconds($TimeoutSec)
while ([DateTimeOffset]::Now -lt $deadline) {
    if ($process.HasExited) {
        break
    }
    Start-Sleep -Milliseconds 250
    $process.Refresh()
}

if (-not $process.HasExited) {
    Stop-Process -Id $processId -Force
    Write-Host "Forced QEMU PID $processId to stop after $TimeoutSec seconds."
}
else {
    Write-Host "Guest '$Name' stopped normally."
}
Remove-Item -LiteralPath $pidPath -Force -ErrorAction SilentlyContinue
