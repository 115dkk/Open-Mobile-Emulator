# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Provides shared launcher functions for Open Mobile Emulator.
.DESCRIPTION
Resolves local data paths and tools, validates artifact URLs, communicates with
QEMU over QMP, converts QEMU screenshots, waits for adb, and builds QEMU
argument arrays. Import this module from the launcher scripts.
.EXAMPLE
Import-Module (Join-Path $PSScriptRoot 'OME.Common.psm1')
Get-OmePath -Kind logs
#>
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

<#
.SYNOPSIS
Gets the OME data root.
.DESCRIPTION
Returns OME_HOME when set or the default directory below LOCALAPPDATA.
.EXAMPLE
Get-OmeHome
#>
function Get-OmeHome {
    [CmdletBinding()]
    param()

    if (-not [string]::IsNullOrWhiteSpace($env:OME_HOME)) {
        return [IO.Path]::GetFullPath($env:OME_HOME)
    }
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw 'LOCALAPPDATA is not set and OME_HOME was not provided.'
    }
    return [IO.Path]::Combine($env:LOCALAPPDATA, 'OpenMobileEmulator')
}

<#
.SYNOPSIS
Gets and creates an OME data subdirectory.
.DESCRIPTION
Resolves a standard data folder and creates it lazily below OME_HOME.
.PARAMETER Kind
Selects artifacts, vm, logs, or apks.
.PARAMETER Name
Adds a relative subdirectory below the selected folder.
.EXAMPLE
Get-OmePath -Kind vm -Name default
#>
function Get-OmePath {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateSet('artifacts', 'vm', 'logs', 'apks')]
        [string]$Kind,

        [string]$Name
    )

    $basePath = [IO.Path]::Combine((Get-OmeHome), $Kind)
    $path = $basePath
    if (-not [string]::IsNullOrWhiteSpace($Name)) {
        if ([IO.Path]::IsPathRooted($Name)) {
            throw "Name must be relative: $Name"
        }
        $path = [IO.Path]::Combine($basePath, $Name)
    }

    $baseFull = [IO.Path]::GetFullPath($basePath)
    $pathFull = [IO.Path]::GetFullPath($path)
    $relativePath = [IO.Path]::GetRelativePath($baseFull, $pathFull)
    $parentPrefix = [string]::Concat('..', [IO.Path]::DirectorySeparatorChar)
    if ([IO.Path]::IsPathRooted($relativePath) -or $relativePath -ceq '..' -or $relativePath.StartsWith($parentPrefix, [StringComparison]::Ordinal)) {
        throw "Resolved path is outside the $Kind directory: $Name"
    }

    [void][IO.Directory]::CreateDirectory($pathFull)
    return $pathFull
}

<#
.SYNOPSIS
Gets the repository root.
.DESCRIPTION
Returns the parent of the launcher directory containing this module.
.EXAMPLE
Get-OmeRepoRoot
#>
function Get-OmeRepoRoot {
    [CmdletBinding()]
    param()

    return [IO.Directory]::GetParent($PSScriptRoot).FullName
}

<#
.SYNOPSIS
Reads the artifact manifest.
.DESCRIPTION
Parses manifests/artifacts.json from the repository.
.EXAMPLE
(Get-OmeManifest).artifacts
#>
function Get-OmeManifest {
    [CmdletBinding()]
    param()

    $manifestPath = [IO.Path]::Combine((Get-OmeRepoRoot), 'manifests', 'artifacts.json')
    if (-not [IO.File]::Exists($manifestPath)) {
        throw "Artifact manifest was not found: $manifestPath"
    }
    return Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json -Depth 100
}

<#
.SYNOPSIS
Gets one manifest artifact.
.DESCRIPTION
Returns the uniquely named artifact entry or throws when it is absent.
.PARAMETER Name
Exact artifact name from the manifest.
.EXAMPLE
Get-OmeArtifact -Name bliss-os-16.9.7-gapps-x86_64-iso
#>
function Get-OmeArtifact {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateNotNullOrEmpty()]
        [string]$Name
    )

    $matches = @((Get-OmeManifest).artifacts | Where-Object { $_.name -ceq $Name })
    if ($matches.Count -eq 0) {
        throw "Artifact is not in the manifest: $Name"
    }
    if ($matches.Count -gt 1) {
        throw "Artifact name is duplicated in the manifest: $Name"
    }
    return $matches[0]
}

<#
.SYNOPSIS
Tests an artifact URL host.
.DESCRIPTION
Accepts HTTPS only when the host equals or is below a manifest allowed host.
.PARAMETER Url
Absolute URL to test.
.EXAMPLE
Test-OmeAllowedUrl -Url https://downloads.sourceforge.net/x
#>
function Test-OmeAllowedUrl {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateNotNullOrEmpty()]
        [string]$Url
    )

    $uri = $null
    if (-not [Uri]::TryCreate($Url, [UriKind]::Absolute, [ref]$uri)) {
        return $false
    }
    if ($uri.Scheme -cne 'https' -or [string]::IsNullOrWhiteSpace($uri.DnsSafeHost)) {
        return $false
    }

    $hostName = $uri.DnsSafeHost.TrimEnd('.').ToLowerInvariant()
    foreach ($allowedHostValue in (Get-OmeManifest).allowed_hosts) {
        $allowedHost = ([string]$allowedHostValue).Trim().TrimEnd('.').ToLowerInvariant()
        if ([string]::IsNullOrWhiteSpace($allowedHost)) {
            continue
        }
        if (($hostName -ceq $allowedHost) -or $hostName.EndsWith(".$allowedHost", [StringComparison]::Ordinal)) {
            return $true
        }
    }
    return $false
}

