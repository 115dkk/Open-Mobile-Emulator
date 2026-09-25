# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Starts an Open Mobile Emulator QEMU guest.
.DESCRIPTION
Validates guest storage, firmware, installer media, and WHPX state; builds a
fixed QEMU device configuration; logs the exact arguments; and supervises QMP
startup. Windows command-line quoting preserves argument elements that contain spaces.
.PARAMETER Name
Guest name below OME_HOME\vm.
.PARAMETER Cdrom
Attaches and boots the installer ISO declared by the guest metadata or manifest.
.PARAMETER Gpu
Selects std, virtio, or virgl display hardware.
.PARAMETER Accel
Selects whpx or tcg. TCG is intended only for smoke tests.
.PARAMETER Cpu
QEMU CPU model.
.PARAMETER MemoryMB
Guest memory in MiB.
.PARAMETER Smp
Virtual CPU count.
.PARAMETER QmpPort
Loopback QMP TCP port.
.PARAMETER AdbPort
Loopback adb forwarding port.
.PARAMETER Audio
Selects dsound, sdl, or no audio device.
.PARAMETER Display
Selects the SDL or GTK display frontend.
.PARAMETER ExtraArgs
Additional QEMU argument elements appended last.
.PARAMETER DryRun
Prints numbered argument elements without starting QEMU.
.PARAMETER Wait
Waits until QEMU exits.
.EXAMPLE
./Start-Guest.ps1 -Name default -Accel whpx -Gpu std
.EXAMPLE
./Start-Guest.ps1 -Name default -Accel tcg -Audio none -DryRun
#>
[CmdletBinding()]
param(
    [string]$Name = 'default',
    [switch]$Cdrom,

    [ValidateSet('std', 'virtio', 'virgl')]
    [string]$Gpu = 'std',

    [ValidateSet('whpx', 'tcg')]
    [string]$Accel = 'whpx',

    [string]$Cpu = 'max',

    [ValidateRange(128, 1048576)]
    [int]$MemoryMB = 6144,

    [ValidateRange(1, 1024)]
    [int]$Smp = 4,

    [ValidateRange(1, 65535)]
    [int]$QmpPort = 4444,

    [ValidateRange(1, 65535)]
    [int]$AdbPort = 5555,

    [ValidateSet('dsound', 'sdl', 'none')]
    [string]$Audio = 'dsound',

    [ValidateSet('sdl', 'gtk')]
    [string]$Display = 'sdl',

    [string[]]$ExtraArgs = @(),
    [switch]$DryRun,
    [switch]$Wait
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module ([IO.Path]::Combine($PSScriptRoot, 'OME.Common.psm1')) -Force

function Get-WhpxFeatureState {
    [CmdletBinding()]
    param()

    try {
        $feature = Get-CimInstance -ClassName Win32_OptionalFeature -Filter "Name='HypervisorPlatform'" -ErrorAction Stop
        if ($null -ne $feature -and [int]$feature.InstallState -eq 1) {
            return 'Enabled'
        }
        if ($null -ne $feature -and [int]$feature.InstallState -eq 2) {
            return 'Disabled'
        }
        return 'Unavailable'
    }
    catch {
        return 'Unknown'
    }
}

function ConvertTo-WindowsCommandLineArgument {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [AllowEmptyString()]
        [string]$Value
    )

    if ($Value.Length -gt 0 -and $Value -notmatch '[\s"]') {
        return $Value
    }

    $builder = [Text.StringBuilder]::new()
    [void]$builder.Append('"')
    $backslashCount = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq [char]'\') {
            $backslashCount++
            continue
        }
        if ($character -eq [char]'"') {
            [void]$builder.Append([char]'\', ($backslashCount * 2) + 1)
            [void]$builder.Append([char]'"')
            $backslashCount = 0
            continue
        }
        if ($backslashCount -gt 0) {
            [void]$builder.Append([char]'\', $backslashCount)
            $backslashCount = 0
        }
        [void]$builder.Append($character)
    }
    if ($backslashCount -gt 0) {
        [void]$builder.Append([char]'\', $backslashCount * 2)
    }
    [void]$builder.Append('"')
    return $builder.ToString()
}

$qemu = Find-OmeQemu
if ($null -eq $qemu) {
    throw 'QEMU was not found. Install it or set OME_QEMU_DIR.'
}
$firmware = Find-OmeFirmware -Qemu $qemu
if ($null -eq $firmware) {
    throw 'EDK2/OVMF firmware was not found. Install QEMU firmware or set OME_FIRMWARE_DIR.'
}

$vmDirectory = Get-OmePath -Kind vm -Name $Name
$diskPath = [IO.Path]::Combine($vmDirectory, 'disk.qcow2')
$varsPath = [IO.Path]::Combine($vmDirectory, 'efivars.fd')
$metadataPath = [IO.Path]::Combine($vmDirectory, 'vm.json')
$pidPath = [IO.Path]::Combine($vmDirectory, 'qemu.pid')
if (-not $DryRun -and [IO.File]::Exists($pidPath)) {
    $recordedPid = 0
    $pidValue = (Get-Content -LiteralPath $pidPath -Raw).Trim()
    if ([int]::TryParse($pidValue, [ref]$recordedPid)) {
        $recordedProcess = Get-Process -Id $recordedPid -ErrorAction SilentlyContinue
        if ($null -ne $recordedProcess -and $recordedProcess.ProcessName -match '^qemu-system-') {
            throw "Guest '$Name' already has a running QEMU process with PID $recordedPid. Stop it before starting another instance."
        }
    }
    Remove-Item -LiteralPath $pidPath -Force
}
if (-not [IO.File]::Exists($diskPath)) {
    throw "Guest disk is missing: $diskPath. Run New-GuestDisk.ps1 -Name '$Name' first."
}
if (-not [IO.File]::Exists($varsPath) -and -not $DryRun) {
    Copy-Item -LiteralPath $firmware.VarsTemplate -Destination $varsPath
}

if ($Accel -ceq 'whpx' -and (Get-WhpxFeatureState) -cne 'Enabled') {
    throw 'HypervisorPlatform is not Enabled. Run Enable-Whpx.ps1 -Consent and reboot. Use -Accel tcg for smoke tests only.'
}

$isoPath = $null
if ($Cdrom) {
    $artifactName = $null
    if ([IO.File]::Exists($metadataPath)) {
        try {
            $metadata = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
            if ($metadata.PSObject.Properties.Name -contains 'isoArtifactName') {
                $artifactName = [string]$metadata.isoArtifactName
            }
        }
        catch {
            throw "Could not read guest metadata '$metadataPath': $($_.Exception.Message)"
        }
    }
    if ([string]::IsNullOrWhiteSpace($artifactName)) {
        $artifactName = [string](@((Get-OmeManifest).artifacts | Where-Object { $_.fetched_by -ceq 'installer' }) | Select-Object -First 1).name
    }
    if ([string]::IsNullOrWhiteSpace($artifactName)) {
        throw 'No installer artifact is declared in the manifest.'
    }
    $artifact = Get-OmeArtifact -Name $artifactName
    $isoPath = [IO.Path]::Combine((Get-OmePath -Kind artifacts), [IO.Path]::GetFileName([string]$artifact.filename))
    if (-not [IO.File]::Exists($isoPath)) {
        throw "Installer ISO is missing: $isoPath. Run Get-Artifacts.ps1 -Name '$artifactName'."
    }
    $hash = (Get-FileHash -LiteralPath $isoPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -cne ([string]$artifact.sha256).ToLowerInvariant()) {
        throw "Installer ISO SHA-256 does not match the manifest: $isoPath"
    }
}

$argumentParameters = @{
    Name = $Name
    DiskPath = $diskPath
    FirmwareCode = $firmware.Code
    FirmwareVars = $varsPath
    Cdrom = $Cdrom
    IsoPath = $isoPath
    Gpu = $Gpu
    Accel = $Accel
    Cpu = $Cpu
    MemoryMB = $MemoryMB
    Smp = $Smp
    QmpPort = $QmpPort
    AdbPort = $AdbPort
    Audio = $Audio
    Display = $Display
    ExtraArgs = $ExtraArgs
}
$arguments = Get-OmeQemuArguments @argumentParameters

if ($DryRun) {
    Write-Host "Executable: $($qemu.SystemExe)"
    for ($index = 0; $index -lt $arguments.Count; $index++) {
        Write-Host ('[{0:D2}] {1}' -f $index, $arguments[$index])
    }
    exit 0
}

$logsDirectory = Get-OmePath -Kind logs
$timestamp = Get-Date -Format 'yyyyMMdd-HHmmssfff'
$commandLog = [IO.Path]::Combine($logsDirectory, "qemu-$Name-$timestamp.log")
$stdoutLog = [IO.Path]::Combine($logsDirectory, "qemu-$Name-$timestamp.stdout.log")
$stderrLog = [IO.Path]::Combine($logsDirectory, "qemu-$Name-$timestamp.stderr.log")
$quotedExecutable = ConvertTo-WindowsCommandLineArgument -Value $qemu.SystemExe
$quotedArguments = @($arguments | ForEach-Object { ConvertTo-WindowsCommandLineArgument -Value $_ })
$commandLine = (@($quotedExecutable) + $quotedArguments) -join ' '
Set-Content -LiteralPath $commandLog -Value $commandLine -Encoding utf8

# Start-Process owns the redirected files after this launcher exits. Every
# argument is already quoted with the Windows CommandLineToArgvW rules, which
# keeps paths such as C:\Program Files\qemu in one QEMU argument.
$process = Start-Process -FilePath $qemu.SystemExe -ArgumentList $quotedArguments -PassThru -RedirectStandardOutput $stdoutLog -RedirectStandardError $stderrLog
Set-Content -LiteralPath $pidPath -Value ([string]$process.Id) -Encoding ascii
Write-Host "QEMU started with PID $($process.Id)."
Write-Host "Command log: $commandLog"
Write-Host "Standard output log: $stdoutLog"
Write-Host "Standard error log: $stderrLog"

$deadline = [DateTimeOffset]::Now.AddSeconds(10)
$status = $null
while ([DateTimeOffset]::Now -lt $deadline -and -not $process.HasExited) {
    try {
        $status = Invoke-OmeQmp -Command 'query-status' -Port $QmpPort -TimeoutMs 1000
        break
    }
    catch {
        Start-Sleep -Milliseconds 250
    }
}
if ($null -eq $status) {
    $exitDescription = if ($process.HasExited) { "exit code $($process.ExitCode)" } else { 'QMP startup timeout' }
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit()
    }
    Remove-Item -LiteralPath $pidPath -Force -ErrorAction SilentlyContinue
    throw "QEMU did not become ready ($exitDescription). See $stderrLog"
}
Write-Host "QMP status: $($status | ConvertTo-Json -Compress)"

if ($Wait) {
    $process.WaitForExit()
    Remove-Item -LiteralPath $pidPath -Force -ErrorAction SilentlyContinue
    Write-Host "QEMU exited with code $($process.ExitCode)."
    exit $process.ExitCode
}
