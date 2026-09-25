# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Checks repository paths against the R1 forbidden-file patterns.

.DESCRIPTION
Reads ci/forbidden-patterns.txt and ci/allowlist.txt, selects staged files,
explicit paths, or all tracked and unignored untracked files, and reports each
forbidden path that is not explicitly allowlisted.

.PARAMETER Staged
Checks only added, copied, modified, or renamed paths in the Git index.

.PARAMETER Path
Checks only the supplied repository-relative or absolute paths.

.PARAMETER RepoRoot
Specifies the repository root. The default is the parent of this script folder.
#>
[CmdletBinding()]
param(
    [switch]$Staged,
    [string[]]$Path,
    [string]$RepoRoot = (Split-Path -Parent $PSScriptRoot)
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-GitCommand {
    param(
        [Parameter(Mandatory)]
        [string]$WorkingDirectory,
        [Parameter(Mandatory)]
        [string[]]$ArgumentList
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

function ConvertTo-RepositoryPath {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$InputPath
    )

    $fullPath = if ([System.IO.Path]::IsPathRooted($InputPath)) {
        [System.IO.Path]::GetFullPath($InputPath)
    }
    else {
        [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($Root, $InputPath))
    }
    $relativePath = [System.IO.Path]::GetRelativePath($Root, $fullPath)
    if (
        [System.IO.Path]::IsPathRooted($relativePath) -or
        $relativePath -eq '..' -or
        $relativePath.StartsWith("..$([System.IO.Path]::DirectorySeparatorChar)", [System.StringComparison]::Ordinal)
    ) {
        throw "Path is outside the repository: $InputPath"
    }
    return $relativePath.Replace([System.IO.Path]::DirectorySeparatorChar, '/')
}

try {
    if ($Staged -and $PSBoundParameters.ContainsKey('Path')) {
        throw 'Use either -Staged or -Path, not both.'
    }

    $resolvedRoot = [System.IO.Path]::GetFullPath($RepoRoot)
    if (-not [System.IO.Directory]::Exists($resolvedRoot)) {
        throw "Repository root does not exist: $resolvedRoot"
    }
    $gitProbe = Invoke-GitCommand -WorkingDirectory $resolvedRoot -ArgumentList @(
        'rev-parse', '--is-inside-work-tree'
    )
    if ($gitProbe.ExitCode -ne 0 -or $gitProbe.StdOut.Trim() -ne 'true') {
        throw "Not a Git work tree: $resolvedRoot. $($gitProbe.StdErr.Trim())"
    }

    $patternsPath = Join-Path $resolvedRoot 'ci/forbidden-patterns.txt'
    $allowlistPath = Join-Path $resolvedRoot 'ci/allowlist.txt'
    if (-not [System.IO.File]::Exists($patternsPath)) {
        throw "Pattern file is missing: $patternsPath"
    }
    if (-not [System.IO.File]::Exists($allowlistPath)) {
        throw "Allowlist file is missing: $allowlistPath"
    }

    $patterns = @(
        Get-Content -LiteralPath $patternsPath |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ -and -not $_.StartsWith('#', [System.StringComparison]::Ordinal) }
    )
    if ($patterns.Count -eq 0) {
        throw "Pattern file contains no patterns: $patternsPath"
    }

    $allowlist = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    foreach ($line in Get-Content -LiteralPath $allowlistPath) {
        $entry = ($line -split '#', 2)[0].Trim().Replace('\', '/')
        if ($entry) {
            [void]$allowlist.Add($entry)
        }
    }

    $candidatePaths = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    if ($Staged) {
        $gitResult = Invoke-GitCommand -WorkingDirectory $resolvedRoot -ArgumentList @(
            'diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z'
        )
        if ($gitResult.ExitCode -ne 0) {
            throw "git diff failed: $($gitResult.StdErr.Trim())"
        }
        foreach ($candidate in ConvertFrom-NulList -Value $gitResult.StdOut) {
            [void]$candidatePaths.Add($candidate.Replace('\', '/'))
        }
    }
    elseif ($PSBoundParameters.ContainsKey('Path')) {
        foreach ($candidate in $Path) {
            if ([string]::IsNullOrWhiteSpace($candidate)) {
                throw 'A value passed to -Path is empty.'
            }
            [void]$candidatePaths.Add((ConvertTo-RepositoryPath -Root $resolvedRoot -InputPath $candidate))
        }
    }
    else {
        foreach ($gitArguments in @(
                @('ls-files', '-z'),
                @('ls-files', '-o', '--exclude-standard', '-z')
            )) {
            $gitResult = Invoke-GitCommand -WorkingDirectory $resolvedRoot -ArgumentList $gitArguments
            if ($gitResult.ExitCode -ne 0) {
                throw "git ls-files failed: $($gitResult.StdErr.Trim())"
            }
            foreach ($candidate in ConvertFrom-NulList -Value $gitResult.StdOut) {
                [void]$candidatePaths.Add($candidate.Replace('\', '/'))
            }
        }
    }

    $findings = 0
    foreach ($candidate in @($candidatePaths) | Sort-Object) {
        if ($allowlist.Contains($candidate)) {
            continue
        }
        $fileName = ($candidate -split '/')[-1]
        foreach ($pattern in $patterns) {
            if ($fileName -ilike $pattern -or $candidate -ilike $pattern) {
                Write-Output "FORBIDDEN $pattern $candidate"
                $findings++
                break
            }
        }
    }

    Write-Output "SUMMARY forbidden findings=$findings candidates=$($candidatePaths.Count)"
    if ($findings -gt 0) {
        exit 1
    }
    exit 0
}
catch {
    [System.Console]::Error.WriteLine("ERROR Check-Forbidden: $($_.Exception.Message)")
    exit 2
}