function Test-OmePortableExecutable {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    try {
        $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        try {
            if ($stream.Length -lt 2) {
                return $false
            }
            return ($stream.ReadByte() -eq 0x4d) -and ($stream.ReadByte() -eq 0x5a)
        }
        finally {
            $stream.Dispose()
        }
    }
    catch {
        return $false
    }
}

function Get-OmeQemuVersion {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$SystemExe
    )

    if (-not (Test-OmePortableExecutable -Path $SystemExe)) {
        return 'unknown'
    }
    try {
        $output = @(& $SystemExe --version 2>&1)
        if ($LASTEXITCODE -ne 0 -or $output.Count -eq 0) {
            return 'unknown'
        }
        $firstLine = [string]$output[0]
        if ($firstLine -match '(?i)QEMU emulator version\s+([^\s]+)') {
            return $Matches[1]
        }
        return $firstLine.Trim()
    }
    catch {
        return 'unknown'
    }
}

function Get-OmeQemuShareDirectory {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$BinaryDirectory
    )

    $parentDirectory = [IO.Directory]::GetParent($BinaryDirectory)
    $candidates = [Collections.Generic.List[string]]::new()
    $candidates.Add([IO.Path]::Combine($BinaryDirectory, 'share'))
    if ($null -ne $parentDirectory) {
        $candidates.Add([IO.Path]::Combine($parentDirectory.FullName, 'share'))
    }
    foreach ($candidate in $candidates) {
        if ([IO.Directory]::Exists($candidate)) {
            return [IO.Path]::GetFullPath($candidate)
        }
    }
    return $null
}

<#
.SYNOPSIS
Finds QEMU tools.
.DESCRIPTION
Searches the configured, custom-build, distribution, and PATH locations.
.EXAMPLE
Find-OmeQemu
#>
function Find-OmeQemu {
    [CmdletBinding()]
    param()

    $candidates = [Collections.Generic.List[object]]::new()
    if (-not [string]::IsNullOrWhiteSpace($env:OME_QEMU_DIR)) {
        $candidates.Add([pscustomobject]@{ Directory = $env:OME_QEMU_DIR; Source = 'env' })
    }
    $candidates.Add([pscustomobject]@{
        Directory = [IO.Path]::Combine((Get-OmeRepoRoot), 'qemu-build', 'out', 'bin')
        Source = 'custom-build'
    })
    $candidates.Add([pscustomobject]@{ Directory = 'C:\Program Files\qemu'; Source = 'distribution' })

    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($candidate in $candidates) {
        $directory = [IO.Path]::GetFullPath([string]$candidate.Directory)
        if (-not $seen.Add($directory)) {
            continue
        }
        $systemExe = [IO.Path]::Combine($directory, 'qemu-system-x86_64.exe')
        if (-not [IO.File]::Exists($systemExe)) {
            continue
        }
        $imgCandidate = [IO.Path]::Combine($directory, 'qemu-img.exe')
        $imgExe = if ([IO.File]::Exists($imgCandidate)) { $imgCandidate } else { $null }
        return [pscustomobject]@{
            SystemExe = $systemExe
            ImgExe = $imgExe
            ShareDir = Get-OmeQemuShareDirectory -BinaryDirectory $directory
            Version = Get-OmeQemuVersion -SystemExe $systemExe
            Source = [string]$candidate.Source
        }
    }

    $systemCommand = Get-Command 'qemu-system-x86_64.exe' -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $systemCommand) {
        return $null
    }
    $systemPath = $systemCommand.Source
    $systemDirectory = [IO.Path]::GetDirectoryName($systemPath)
    $imgPath = [IO.Path]::Combine($systemDirectory, 'qemu-img.exe')
    if (-not [IO.File]::Exists($imgPath)) {
        $imgCommand = Get-Command 'qemu-img.exe' -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        $imgPath = if ($null -ne $imgCommand) { $imgCommand.Source } else { $null }
    }
    return [pscustomobject]@{
        SystemExe = $systemPath
        ImgExe = $imgPath
        ShareDir = Get-OmeQemuShareDirectory -BinaryDirectory $systemDirectory
        Version = Get-OmeQemuVersion -SystemExe $systemPath
        Source = 'path'
    }
}

