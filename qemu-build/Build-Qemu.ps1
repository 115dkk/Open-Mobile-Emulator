# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0
<#
.SYNOPSIS
Builds the OME QEMU distribution in MSYS2 UCRT64.
.DESCRIPTION
Runs one step or all, streaming output. Uses a temporary SUBST drive for paths
with spaces. Does not change Windows virtualization settings.
.PARAMETER Step
all, deps, clone, configure, build or dist. Individual steps require prior outputs.
.PARAMETER Clean
Removes generated out and dist directories before running the requested step.
.PARAMETER IncludeSource
With Clean, also removes src. Source is retained unless both switches are set.
.EXAMPLE
pwsh -File .\qemu-build\Build-Qemu.ps1
.EXAMPLE
pwsh -File .\qemu-build\Build-Qemu.ps1 -Clean -IncludeSource
#>
[CmdletBinding(SupportsShouldProcess, ConfirmImpact = 'Medium')]
param(
    [ValidateSet('all', 'deps', 'clone', 'configure', 'build', 'dist')]
    [string]$Step = 'all',
    [switch]$Clean,
    [switch]$IncludeSource
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

if (-not $IsWindows) { throw 'This wrapper requires Windows.' }
if ($IncludeSource -and -not $Clean) { throw '-IncludeSource requires -Clean.' }
$shell = 'C:\msys64\msys2_shell.cmd'
if (-not (Test-Path -LiteralPath $shell -PathType Leaf)) {
    throw 'MSYS2 is missing. Complete its installation in C:\msys64 first.'
}
if ($Clean) {
    $names = @('out', 'dist')
    if ($IncludeSource) { $names += 'src' }
    foreach ($name in $names) {
        $target = Join-Path $PSScriptRoot $name
        if (-not (Test-Path -LiteralPath $target)) { continue }
        $item = Get-Item -LiteralPath $target -Force
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Refusing to clean a reparse point: $target"
        }
        if ($PSCmdlet.ShouldProcess($target, 'Remove generated build directory')) {
            Remove-Item -LiteralPath $target -Recurse -Force
        } else { return }
    }
}
if (-not $PSCmdlet.ShouldProcess($PSScriptRoot, "Run QEMU build step '$Step'")) { return }

$drive = $null
$exitCode = 1
$previousRepositoryRoot = $env:OME_REPOSITORY_ROOT
$env:OME_REPOSITORY_ROOT = Split-Path -Parent $PSScriptRoot
try {
    $workDirectory = $PSScriptRoot
    # https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/configure rejects whitespace.
    # https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/subst
    if ($workDirectory -match '\s') {
        $stateDirectory = Join-Path $PSScriptRoot 'out'
        $driveFile = Join-Path $stateDirectory 'subst-drive.txt'
        if (Test-Path -LiteralPath $driveFile) {
            $candidate = (Get-Content -LiteralPath $driveFile -Raw).Trim()
            if ($candidate -notmatch '^[D-Z]:$') { throw 'Invalid saved SUBST drive.' }
            if (Test-Path "$candidate\") { throw "Build drive $candidate is occupied. Free it or use -Clean." }
        } else {
            $candidate = $null
            foreach ($letter in 90..68) {
                $name = '{0}:' -f [char]$letter
                if (-not (Test-Path "$name\")) { $candidate = $name; break }
            }
            if ($null -eq $candidate) { throw 'No free drive letter for the build.' }
        }
        & subst.exe $candidate $PSScriptRoot
        if ($LASTEXITCODE -ne 0) { throw 'Could not create the temporary build drive.' }
        $drive = $candidate
        New-Item -ItemType Directory -Path $stateDirectory -Force | Out-Null
        [IO.File]::WriteAllText($driveFile, "$drive`n", [Text.UTF8Encoding]::new($false))
        $workDirectory = "$drive\"
    }
    Push-Location -LiteralPath $workDirectory
    try {
        # Verified in C:\msys64\msys2_shell.cmd and https://www.msys2.org/docs/terminals/
        & $shell -ucrt64 -defterm -no-start -here -c "./build-qemu.sh $Step"
        $exitCode = $LASTEXITCODE
    } finally { Pop-Location }
} finally {
    $env:OME_REPOSITORY_ROOT = $previousRepositoryRoot
    if ($null -ne $drive) {
        & subst.exe $drive /d
        if ($LASTEXITCODE -ne 0) { Write-Warning "Could not release temporary drive $drive" }
    }
}
exit $exitCode
