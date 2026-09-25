# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Reports whether this Windows host is ready to run Open Mobile Emulator.
.DESCRIPTION
Reads CPU, memory, hypervisor, optional feature, QEMU, firmware, adb, and disk
state without changing the computer. The process always exits with code zero.
.PARAMETER Json
Writes a JSON report instead of a formatted table.
.EXAMPLE
./Check-Host.ps1
.EXAMPLE
./Check-Host.ps1 -Json
#>
[CmdletBinding()]
param(
    [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force

$rows = [Collections.Generic.List[object]]::new()
$errors = [Collections.Generic.List[string]]::new()

function Add-CheckRow {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Check,

        [Parameter(Mandatory)]
        [string]$Status,

        [string]$Details = ''
    )

    $rows.Add([pscustomobject]@{ Check = $Check; Status = $Status; Details = $Details })
}

function Get-OptionalFeatureState {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Name
    )

    try {
        $escapedName = $Name.Replace("'", "''")
        $feature = Get-CimInstance -ClassName Win32_OptionalFeature -Filter "Name='$escapedName'" -ErrorAction Stop
        if ($null -eq $feature) {
            return 'Not found'
        }
        return $(switch ([int]$feature.InstallState) {
            1 { 'Enabled' }
            2 { 'Disabled' }
            3 { 'Absent' }
            default { "Unknown ($($feature.InstallState))" }
        })
    }
    catch {
        $errors.Add("Optional feature ${Name}: $($_.Exception.Message)")
        return 'Unknown'
    }
}

try {
    try {
        $processor = Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop | Select-Object -First 1
        $logicalCores = (Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop).NumberOfLogicalProcessors
        Add-CheckRow -Check 'CPU' -Status 'Detected' -Details "$($processor.Name.Trim()); $logicalCores logical cores"
    }
    catch {
        $errors.Add("CPU: $($_.Exception.Message)")
        Add-CheckRow -Check 'CPU' -Status 'Unknown' -Details $_.Exception.Message
    }

    try {
        $computerSystem = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
        $ramGb = [Math]::Round([double]$computerSystem.TotalPhysicalMemory / 1GB, 1)
        Add-CheckRow -Check 'RAM' -Status 'Detected' -Details "$ramGb GB"
        Add-CheckRow -Check 'Hypervisor present' -Status $(if ($computerSystem.HypervisorPresent) { 'Yes' } else { 'No' }) -Details ''
    }
    catch {
        $errors.Add("Computer system: $($_.Exception.Message)")
        Add-CheckRow -Check 'RAM' -Status 'Unknown' -Details $_.Exception.Message
        Add-CheckRow -Check 'Hypervisor present' -Status 'Unknown' -Details $_.Exception.Message
    }

    $featureStates = [ordered]@{}
    foreach ($featureName in @('HypervisorPlatform', 'Microsoft-Hyper-V-Hypervisor', 'VirtualMachinePlatform')) {
        $featureStates[$featureName] = Get-OptionalFeatureState -Name $featureName
        Add-CheckRow -Check "Feature: $featureName" -Status $featureStates[$featureName] -Details ''
    }

    $whpxDlls = @(
        'C:\Windows\System32\WinHvPlatform.dll',
        'C:\Windows\System32\WinHvEmulation.dll'
    )
    $presentDlls = @($whpxDlls | Where-Object { [IO.File]::Exists($_) })
    Add-CheckRow -Check 'WHPX DLLs' -Status $(if ($presentDlls.Count -eq $whpxDlls.Count) { 'Present' } else { 'Missing' }) -Details ($presentDlls -join '; ')

    $qemu = Find-OmeQemu
    if ($null -eq $qemu) {
        Add-CheckRow -Check 'QEMU' -Status 'Not found' -Details ''
        Add-CheckRow -Check 'qemu-img' -Status 'Not found' -Details ''
    }
    else {
        Add-CheckRow -Check 'QEMU' -Status 'Found' -Details "$($qemu.SystemExe); version $($qemu.Version); source $($qemu.Source)"
        Add-CheckRow -Check 'qemu-img' -Status $(if ($null -ne $qemu.ImgExe) { 'Found' } else { 'Not found' }) -Details $(if ($null -ne $qemu.ImgExe) { $qemu.ImgExe } else { '' })
    }

    $firmware = Find-OmeFirmware -Qemu $qemu
    Add-CheckRow -Check 'Firmware' -Status $(if ($null -ne $firmware) { 'Found' } else { 'Not found' }) -Details $(if ($null -ne $firmware) { "code=$($firmware.Code); vars=$($firmware.VarsTemplate)" } else { '' })

    $adb = Find-OmeAdb
    if ($null -eq $adb) {
        Add-CheckRow -Check 'adb' -Status 'Not found' -Details ''
    }
    else {
        $adbVersion = 'unknown'
        try {
            $versionOutput = @(& $adb version 2>&1)
            if ($LASTEXITCODE -eq 0 -and $versionOutput.Count -gt 0) {
                $adbVersion = ([string]$versionOutput[0]).Trim()
            }
        }
        catch {
            $errors.Add("adb version: $($_.Exception.Message)")
        }
        Add-CheckRow -Check 'adb' -Status 'Found' -Details "$adb; $adbVersion"
    }

    $omeHomePath = Get-OmeHome
    Add-CheckRow -Check 'OME_HOME' -Status 'Configured' -Details $omeHomePath
    $freeDiskGb = 0.0
    try {
        $root = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($omeHomePath))
        $drive = [IO.DriveInfo]::new($root)
        $freeDiskGb = [Math]::Round($drive.AvailableFreeSpace / 1GB, 1)
        Add-CheckRow -Check 'Free disk' -Status $(if ($freeDiskGb -ge 40) { 'Sufficient' } else { 'Insufficient' }) -Details "$freeDiskGb GB on $root"
    }
    catch {
        $errors.Add("Free disk: $($_.Exception.Message)")
        Add-CheckRow -Check 'Free disk' -Status 'Unknown' -Details $_.Exception.Message
    }

    $ready = ($featureStates['HypervisorPlatform'] -ceq 'Enabled') -and
        ($null -ne $qemu) -and
        ($null -ne $firmware) -and
        ($freeDiskGb -ge 40)

    if ($Json) {
        [pscustomobject]@{
            GeneratedAt = [DateTimeOffset]::Now.ToString('o')
            Rows = $rows.ToArray()
            ReadyForM0 = $ready
            Errors = $errors.ToArray()
        } | ConvertTo-Json -Depth 10
    }
    else {
        $rows | Format-Table -AutoSize
        if ($errors.Count -gt 0) {
            Write-Host 'Read errors:' -ForegroundColor Yellow
            foreach ($message in $errors) {
                Write-Host "  $message" -ForegroundColor Yellow
            }
        }
        Write-Host "Ready for M0: $(if ($ready) { 'yes' } else { 'no' })"
    }
}
catch {
    if ($Json) {
        [pscustomobject]@{
            GeneratedAt = [DateTimeOffset]::Now.ToString('o')
            Rows = $rows.ToArray()
            ReadyForM0 = $false
            Errors = @($errors.ToArray()) + @($_.Exception.Message)
        } | ConvertTo-Json -Depth 10
    }
    else {
        Write-Host "Host check could not complete: $($_.Exception.Message)" -ForegroundColor Red
        Write-Host 'Ready for M0: no'
    }
}

exit 0