<#
.SYNOPSIS
Finds EDK2 or OVMF firmware.
.DESCRIPTION
Returns a code and variable-template pair found in configured QEMU directories.
.PARAMETER Qemu
QEMU discovery object. It may be null when OME_FIRMWARE_DIR is set.
.EXAMPLE
Find-OmeFirmware -Qemu (Find-OmeQemu)
#>
function Find-OmeFirmware {
    [CmdletBinding()]
    param(
        [AllowNull()]
        [object]$Qemu
    )

    $directories = [Collections.Generic.List[string]]::new()
    if (-not [string]::IsNullOrWhiteSpace($env:OME_FIRMWARE_DIR)) {
        $directories.Add([IO.Path]::GetFullPath($env:OME_FIRMWARE_DIR))
    }
    if ($null -ne $Qemu -and $null -ne $Qemu.ShareDir -and -not [string]::IsNullOrWhiteSpace([string]$Qemu.ShareDir)) {
        $shareDirectory = [IO.Path]::GetFullPath([string]$Qemu.ShareDir)
        $directories.Add($shareDirectory)
        # The distribution installer puts firmware directly in share\; a QEMU built
        # with --datadir=share/qemu (qemu-build/) puts it in share\qemu\.
        $directories.Add([IO.Path]::Combine($shareDirectory, 'qemu'))
    }

    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($directory in $directories) {
        if (-not $seen.Add($directory) -or -not [IO.Directory]::Exists($directory)) {
            continue
        }
        $pairs = @(
            @('edk2-x86_64-code.fd', 'edk2-i386-vars.fd'),
            @('OVMF_CODE.fd', 'OVMF_VARS.fd')
        )
        foreach ($pair in $pairs) {
            $code = [IO.Path]::Combine($directory, $pair[0])
            $vars = [IO.Path]::Combine($directory, $pair[1])
            if ([IO.File]::Exists($code) -and [IO.File]::Exists($vars)) {
                return [pscustomobject]@{ Code = $code; VarsTemplate = $vars }
            }
        }
    }
    return $null
}

<#
.SYNOPSIS
Finds adb.exe.
.DESCRIPTION
Searches OME_ADB, Android SDK locations, and PATH.
.EXAMPLE
Find-OmeAdb
#>
function Find-OmeAdb {
    [CmdletBinding()]
    param()

    $candidates = [Collections.Generic.List[string]]::new()
    if (-not [string]::IsNullOrWhiteSpace($env:OME_ADB)) {
        $candidates.Add($env:OME_ADB)
    }
    if (-not [string]::IsNullOrWhiteSpace($env:ANDROID_HOME)) {
        $candidates.Add([IO.Path]::Combine($env:ANDROID_HOME, 'platform-tools', 'adb.exe'))
    }
    if (-not [string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        $candidates.Add([IO.Path]::Combine($env:LOCALAPPDATA, 'Android', 'Sdk', 'platform-tools', 'adb.exe'))
    }
    foreach ($candidate in $candidates) {
        if ([IO.File]::Exists($candidate)) {
            return [IO.Path]::GetFullPath($candidate)
        }
    }
    $command = Get-Command 'adb.exe' -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $command) {
        return $command.Source
    }
    return $null
}

<#
.SYNOPSIS
Finds the newest Android SDK apksigner.
.DESCRIPTION
Searches versioned build-tools directories under configured SDK roots.
.EXAMPLE
Find-OmeApkSigner
#>
function Find-OmeApkSigner {
    [CmdletBinding()]
    param()

    $sdkRoots = [Collections.Generic.List[string]]::new()
    if (-not [string]::IsNullOrWhiteSpace($env:ANDROID_HOME)) {
        $sdkRoots.Add($env:ANDROID_HOME)
    }
    if (-not [string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        $sdkRoots.Add([IO.Path]::Combine($env:LOCALAPPDATA, 'Android', 'Sdk'))
    }

    $signers = [Collections.Generic.List[object]]::new()
    $seenRoots = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($rootValue in $sdkRoots) {
        $root = [IO.Path]::GetFullPath($rootValue)
        if (-not $seenRoots.Add($root)) {
            continue
        }
        $buildTools = [IO.Path]::Combine($root, 'build-tools')
        if (-not [IO.Directory]::Exists($buildTools)) {
            continue
        }
        foreach ($directory in Get-ChildItem -LiteralPath $buildTools -Directory) {
            $signer = [IO.Path]::Combine($directory.FullName, 'apksigner.bat')
            if (-not [IO.File]::Exists($signer)) {
                continue
            }
            $parsedVersion = [version]'0.0'
            [void][version]::TryParse($directory.Name, [ref]$parsedVersion)
            $signers.Add([pscustomobject]@{ Path = $signer; Version = $parsedVersion; Name = $directory.Name })
        }
    }

    $newest = $signers | Sort-Object -Property @{ Expression = 'Version'; Descending = $true }, @{ Expression = 'Name'; Descending = $true } | Select-Object -First 1
    if ($null -eq $newest) {
        return $null
    }
    return $newest.Path
}

<#
.SYNOPSIS
Writes an OME launcher log message.
.DESCRIPTION
Writes a coloured console line and appends an ISO-8601 entry to launcher.log.
.PARAMETER Message
Message text.
.PARAMETER Level
Info, Warn, or Error.
.EXAMPLE
Write-OmeLog -Message 'Guest started.' -Level Info
#>
function Write-OmeLog {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$Message,

        [ValidateSet('Info', 'Warn', 'Error')]
        [string]$Level = 'Info'
    )

    $timestamp = [DateTimeOffset]::Now.ToString('o')
    $line = "[$timestamp] [$Level] $Message"
    $colour = switch ($Level) {
        'Info' { 'Gray' }
        'Warn' { 'Yellow' }
        'Error' { 'Red' }
    }
    Write-Host $line -ForegroundColor $colour
    $logPath = [IO.Path]::Combine((Get-OmePath -Kind logs), 'launcher.log')
    Add-Content -LiteralPath $logPath -Value $line -Encoding utf8
}

