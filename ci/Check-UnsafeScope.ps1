# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Checks the Rust unsafe-code scope established by ADR-0002.

.DESCRIPTION
Scans tracked and unignored untracked Rust files below host, enforces crate-root
attributes, and checks the workspace and platform-crate lint values. Comments are
removed before unsafe-token scanning. String literals may remain and therefore
may produce a conservative finding.

.PARAMETER RepositoryRoot
Overrides repository discovery for isolated policy tests.
#>
[CmdletBinding()]
param(
    [string]$RepositoryRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-GitCommand {
    param(
        [Parameter(Mandatory)][string]$WorkingDirectory,
        [Parameter(Mandatory)][string[]]$ArgumentList
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = 'git'
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.StandardOutputEncoding = [System.Text.UTF8Encoding]::new($false)
    $startInfo.StandardErrorEncoding = [System.Text.UTF8Encoding]::new($false)
    foreach ($argument in $ArgumentList) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw 'Failed to start git.'
    }
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $result = [pscustomobject]@{
        ExitCode = $process.ExitCode
        StdOut = $stdoutTask.GetAwaiter().GetResult()
        StdErr = $stderrTask.GetAwaiter().GetResult()
    }
    $process.Dispose()
    return $result
}

function ConvertFrom-NulList {
    param([AllowEmptyString()][string]$Value)

    if ([string]::IsNullOrEmpty($Value)) {
        return @()
    }
    return @($Value.Split([char[]]@([char]0), [System.StringSplitOptions]::RemoveEmptyEntries))
}

function Remove-RustComments {
    param([Parameter(Mandatory)][string]$Text)

    $builder = [System.Text.StringBuilder]::new($Text.Length)
    $index = 0
    $blockDepth = 0
    $inLine = $false
    $inString = $false
    $escaped = $false
    while ($index -lt $Text.Length) {
        $current = $Text[$index]
        $next = if ($index + 1 -lt $Text.Length) { $Text[$index + 1] } else { [char]0 }
        if ($inLine) {
            if ($current -eq "`n" -or $current -eq "`r") {
                $inLine = $false
                [void]$builder.Append($current)
            }
            else {
                [void]$builder.Append(' ')
            }
            $index++
            continue
        }
        if ($blockDepth -gt 0) {
            if ($current -eq '/' -and $next -eq '*') {
                $blockDepth++
                [void]$builder.Append('  ')
                $index += 2
                continue
            }
            if ($current -eq '*' -and $next -eq '/') {
                $blockDepth--
                [void]$builder.Append('  ')
                $index += 2
                continue
            }
            if ($current -eq "`n" -or $current -eq "`r") {
                [void]$builder.Append($current)
            }
            else {
                [void]$builder.Append(' ')
            }
            $index++
            continue
        }
        if ($inString) {
            [void]$builder.Append($current)
            if ($escaped) {
                $escaped = $false
            }
            elseif ($current -eq '\') {
                $escaped = $true
            }
            elseif ($current -eq '"') {
                $inString = $false
            }
            $index++
            continue
        }
        if ($current -eq '/' -and $next -eq '/') {
            $inLine = $true
            [void]$builder.Append('  ')
            $index += 2
            continue
        }
        if ($current -eq '/' -and $next -eq '*') {
            $blockDepth = 1
            [void]$builder.Append('  ')
            $index += 2
            continue
        }
        if ($current -eq '"') {
            $inString = $true
        }
        [void]$builder.Append($current)
        $index++
    }
    return $builder.ToString()
}

function Test-TomlLint {
    param(
        [Parameter(Mandatory)][string]$Content,
        [Parameter(Mandatory)][string]$Section,
        [Parameter(Mandatory)][string]$Expected
    )

    $sectionPattern = [regex]::Escape("[$Section]")
    $match = [regex]::Match(
        $Content,
        "(?ms)^\s*$sectionPattern\s*$\s*(?<body>.*?)(?=^\s*\[|\z)"
    )
    if (-not $match.Success) {
        return $false
    }
    return [regex]::IsMatch(
        $match.Groups['body'].Value,
        "(?m)^\s*unsafe_code\s*=\s*`"$Expected`"\s*(?:#.*)?$"
    )
}

try {
    $root = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
        $startDirectory = Split-Path -Parent $PSScriptRoot
        $probe = Invoke-GitCommand -WorkingDirectory $startDirectory -ArgumentList @(
            'rev-parse', '--show-toplevel'
        )
        if ($probe.ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($probe.StdOut)) {
            throw 'Could not find the repository root.'
        }
        [System.IO.Path]::GetFullPath($probe.StdOut.Trim())
    }
    else {
        [System.IO.Path]::GetFullPath($RepositoryRoot)
    }

    $candidatePaths = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    foreach ($arguments in @(
            @('ls-files', '-z', '--', 'host/*.rs', 'host/**/*.rs'),
            @('ls-files', '-o', '--exclude-standard', '-z', '--', 'host/*.rs', 'host/**/*.rs')
        )) {
        $result = Invoke-GitCommand -WorkingDirectory $root -ArgumentList $arguments
        if ($result.ExitCode -ne 0) {
            throw "git ls-files failed: $($result.StdErr.Trim())"
        }
        foreach ($candidate in ConvertFrom-NulList $result.StdOut) {
            if ($candidate.EndsWith('.rs', [System.StringComparison]::OrdinalIgnoreCase)) {
                [void]$candidatePaths.Add($candidate.Replace('\', '/'))
            }
        }
    }

    $findings = 0
    $checked = 0
    foreach ($candidate in @($candidatePaths) | Sort-Object) {
        $fullPath = Join-Path $root $candidate
        if (-not [System.IO.File]::Exists($fullPath)) {
            throw "Rust source does not exist: $candidate"
        }
        $content = Get-Content -LiteralPath $fullPath -Raw
        $withoutComments = Remove-RustComments $content
        if (
            [regex]::IsMatch($withoutComments, '\bunsafe\b') -and
            -not $candidate.StartsWith(
                'host/crates/ome-platform-win/src/ffi/',
                [System.StringComparison]::OrdinalIgnoreCase
            )
        ) {
            Write-Output "UNSAFE $candidate outside-ffi"
            $findings++
        }
        $checked++
    }

    $rootPaths = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    foreach ($candidate in $candidatePaths) {
        if (
            $candidate -match '^host/crates/[^/]+/src/(?:lib|main)\.rs$' -or
            $candidate -in @('host/app/src/lib.rs', 'host/app/src/main.rs', 'host/app/build.rs')
        ) {
            [void]$rootPaths.Add($candidate)
        }
    }
    foreach ($candidate in @($rootPaths) | Sort-Object) {
        $firstLines = @(Get-Content -LiteralPath (Join-Path $root $candidate) -TotalCount 5)
        if ($candidate -eq 'host/crates/ome-platform-win/src/lib.rs') {
            if (-not ($firstLines -match '^#!\[deny\(unsafe_code\)\]$')) {
                Write-Output "UNSAFE $candidate missing-deny"
                $findings++
            }
            $allContent = Get-Content -LiteralPath (Join-Path $root $candidate) -Raw
            if ($allContent -notmatch '(?m)^#!\[deny\(clippy::undocumented_unsafe_blocks\)\]$') {
                Write-Output "UNSAFE $candidate missing-undocumented-block-deny"
                $findings++
            }
        }
        elseif (-not ($firstLines -match '^#!\[forbid\(unsafe_code\)\]$')) {
            Write-Output "UNSAFE $candidate missing-forbid"
            $findings++
        }
    }

    $workspaceCargo = Join-Path $root 'host/Cargo.toml'
    if (-not [System.IO.File]::Exists($workspaceCargo)) {
        throw 'host/Cargo.toml does not exist.'
    }
    if (-not (Test-TomlLint (Get-Content -LiteralPath $workspaceCargo -Raw) 'workspace.lints.rust' 'forbid')) {
        Write-Output 'UNSAFE host/Cargo.toml workspace-lint'
        $findings++
    }
    $platformCargo = Join-Path $root 'host/crates/ome-platform-win/Cargo.toml'
    if (-not [System.IO.File]::Exists($platformCargo)) {
        throw 'ome-platform-win/Cargo.toml does not exist.'
    }
    if (-not (Test-TomlLint (Get-Content -LiteralPath $platformCargo -Raw) 'lints.rust' 'deny')) {
        Write-Output 'UNSAFE host/crates/ome-platform-win/Cargo.toml platform-lint'
        $findings++
    }

    Write-Output "SUMMARY unsafe-scope findings=$findings checked=$checked"
    if ($findings -gt 0) {
        exit 1
    }
    exit 0
}
catch {
    [System.Console]::Error.WriteLine("ERROR Check-UnsafeScope: $($_.Exception.Message)")
    exit 2
}
