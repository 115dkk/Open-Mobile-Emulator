# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Checks source files for the R5 SPDX license header.

.DESCRIPTION
Selects staged files, explicit paths, or all tracked and unignored untracked
files. Checked source extensions must identify GPL-2.0-or-later in their first
five lines. AOSP-derived files in the two designated directories may instead
identify Apache-2.0.

.PARAMETER Staged
Checks only added, copied, modified, or renamed paths in the Git index.

.PARAMETER Path
Checks only the supplied repository-relative or absolute paths.
#>
[CmdletBinding()]
param(
    [switch]$Staged,
    [string[]]$Path
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

function Find-RepositoryRoot {
    $startDirectory = Split-Path -Parent $PSScriptRoot
    $probe = Invoke-GitCommand -WorkingDirectory $startDirectory -ArgumentList @(
        'rev-parse', '--show-toplevel'
    )
    if ($probe.ExitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($probe.StdOut)) {
        return [System.IO.Path]::GetFullPath($probe.StdOut.Trim())
    }
    throw "Could not find the Git repository containing $PSScriptRoot."
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

    $repoRoot = Find-RepositoryRoot
    $candidatePaths = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    if ($Staged) {
        $gitResult = Invoke-GitCommand -WorkingDirectory $repoRoot -ArgumentList @(
            'diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z'
        )
        if ($gitResult.ExitCode -ne 0) {
            throw "git diff failed: $($gitResult.StdErr.Trim())"
        }
        foreach ($candidate in ConvertFrom-NulList $gitResult.StdOut) {
            [void]$candidatePaths.Add($candidate.Replace('\', '/'))
        }
    }
    elseif ($PSBoundParameters.ContainsKey('Path')) {
        foreach ($candidate in $Path) {
            if ([string]::IsNullOrWhiteSpace($candidate)) {
                throw 'A value passed to -Path is empty.'
            }
            [void]$candidatePaths.Add((ConvertTo-RepositoryPath $repoRoot $candidate))
        }
    }
    else {
        foreach ($gitArguments in @(
                @('ls-files', '-z'),
                @('ls-files', '-o', '--exclude-standard', '-z')
            )) {
            $gitResult = Invoke-GitCommand -WorkingDirectory $repoRoot -ArgumentList $gitArguments
            if ($gitResult.ExitCode -ne 0) {
                throw "git ls-files failed: $($gitResult.StdErr.Trim())"
            }
            foreach ($candidate in ConvertFrom-NulList $gitResult.StdOut) {
                [void]$candidatePaths.Add($candidate.Replace('\', '/'))
            }
        }
    }

    $checkedExtensions = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    foreach ($extension in @(
            '.ps1', '.psm1', '.psd1', '.sh', '.bash', '.py', '.c', '.h',
            '.cpp', '.hpp', '.rs', '.cs', '.java', '.kt', '.js', '.ts',
            '.tsx', '.mjs', '.css', '.toml', '.yml', '.yaml'
        )) {
        [void]$checkedExtensions.Add($extension)
    }

    $findings = 0
    $checked = 0
    foreach ($candidate in @($candidatePaths) | Sort-Object) {
        if (
            $candidate -eq 'LICENSE' -or
            $candidate -eq '.git' -or
            $candidate.StartsWith('.git/', [System.StringComparison]::OrdinalIgnoreCase)
        ) {
            continue
        }
        if (-not $checkedExtensions.Contains([System.IO.Path]::GetExtension($candidate))) {
            continue
        }

        $ignoreResult = Invoke-GitCommand -WorkingDirectory $repoRoot -ArgumentList @(
            'check-ignore', '-q', '--', $candidate
        )
        if ($ignoreResult.ExitCode -eq 0) {
            continue
        }
        if ($ignoreResult.ExitCode -ne 1) {
            throw "git check-ignore failed for $candidate`: $($ignoreResult.StdErr.Trim())"
        }

        if ($Staged) {
            $blobResult = Invoke-GitCommand -WorkingDirectory $repoRoot -ArgumentList @(
                'show', "`:$candidate"
            )
            if ($blobResult.ExitCode -ne 0) {
                throw "Could not read staged content for $candidate`: $($blobResult.StdErr.Trim())"
            }
            $firstLines = @(
                $blobResult.StdOut -split "`r`n|`n|`r" |
                    Select-Object -First 5
            )
        }
        else {
            $fullPath = Join-Path $repoRoot $candidate
            if (-not [System.IO.File]::Exists($fullPath)) {
                throw "Source file does not exist: $candidate"
            }
            $firstLines = @(Get-Content -LiteralPath $fullPath -TotalCount 5)
        }
        $spdxLines = @($firstLines | Where-Object { $_ -match 'SPDX-License-Identifier:\s*\S+' })
        $isApacheAllowed =
            $candidate.StartsWith('guest/overlay/', [System.StringComparison]::OrdinalIgnoreCase) -or
            $candidate.StartsWith('translator/bundle-template/', [System.StringComparison]::OrdinalIgnoreCase)
        $hasGpl = @($firstLines | Where-Object {
                $_ -match 'SPDX-License-Identifier:\s*GPL-2\.0-or-later\s*(?:\*/)?\s*$'
            }).Count -gt 0
        $hasApache = @($firstLines | Where-Object {
                $_ -match 'SPDX-License-Identifier:\s*Apache-2\.0\s*(?:\*/)?\s*$'
            }).Count -gt 0

        $checked++
        if ($hasGpl -or ($isApacheAllowed -and $hasApache)) {
            continue
        }
        $problem = if ($spdxLines.Count -eq 0) { 'missing' } else { 'wrong-license' }
        Write-Output "SPDX $candidate $problem"
        $findings++
    }

    Write-Output "SUMMARY spdx findings=$findings checked=$checked candidates=$($candidatePaths.Count)"
    if ($findings -gt 0) {
        exit 1
    }
    exit 0
}
catch {
    [System.Console]::Error.WriteLine("ERROR Check-SpdxHeaders: $($_.Exception.Message)")
    exit 2
}