function Read-OmeQmpMessage {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [IO.StreamReader]$Reader,

        [string]$ExpectedId,

        [switch]$Greeting
    )

    while ($true) {
        $line = $Reader.ReadLine()
        if ($null -eq $line) {
            throw 'QMP closed the connection before sending a response.'
        }
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        try {
            $message = $line | ConvertFrom-Json -Depth 100
        }
        catch {
            throw "QMP returned invalid JSON: $line"
        }
        $propertyNames = @($message.PSObject.Properties.Name)
        if ($propertyNames -contains 'event') {
            continue
        }
        if ($Greeting -and $propertyNames -contains 'QMP') {
            return $message
        }
        if ($propertyNames -contains 'error') {
            $description = if ($null -ne $message.error.desc) { [string]$message.error.desc } else { 'Unknown QMP error' }
            throw "QMP error: $description"
        }
        if (-not [string]::IsNullOrWhiteSpace($ExpectedId)) {
            if (-not ($propertyNames -contains 'id') -or ([string]$message.id -cne $ExpectedId)) {
                continue
            }
        }
        if ($propertyNames -contains 'return') {
            return $message
        }
    }
}

function Send-OmeQmpObject {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [IO.StreamWriter]$Writer,

        [Parameter(Mandatory)]
        [Collections.IDictionary]$Object
    )

    $json = $Object | ConvertTo-Json -Compress -Depth 100
    $Writer.WriteLine($json)
    $Writer.Flush()
}

<#
.SYNOPSIS
Runs one QMP command.
.DESCRIPTION
Negotiates QMP capabilities, skips events, and returns the command result.
.PARAMETER Command
QMP execute command name.
.PARAMETER Arguments
Optional QMP command arguments.
.PARAMETER HostName
QMP TCP host.
.PARAMETER Port
QMP TCP port.
.PARAMETER TimeoutMs
Connection and stream timeout in milliseconds.
.EXAMPLE
Invoke-OmeQmp -Command query-status
#>
function Invoke-OmeQmp {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateNotNullOrEmpty()]
        [string]$Command,

        [hashtable]$Arguments,

        [string]$HostName = '127.0.0.1',

        [ValidateRange(1, 65535)]
        [int]$Port = 4444,

        [ValidateRange(1, 600000)]
        [int]$TimeoutMs = 5000
    )

    $client = [Net.Sockets.TcpClient]::new()
    try {
        $connectTask = $client.ConnectAsync($HostName, $Port)
        if (-not $connectTask.Wait($TimeoutMs)) {
            throw "Timed out connecting to QMP at ${HostName}:$Port."
        }
        [void]$connectTask.GetAwaiter().GetResult()

        $stream = $client.GetStream()
        $stream.ReadTimeout = $TimeoutMs
        $stream.WriteTimeout = $TimeoutMs
        $encoding = [Text.UTF8Encoding]::new($false)
        $reader = [IO.StreamReader]::new($stream, $encoding, $false, 4096, $true)
        $writer = [IO.StreamWriter]::new($stream, $encoding, 4096, $true)
        $writer.NewLine = "`r`n"
        try {
            [void](Read-OmeQmpMessage -Reader $reader -Greeting)

            $capabilityId = "capabilities-$([Guid]::NewGuid().ToString('N'))"
            Send-OmeQmpObject -Writer $writer -Object ([ordered]@{
                execute = 'qmp_capabilities'
                id = $capabilityId
            })
            [void](Read-OmeQmpMessage -Reader $reader -ExpectedId $capabilityId)

            $commandId = "command-$([Guid]::NewGuid().ToString('N'))"
            $request = [ordered]@{ execute = $Command }
            if ($null -ne $Arguments -and $Arguments.Count -gt 0) {
                $request.arguments = $Arguments
            }
            $request.id = $commandId
            Send-OmeQmpObject -Writer $writer -Object $request
            $response = Read-OmeQmpMessage -Reader $reader -ExpectedId $commandId
            Write-Output -NoEnumerate $response.return
        }
        finally {
            $writer.Dispose()
            $reader.Dispose()
        }
    }
    finally {
        $client.Dispose()
    }
}

