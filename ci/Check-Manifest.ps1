# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7.0

<#
.SYNOPSIS
Validates the R2 external-artifact manifest.

.DESCRIPTION
Checks the manifest schema version, allowed download hosts, required artifact
fields, SHA-256 values, fetch roles, URL hosts, sizes, and unique names.

.PARAMETER Path
Specifies the manifest JSON file. A relative path is resolved from the
repository root.
#>
[CmdletBinding()]
param(
    [string]$Path = 'manifests/artifacts.json'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-JsonProperty {
    param(
        [Parameter(Mandatory)][System.Text.Json.JsonElement]$Object,
        [Parameter(Mandatory)][string]$Name
    )

    foreach ($property in $Object.EnumerateObject()) {
        if ($property.Name -ceq $Name) {
            return [pscustomobject]@{ Found = $true; Value = $property.Value.Clone() }
        }
    }
    return [pscustomobject]@{ Found = $false; Value = $null }
}

function Add-ManifestProblem {
    param(
        [Parameter(Mandatory)]
        [AllowEmptyCollection()]
        [System.Collections.Generic.List[string]]$Problems,
        [Parameter(Mandatory)][string]$Artifact,
        [Parameter(Mandatory)][string]$Problem
    )

    [void]$Problems.Add("MANIFEST $Artifact $Problem")
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$manifestPath = if ([System.IO.Path]::IsPathRooted($Path)) {
    [System.IO.Path]::GetFullPath($Path)
}
else {
    [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($repoRoot, $Path))
}
if (-not [System.IO.File]::Exists($manifestPath)) {
    [System.Console]::Error.WriteLine("ERROR Check-Manifest: File does not exist: $manifestPath")
    exit 2
}

try {
    $jsonText = [System.IO.File]::ReadAllText($manifestPath)
    $document = [System.Text.Json.JsonDocument]::Parse($jsonText)
}
catch [System.Text.Json.JsonException] {
    $message = $_.Exception.Message.Replace("`r", ' ').Replace("`n", ' ')
    Write-Output "MANIFEST manifest invalid JSON: $message"
    Write-Output 'SUMMARY manifest findings=1 artifacts=0'
    exit 1
}
catch {
    [System.Console]::Error.WriteLine("ERROR Check-Manifest: $($_.Exception.Message)")
    exit 2
}

try {
    $problems = [System.Collections.Generic.List[string]]::new()
    $root = $document.RootElement
    if ($root.ValueKind -ne [System.Text.Json.JsonValueKind]::Object) {
        Add-ManifestProblem $problems 'manifest' 'top level must be an object'
        foreach ($problem in $problems) {
            Write-Output $problem
        }
        Write-Output "SUMMARY manifest findings=$($problems.Count) artifacts=0"
        exit 1
    }

    $schemaProperty = Get-JsonProperty $root 'schema_version'
    $schemaVersion = 0
    if (
        -not $schemaProperty.Found -or
        $schemaProperty.Value.ValueKind -ne [System.Text.Json.JsonValueKind]::Number -or
        -not $schemaProperty.Value.TryGetInt32([ref]$schemaVersion) -or
        $schemaVersion -ne 1
    ) {
        Add-ManifestProblem $problems 'manifest' 'schema_version must be 1'
    }

    $validAllowedHosts = [System.Collections.Generic.List[string]]::new()
    $allowedHostsProperty = Get-JsonProperty $root 'allowed_hosts'
    if (
        -not $allowedHostsProperty.Found -or
        $allowedHostsProperty.Value.ValueKind -ne [System.Text.Json.JsonValueKind]::Array -or
        $allowedHostsProperty.Value.GetArrayLength() -eq 0
    ) {
        Add-ManifestProblem $problems 'manifest' 'allowed_hosts must be a non-empty array'
    }
    else {
        $hostIndex = 0
        foreach ($hostElement in $allowedHostsProperty.Value.EnumerateArray()) {
            $hostName = if ($hostElement.ValueKind -eq [System.Text.Json.JsonValueKind]::String) {
                $hostElement.GetString()
            }
            else {
                $null
            }
            if (
                [string]::IsNullOrWhiteSpace($hostName) -or
                $hostName -cne $hostName.ToLowerInvariant() -or
                $hostName -notmatch '^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)*$'
            ) {
                Add-ManifestProblem $problems 'manifest' "allowed_hosts[$hostIndex] must be a lowercase host name"
            }
            else {
                [void]$validAllowedHosts.Add($hostName)
            }
            $hostIndex++
        }
    }

    $artifactsProperty = Get-JsonProperty $root 'artifacts'
    $artifactCount = 0
    if (
        -not $artifactsProperty.Found -or
        $artifactsProperty.Value.ValueKind -ne [System.Text.Json.JsonValueKind]::Array
    ) {
        Add-ManifestProblem $problems 'manifest' 'artifacts must be an array'
    }
    else {
        $artifactCount = $artifactsProperty.Value.GetArrayLength()
        $seenNames = [System.Collections.Generic.HashSet[string]]::new(
            [System.StringComparer]::Ordinal
        )
        $artifactIndex = 0
        foreach ($artifactElement in $artifactsProperty.Value.EnumerateArray()) {
            $artifactLabel = "artifact[$artifactIndex]"
            if ($artifactElement.ValueKind -ne [System.Text.Json.JsonValueKind]::Object) {
                Add-ManifestProblem $problems $artifactLabel 'must be an object'
                $artifactIndex++
                continue
            }

            $nameProperty = Get-JsonProperty $artifactElement 'name'
            if (
                $nameProperty.Found -and
                $nameProperty.Value.ValueKind -eq [System.Text.Json.JsonValueKind]::String -and
                -not [string]::IsNullOrWhiteSpace($nameProperty.Value.GetString())
            ) {
                $artifactLabel = $nameProperty.Value.GetString()
            }

            $fieldValues = @{}
            foreach ($fieldName in @(
                    'name', 'version', 'filename', 'url', 'sha256', 'license',
                    'provenance_note', 'fetched_by'
                )) {
                $fieldProperty = Get-JsonProperty $artifactElement $fieldName
                if (
                    -not $fieldProperty.Found -or
                    $fieldProperty.Value.ValueKind -ne [System.Text.Json.JsonValueKind]::String -or
                    [string]::IsNullOrWhiteSpace($fieldProperty.Value.GetString())
                ) {
                    Add-ManifestProblem $problems $artifactLabel "$fieldName must be a non-empty string"
                    $fieldValues[$fieldName] = $null
                }
                else {
                    $fieldValues[$fieldName] = $fieldProperty.Value.GetString()
                }
            }

            if ($null -ne $fieldValues['name'] -and -not $seenNames.Add($fieldValues['name'])) {
                Add-ManifestProblem $problems $artifactLabel 'name must be unique'
            }
            if (
                $null -ne $fieldValues['sha256'] -and
                $fieldValues['sha256'] -cnotmatch '^[0-9a-f]{64}$'
            ) {
                Add-ManifestProblem $problems $artifactLabel 'sha256 must be exactly 64 lowercase hexadecimal characters'
            }
            if (
                $null -ne $fieldValues['fetched_by'] -and
                $fieldValues['fetched_by'] -cnotin @('installer', 'builder')
            ) {
                Add-ManifestProblem $problems $artifactLabel 'fetched_by must be installer or builder'
            }

            if ($null -ne $fieldValues['url']) {
                $uri = $null
                if (
                    -not [System.Uri]::TryCreate($fieldValues['url'], [System.UriKind]::Absolute, [ref]$uri) -or
                    $uri.Scheme -cne 'https' -or
                    [string]::IsNullOrWhiteSpace($uri.Host)
                ) {
                    Add-ManifestProblem $problems $artifactLabel 'url must be an absolute HTTPS URL'
                }
                else {
                    $hostAllowed = $false
                    foreach ($allowedHost in $validAllowedHosts) {
                        if (
                            $uri.DnsSafeHost -ieq $allowedHost -or
                            $uri.DnsSafeHost.EndsWith(".$allowedHost", [System.StringComparison]::OrdinalIgnoreCase)
                        ) {
                            $hostAllowed = $true
                            break
                        }
                    }
                    if (-not $hostAllowed) {
                        Add-ManifestProblem $problems $artifactLabel "url host is not allowed: $($uri.DnsSafeHost)"
                    }
                }
            }

            $sizeProperty = Get-JsonProperty $artifactElement 'size_bytes'
            if ($sizeProperty.Found) {
                $sizeValue = 0L
                if (
                    $sizeProperty.Value.ValueKind -ne [System.Text.Json.JsonValueKind]::Number -or
                    -not $sizeProperty.Value.TryGetInt64([ref]$sizeValue) -or
                    $sizeValue -le 0
                ) {
                    Add-ManifestProblem $problems $artifactLabel 'size_bytes must be a positive integer'
                }
            }
            $artifactIndex++
        }
    }

    foreach ($problem in $problems) {
        Write-Output $problem
    }
    Write-Output "SUMMARY manifest findings=$($problems.Count) artifacts=$artifactCount"
    if ($problems.Count -gt 0) {
        exit 1
    }
    exit 0
}
catch {
    [System.Console]::Error.WriteLine("ERROR Check-Manifest: $($_.Exception.Message)")
    exit 2
}
finally {
    $document.Dispose()
}
