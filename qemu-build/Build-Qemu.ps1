# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0
<#
.SYNOPSIS
Builds the OME QEMU distribution in MSYS2 UCRT64.
.DESCRIPTION
Runs one build step or all of them, streaming output. QEMU's configure and Meson
reject whitespace in source and build paths, and Meson resolves a SUBST drive
back to the real path, so a substituted drive letter does not help. When the
repository path contains whitespace the work tree (src, out, dist) is therefore
placed under %LOCALAPPDATA%\OpenMobileEmulator\qemu-build (override with
-WorkRoot or the OME_QEMU_BUILD_WORK environment variable), and qemu-build\out
inside the repository becomes a junction to the work tree's out\ so the launcher
still finds qemu-build\out\bin\qemu-system-x86_64.exe.
Does not change Windows virtualization settings.
.PARAMETER Step
all, deps, clone, configure, build, dist, thirdparty or sourceoffer. Individual steps
require prior outputs; thirdparty and sourceoffer need a finished dist.
.PARAMETER Clean
Removes the generated out and dist directories in the work tree before running.
.PARAMETER IncludeSource
With Clean, also removes src. Source is retained unless both switches are set.
.PARAMETER WorkRoot
Whitespace-free directory for src, out and dist. Defaults are described above.
.EXAMPLE
pwsh -File .\qemu-build\Build-Qemu.ps1
.EXAMPLE
pwsh -File .\qemu-build\Build-Qemu.ps1 -Clean -IncludeSource
#>
[CmdletBinding(SupportsShouldProcess, ConfirmImpact = 'Medium')]
param(
    [ValidateSet('all', 'deps', 'clone', 'configure', 'build', 'dist', 'thirdparty', 'sourceoffer')]
    [string]$Step = 'all',
    [switch]$Clean,
    [switch]$IncludeSource,
    [string]$WorkRoot
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

# Resolve the work tree.
if (-not $WorkRoot) {
    if ($env:OME_QEMU_BUILD_WORK) {
        $WorkRoot = $env:OME_QEMU_BUILD_WORK
    } elseif ($PSScriptRoot -match '\s') {
        $WorkRoot = Join-Path $env:LOCALAPPDATA 'OpenMobileEmulator\qemu-build'
    } else {
        $WorkRoot = $PSScriptRoot
    }
}
if ($WorkRoot -match '\s') {
    throw "The work tree path must not contain whitespace: $WorkRoot (use -WorkRoot or OME_QEMU_BUILD_WORK)."
}
$WorkRoot = [IO.Path]::GetFullPath($WorkRoot)
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null
$sameTree = $WorkRoot.TrimEnd('\') -ieq $PSScriptRoot.TrimEnd('\')

function Remove-Tree {
    param([Parameter(Mandatory)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        # A junction: remove the link itself, never its target.
        [IO.Directory]::Delete($Path)
        return
    }
    Remove-Item -LiteralPath $Path -Recurse -Force
}

if ($Clean) {
    $names = @('out', 'dist')
    if ($IncludeSource) { $names += 'src' }
    foreach ($name in $names) {
        $target = Join-Path $WorkRoot $name
        if (-not (Test-Path -LiteralPath $target)) { continue }
        if ($PSCmdlet.ShouldProcess($target, 'Remove generated build directory')) {
            Remove-Tree -Path $target
        } else { return }
    }
    if (-not $sameTree) { Remove-Tree -Path (Join-Path $PSScriptRoot 'out') }
}
if (-not $PSCmdlet.ShouldProcess($WorkRoot, "Run QEMU build step '$Step'")) { return }

$exitCode = 1
$previousWork = $env:OME_QEMU_BUILD_WORK
$previousRepositoryRoot = $env:OME_REPOSITORY_ROOT
$env:OME_QEMU_BUILD_WORK = $WorkRoot
$env:OME_REPOSITORY_ROOT = Split-Path -Parent $PSScriptRoot
try {
    Push-Location -LiteralPath $PSScriptRoot
    try {
        # Flags verified in C:\msys64\msys2_shell.cmd and https://www.msys2.org/docs/terminals/
        & $shell -ucrt64 -defterm -no-start -here -c "./build-qemu.sh $Step"
        $exitCode = $LASTEXITCODE
    } finally { Pop-Location }
} finally {
    $env:OME_QEMU_BUILD_WORK = $previousWork
    $env:OME_REPOSITORY_ROOT = $previousRepositoryRoot
}

# Expose the work tree's out\ inside the repository for the launcher (git-ignored path).
if (-not $sameTree) {
    $link = Join-Path $PSScriptRoot 'out'
    $target = Join-Path $WorkRoot 'out'
    if ((Test-Path -LiteralPath $target) -and -not (Test-Path -LiteralPath $link)) {
        New-Item -ItemType Junction -Path $link -Target $target | Out-Null
        Write-Host "Junction created: $link -> $target"
    }
}
exit $exitCode