<#
.SYNOPSIS
Builds the QMP events for one virtio multitouch contact operation.
.DESCRIPTION
Returns a slot/tracking event followed by X/Y data for begin and update.
End and cancel release the slot with tracking-id -1. The caller owns contact
state and must send a touch button release only when the last contact ends.
QEMU 11.1 qapi/ui.json requires every mtt field even when the device ignores
axis/value on the slot event. hw/input/virtio-input-hid.c uses only data events
for coordinates; input-send-event does not expand begin/update automatically.
.PARAMETER Type
Contact operation: begin, update, end, or cancel.
.PARAMETER Slot
Contact slot, 0 through 9 (the ten slots used by QEMU ui/input.c).
.PARAMETER TrackingId
Nonnegative contact identity for begin/update, retained by the caller.
Ignored on end/cancel, which always emit -1.
.PARAMETER X
Absolute horizontal coordinate, 0 through 32767.
.PARAMETER Y
Absolute vertical coordinate, 0 through 32767.
.EXAMPLE
$events = @(ConvertTo-OmeQmpTouchEvent -Type begin -Slot 0 -TrackingId 1 -X 8192 -Y 8192)
Invoke-OmeQmp -Command input-send-event -Arguments @{ events = $events }
#>
function ConvertTo-OmeQmpTouchEvent {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateSet('begin', 'update', 'end', 'cancel')]
        [string]$Type,

        [Parameter(Mandatory)]
        [ValidateRange(0, 9)]
        [int]$Slot,

        [Parameter(Mandatory)]
        [ValidateRange(0, 2147483647)]
        [int]$TrackingId,

        [Parameter(Mandatory)]
        [ValidateRange(0, 32767)]
        [int]$X,

        [Parameter(Mandatory)]
        [ValidateRange(0, 32767)]
        [int]$Y
    )

    $operation = $Type.ToLowerInvariant()
    $released = $operation -in @('end', 'cancel')
    [ordered]@{
        type = 'mtt'
        data = [ordered]@{
            type = $operation
            slot = $Slot
            'tracking-id' = $(if ($released) { -1 } else { $TrackingId })
            axis = 'x'
            value = 0
        }
    }
    if (-not $released) {
        foreach ($axis in @('x', 'y')) {
            [ordered]@{
                type = 'mtt'
                data = [ordered]@{
                    type = 'data'
                    slot = $Slot
                    'tracking-id' = $TrackingId
                    axis = $axis
                    value = $(if ($axis -ceq 'x') { $X } else { $Y })
                }
            }
        }
    }
}

<#
.SYNOPSIS
Measures QMP query latency.
.DESCRIPTION
Times repeated query-status round trips and returns minimum, average, and maximum milliseconds.
.PARAMETER Iterations
Number of query-status round trips.
.PARAMETER HostName
QMP TCP host.
.PARAMETER Port
QMP TCP port.
.PARAMETER TimeoutMs
Timeout for each QMP call.
.EXAMPLE
Measure-OmeQmpLatency -Iterations 20
#>
function Measure-OmeQmpLatency {
    [CmdletBinding()]
    param(
        [ValidateRange(1, 10000)]
        [int]$Iterations = 20,

        [string]$HostName = '127.0.0.1',

        [ValidateRange(1, 65535)]
        [int]$Port = 4444,

        [ValidateRange(1, 600000)]
        [int]$TimeoutMs = 5000
    )

    $client = [Net.Sockets.TcpClient]::new()
    try {
        $connectTask = $client.ConnectAsync($HostName, $Port)
        if (-not $connectTask.Wait($TimeoutMs)) {
            throw "Timed out connecting to QMP at ${HostName}:$Port."
        }
        [void]$connectTask.GetAwaiter().GetResult()
        $stream = $client.GetStream()
        $stream.ReadTimeout = $TimeoutMs
        $stream.WriteTimeout = $TimeoutMs
        $encoding = [Text.UTF8Encoding]::new($false)
        $reader = [IO.StreamReader]::new($stream, $encoding, $false, 4096, $true)
        $writer = [IO.StreamWriter]::new($stream, $encoding, 4096, $true)
        $writer.NewLine = "`r`n"
        try {
            [void](Read-OmeQmpMessage -Reader $reader -Greeting)
            $capabilityId = "capabilities-$([Guid]::NewGuid().ToString('N'))"
            Send-OmeQmpObject -Writer $writer -Object ([ordered]@{
                execute = 'qmp_capabilities'
                id = $capabilityId
            })
            [void](Read-OmeQmpMessage -Reader $reader -ExpectedId $capabilityId)

            $samples = [Collections.Generic.List[double]]::new()
            for ($index = 0; $index -lt $Iterations; $index++) {
                $commandId = "latency-$index-$([Guid]::NewGuid().ToString('N'))"
                $stopwatch = [Diagnostics.Stopwatch]::StartNew()
                Send-OmeQmpObject -Writer $writer -Object ([ordered]@{
                    execute = 'query-status'
                    id = $commandId
                })
                [void](Read-OmeQmpMessage -Reader $reader -ExpectedId $commandId)
                $stopwatch.Stop()
                $samples.Add($stopwatch.Elapsed.TotalMilliseconds)
            }
        }
        finally {
            $writer.Dispose()
            $reader.Dispose()
        }
    }
    finally {
        $client.Dispose()
    }

    $measure = $samples | Measure-Object -Minimum -Maximum -Average
    return [pscustomobject]@{
        Iterations = $Iterations
        MinMs = [Math]::Round([double]$measure.Minimum, 3)
        AverageMs = [Math]::Round([double]$measure.Average, 3)
        MaxMs = [Math]::Round([double]$measure.Maximum, 3)
    }
}

function Read-OmePpmToken {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [byte[]]$Bytes,

        [Parameter(Mandatory)]
        [ref]$Offset
    )

    while ($Offset.Value -lt $Bytes.Length) {
        $value = $Bytes[$Offset.Value]
        if ($value -eq 0x23) {
            while ($Offset.Value -lt $Bytes.Length -and $Bytes[$Offset.Value] -notin @(0x0a, 0x0d)) {
                $Offset.Value++
            }
            continue
        }
        if ([char]::IsWhiteSpace([char]$value)) {
            $Offset.Value++
            continue
        }
        break
    }
    if ($Offset.Value -ge $Bytes.Length) {
        throw 'Unexpected end of PPM header.'
    }

    $start = $Offset.Value
    while ($Offset.Value -lt $Bytes.Length) {
        $value = $Bytes[$Offset.Value]
        if ([char]::IsWhiteSpace([char]$value) -or $value -eq 0x23) {
            break
        }
        $Offset.Value++
    }
    return [Text.Encoding]::ASCII.GetString($Bytes, $start, $Offset.Value - $start)
}

<#
.SYNOPSIS
Converts a P6 PPM file to PNG.
.DESCRIPTION
Copies RGB data into a locked 24-bit System.Drawing bitmap buffer.
.PARAMETER PpmPath
Input P6 PPM path with maxval 255.
.PARAMETER PngPath
Output PNG path.
.EXAMPLE
Convert-OmePpmToPng -PpmPath screen.ppm -PngPath screen.png
#>
function Convert-OmePpmToPng {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$PpmPath,

        [Parameter(Mandatory)]
        [string]$PngPath
    )

    Add-Type -AssemblyName System.Drawing
    $bytes = [IO.File]::ReadAllBytes($PpmPath)
    $offset = 0
    $magic = Read-OmePpmToken -Bytes $bytes -Offset ([ref]$offset)
    $widthToken = Read-OmePpmToken -Bytes $bytes -Offset ([ref]$offset)
    $heightToken = Read-OmePpmToken -Bytes $bytes -Offset ([ref]$offset)
    $maxToken = Read-OmePpmToken -Bytes $bytes -Offset ([ref]$offset)
    if ($magic -cne 'P6') {
        throw "Unsupported PPM type '$magic'; expected P6."
    }
    $width = 0
    $height = 0
    $maxValue = 0
    if (-not [int]::TryParse($widthToken, [ref]$width) -or $width -le 0) {
        throw "Invalid PPM width: $widthToken"
    }
    if (-not [int]::TryParse($heightToken, [ref]$height) -or $height -le 0) {
        throw "Invalid PPM height: $heightToken"
    }
    if (-not [int]::TryParse($maxToken, [ref]$maxValue) -or $maxValue -ne 255) {
        throw "Unsupported PPM maxval '$maxToken'; expected 255."
    }
    if ($offset -ge $bytes.Length -or -not [char]::IsWhiteSpace([char]$bytes[$offset])) {
        throw 'PPM header is not followed by a whitespace delimiter.'
    }
    if ($bytes[$offset] -eq 0x0d -and ($offset + 1) -lt $bytes.Length -and $bytes[$offset + 1] -eq 0x0a) {
        $offset += 2
    }
    else {
        $offset++
    }

    $pixelByteCount64 = [long]$width * [long]$height * 3L
    if ($pixelByteCount64 -gt [int]::MaxValue) {
        throw "PPM image is too large to convert in memory: $pixelByteCount64 bytes."
    }
    $pixelByteCount = [int]$pixelByteCount64
    if (($bytes.Length - $offset) -lt $pixelByteCount) {
        throw "PPM pixel data is truncated: expected $pixelByteCount bytes."
    }

    $outputDirectory = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($PngPath))
    [void][IO.Directory]::CreateDirectory($outputDirectory)
    $bitmap = [Drawing.Bitmap]::new($width, $height, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
    try {
        $rectangle = [Drawing.Rectangle]::new(0, 0, $width, $height)
        $bitmapData = $bitmap.LockBits($rectangle, [Drawing.Imaging.ImageLockMode]::WriteOnly, [Drawing.Imaging.PixelFormat]::Format24bppRgb)
        try {
            if ($bitmapData.Stride -le 0) {
                throw "Unsupported bitmap stride: $($bitmapData.Stride)"
            }
            $buffer = [byte[]]::new($bitmapData.Stride * $height)
            for ($y = 0; $y -lt $height; $y++) {
                for ($x = 0; $x -lt $width; $x++) {
                    $source = $offset + (($y * $width + $x) * 3)
                    $destination = ($y * $bitmapData.Stride) + ($x * 3)
                    $buffer[$destination] = $bytes[$source + 2]
                    $buffer[$destination + 1] = $bytes[$source + 1]
                    $buffer[$destination + 2] = $bytes[$source]
                }
            }
            [Runtime.InteropServices.Marshal]::Copy($buffer, 0, $bitmapData.Scan0, $buffer.Length)
        }
        finally {
            $bitmap.UnlockBits($bitmapData)
        }
        $bitmap.Save($PngPath, [Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        $bitmap.Dispose()
    }
}

<#
.SYNOPSIS
Saves a QMP screenshot as PNG.
.DESCRIPTION
Requests a temporary PPM, converts it to PNG, and deletes the PPM.
.PARAMETER OutFile
Output PNG path.
.PARAMETER HostName
QMP TCP host.
.PARAMETER Port
QMP TCP port.
.PARAMETER TimeoutMs
QMP timeout in milliseconds.
.EXAMPLE
Save-OmeQmpScreenshot -OutFile firmware.png
#>
function Save-OmeQmpScreenshot {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$OutFile,

        [string]$HostName = '127.0.0.1',

        [ValidateRange(1, 65535)]
        [int]$Port = 4444,

        [ValidateRange(1, 600000)]
        [int]$TimeoutMs = 5000
    )

    $logs = Get-OmePath -Kind logs
    $ppmPath = [IO.Path]::Combine($logs, "qmp-screenshot-$([Guid]::NewGuid().ToString('N')).ppm")
    try {
        [void](Invoke-OmeQmp -Command 'screendump' -Arguments @{ filename = $ppmPath } -HostName $HostName -Port $Port -TimeoutMs $TimeoutMs)
        if (-not [IO.File]::Exists($ppmPath)) {
            throw "QMP screendump did not create the expected file: $ppmPath"
        }
        Convert-OmePpmToPng -PpmPath $ppmPath -PngPath $OutFile
        return [IO.Path]::GetFullPath($OutFile)
    }
    finally {
        if ([IO.File]::Exists($ppmPath)) {
            Remove-Item -LiteralPath $ppmPath -Force
        }
    }
}

function Invoke-OmeExternalText {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$FilePath,

        [string[]]$ArgumentList = @(),

        [ValidateRange(1, 3600000)]
        [int]$TimeoutMs = 60000
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $ArgumentList) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    try {
        if (-not $process.Start()) {
            throw "Failed to start: $FilePath"
        }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMs)) {
            $process.Kill($true)
            throw "Command timed out after $TimeoutMs ms: $FilePath"
        }
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            StdOut = $stdout
            StdErr = $stderr
        }
    }
    finally {
        $process.Dispose()
    }
}

<#
.SYNOPSIS
Waits for Android boot completion.
.DESCRIPTION
Connects TCP serials, waits for adb, and polls sys.boot_completed until it is 1.
.PARAMETER Serial
adb device serial.
.PARAMETER TimeoutSec
Overall timeout in seconds.
.EXAMPLE
Wait-OmeAdbBoot -Serial 127.0.0.1:5555
#>
function Wait-OmeAdbBoot {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [ValidateNotNullOrEmpty()]
        [string]$Serial,

        [ValidateRange(1, 86400)]
        [int]$TimeoutSec = 600
    )

    $adb = Find-OmeAdb
    if ($null -eq $adb) {
        throw 'adb.exe was not found. Install Android SDK platform-tools or set OME_ADB.'
    }
    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    if ($Serial -match '^(?:\[[^\]]+\]|[^:]+):\d+$') {
        $connect = Invoke-OmeExternalText -FilePath $adb -ArgumentList @('connect', $Serial) -TimeoutMs ([Math]::Min($TimeoutSec * 1000, 60000))
        if ($connect.ExitCode -ne 0) {
            throw "adb connect failed: $($connect.StdErr.Trim())"
        }
    }

    $remainingMs = [Math]::Max(1, ($TimeoutSec * 1000) - [int]$stopwatch.ElapsedMilliseconds)
    $wait = Invoke-OmeExternalText -FilePath $adb -ArgumentList @('-s', $Serial, 'wait-for-device') -TimeoutMs $remainingMs
    if ($wait.ExitCode -ne 0) {
        throw "adb wait-for-device failed: $($wait.StdErr.Trim())"
    }

    while ($stopwatch.Elapsed.TotalSeconds -lt $TimeoutSec) {
        $remainingMs = [Math]::Max(1, ($TimeoutSec * 1000) - [int]$stopwatch.ElapsedMilliseconds)
        $result = Invoke-OmeExternalText -FilePath $adb -ArgumentList @('-s', $Serial, 'shell', 'getprop', 'sys.boot_completed') -TimeoutMs ([Math]::Min($remainingMs, 15000))
        if ($result.ExitCode -eq 0 -and $result.StdOut.Trim() -ceq '1') {
            return $true
        }
        Start-Sleep -Seconds 2
    }
    throw "Guest $Serial did not report sys.boot_completed=1 within $TimeoutSec seconds."
}

<#
.SYNOPSIS
Builds an OME QEMU argument array.
.DESCRIPTION
Maps guest options to discrete QEMU option and value elements.
.PARAMETER Name
Guest name.
.PARAMETER DiskPath
Guest qcow2 path.
.PARAMETER FirmwareCode
Read-only pflash code path.
.PARAMETER FirmwareVars
Writable pflash variable-store path.
.PARAMETER Cdrom
Attaches installer media.
.PARAMETER IsoPath
Installer ISO path.
.PARAMETER Gpu
std, virtio, or virgl.
.PARAMETER Accel
whpx or tcg.
.PARAMETER Cpu
QEMU CPU model.
.PARAMETER MemoryMB
Guest memory in MiB.
.PARAMETER Smp
Virtual CPU count.
.PARAMETER QmpPort
Loopback QMP port.
.PARAMETER AdbPort
Loopback adb forwarding port.
.PARAMETER Audio
Audio backend or none.
.PARAMETER Display
SDL or GTK display backend.
.PARAMETER ExtraArgs
Additional QEMU elements appended last.
.EXAMPLE
Get-OmeQemuArguments -DiskPath disk.qcow2 -FirmwareCode code.fd -FirmwareVars vars.fd
#>
function Get-OmeQemuArguments {
    [CmdletBinding()]
    param(
        [string]$Name = 'default',

        [Parameter(Mandatory)]
        [string]$DiskPath,

        [Parameter(Mandatory)]
        [string]$FirmwareCode,

        [Parameter(Mandatory)]
        [string]$FirmwareVars,

        [switch]$Cdrom,

        [string]$IsoPath,

        [ValidateSet('std', 'virtio', 'virgl')]
        [string]$Gpu = 'std',

        [ValidateSet('whpx', 'tcg')]
        [string]$Accel = 'whpx',

        [string]$Cpu = 'Skylake-Client-v4',

        [ValidateRange(128, 1048576)]
        [int]$MemoryMB = 8192,

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

        [string[]]$ExtraArgs = @()
    )

    if ($Cdrom -and [string]::IsNullOrWhiteSpace($IsoPath)) {
        throw 'IsoPath is required when Cdrom is selected.'
    }

    $arguments = [Collections.Generic.List[string]]::new()
    $arguments.Add('-name')
    $arguments.Add("OME $Name")
    $arguments.Add('-machine')
    $arguments.Add('q35')
    $arguments.Add('-accel')
    # kernel-irqchip=off with WHPX boots Bliss 16.9.7 on this host (docs/evidence/M0/guest-install.md).
    $arguments.Add($(if ($Accel -ceq 'whpx') { 'whpx,kernel-irqchip=off' } else { 'tcg,thread=multi' }))
    $arguments.Add('-cpu')
    $arguments.Add($Cpu)
    $arguments.Add('-m')
    $arguments.Add([string]$MemoryMB)
    $arguments.Add('-smp')
    $arguments.Add([string]$Smp)
    $arguments.Add('-drive')
    $arguments.Add("if=pflash,format=raw,readonly=on,file=$FirmwareCode")
    $arguments.Add('-drive')
    $arguments.Add("if=pflash,format=raw,file=$FirmwareVars")
    $arguments.Add('-drive')
    $arguments.Add("file=$DiskPath,if=virtio,format=qcow2")

    if ($Cdrom) {
        $arguments.Add('-drive')
        $arguments.Add("file=$IsoPath,media=cdrom,if=none,id=cd0")
        $arguments.Add('-device')
        $arguments.Add('ide-cd,drive=cd0,bootindex=0,bus=ide.0')
    }

    $arguments.Add('-device')
    switch ($Gpu) {
        'std' { $arguments.Add('VGA') }
        'virtio' { $arguments.Add('virtio-vga') }
        # edid=off: QEMU's generated EDID advertises a 75 Hz mode, which pins the
        # guest's vsync to 13.3 ms and the tested game to 38 fps. Without the EDID the
        # guest driver falls back to a 60 Hz CVT mode and the game presents 58 fps on
        # a 60 Hz host monitor (docs/evidence/M0/findings-20260926.md, 2026-09-26).
        'virgl' { $arguments.Add('virtio-vga-gl,edid=off') }
    }

    $displayValue = "$Display,show-cursor=on"
    if ($Gpu -ceq 'virgl') {
        $displayValue += ',gl=on'
    }
    $arguments.Add('-display')
    $arguments.Add($displayValue)
    $arguments.Add('-device')
    $arguments.Add('virtio-net-pci,netdev=n0')
    $arguments.Add('-netdev')
    $arguments.Add("user,id=n0,hostfwd=tcp:127.0.0.1:$AdbPort-:5555")
    $arguments.Add('-usb')
    $arguments.Add('-device')
    $arguments.Add('usb-tablet')
    $arguments.Add('-device')
    $arguments.Add('usb-kbd')

    if ($Audio -cne 'none') {
        $arguments.Add('-audiodev')
        $arguments.Add("$Audio,id=snd0")
        $arguments.Add('-device')
        $arguments.Add('intel-hda')
        $arguments.Add('-device')
        $arguments.Add('hda-duplex,audiodev=snd0')
    }

    $arguments.Add('-qmp')
    $arguments.Add("tcp:127.0.0.1:$QmpPort,server=on,wait=off")
    # A guest-initiated reset under WHPX fails in QEMU 11.1 ("failed to get xsave state",
    # then "WHPX: Unexpected VP exit code 4") and leaves the VM paused with QMP unreachable,
    # observed with both -cpu max and Skylake-Client-v4 (docs/evidence/M0/guest-install.md).
    # Turning the reset into a QEMU exit lets the launcher (or the M2 supervisor) restart it.
    $arguments.Add('-action')
    $arguments.Add('reboot=shutdown')
    $arguments.Add('-rtc')
    $arguments.Add('base=utc')
    # No serial0/parallel0 virtual consoles: their hidden SDL GL windows can leave the thread
    # without a current GL context and blank the display (docs/evidence/M2/embedded-display-freeze.md).
    $arguments.Add('-serial')
    $arguments.Add('none')
    $arguments.Add('-parallel')
    $arguments.Add('none')
    foreach ($extraArgument in $ExtraArgs) {
        $arguments.Add($extraArgument)
    }
    return $arguments.ToArray()
}

Export-ModuleMember -Function @(
    'Get-OmeHome',
    'Get-OmePath',
    'Get-OmeRepoRoot',
    'Get-OmeManifest',
    'Get-OmeArtifact',
    'Test-OmeAllowedUrl',
    'Find-OmeQemu',
    'Find-OmeFirmware',
    'Find-OmeAdb',
    'Find-OmeApkSigner',
    'Write-OmeLog',
    'Invoke-OmeQmp',
    'ConvertTo-OmeQmpTouchEvent',
    'Measure-OmeQmpLatency',
    'Convert-OmePpmToPng',
    'Save-OmeQmpScreenshot',
    'Wait-OmeAdbBoot',
    'Get-OmeQemuArguments'
)
