# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Entry point of the completion driver when the workflow starts it as the standard local user it
# created (m2-dod.yml, "Run fresh-home wizard"). Hosted runners run every job as an administrator
# with UAC off, which is not how anyone runs the product and which keeps WebView2 150+ from opening
# its remote-debugging endpoint (docs/evidence/M2/dod-ci.md, runs 16 to 19).
#
# Pass -Installer to install the current-user NSIS package silently before the driver starts. This
# verifies the installed executable while retaining -App for development builds; -Installer wins
# when both are present.
#
# A process started with another user's credentials carries the caller's environment, and the shell
# folder lookup expands %USERPROFILE% from that environment, so CI run 20 saw the administrator's
# AppData here. The profile directory is therefore read from the machine's ProfileList for this
# user's SID (present once LOGON_WITH_PROFILE created it); when it is absent, -UserRoot (a folder the
# workflow created and granted) stands in. Everything the driver needs comes in as parameters.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Repo,
    [Parameter(Mandatory)][string]$Node,
    [Parameter(Mandatory)][string]$OmeHome,
    [string]$App = '',
    [string]$Installer = '',
    [Parameter(Mandatory)][string]$GameDir,
    [Parameter(Mandatory)][string]$Output,
    [Parameter(Mandatory)][string]$UserRoot
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
if ([string]::IsNullOrWhiteSpace($App) -and [string]::IsNullOrWhiteSpace($Installer)) {
    throw 'Give -App or -Installer'
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$administrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$sid = $identity.User.Value
$profileEntry = Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$sid" -ErrorAction SilentlyContinue
$profilePath = if ($profileEntry -and $profileEntry.ProfileImagePath -and (Test-Path -LiteralPath $profileEntry.ProfileImagePath)) { $profileEntry.ProfileImagePath } else { $null }
$facts = @{
    user = $identity.Name; sid = $sid; administrator = $administrator
    inheritedUserProfile = $env:USERPROFILE; profileListPath = $profileEntry.ProfileImagePath
    hiveLoaded = (Test-Path 'HKCU:\Volatile Environment'); session = (Get-Process -Id $PID).SessionId
    interactive = [Environment]::UserInteractive; node = $Node
    installer = $null; installerExitCode = $null; installedApp = $null
    installedHelper = $null; installedQemu = $null; uninstallEntry = $null
}
$env:USERPROFILE = if ($profilePath) { $profilePath } else { $UserRoot }
$env:LOCALAPPDATA = Join-Path $env:USERPROFILE 'AppData\Local'
$env:APPDATA = Join-Path $env:USERPROFILE 'AppData\Roaming'
$env:TEMP = Join-Path $env:LOCALAPPDATA 'Temp'
$env:TMP = $env:TEMP
$facts.profileUsed = $env:USERPROFILE
foreach ($directory in @($env:LOCALAPPDATA, $env:APPDATA, $env:TEMP)) { New-Item -ItemType Directory -Path $directory -Force | Out-Null }
$env:USERNAME = [Environment]::UserName
$env:HOMEDRIVE = [IO.Path]::GetPathRoot($env:USERPROFILE).TrimEnd('\')
$env:HOMEPATH = $env:USERPROFILE.Substring($env:HOMEDRIVE.Length)
$env:PATH = (Split-Path -Parent $Node) + ';' + $env:PATH
if (-not [string]::IsNullOrWhiteSpace($Installer)) {
    $facts.installer = $Installer
    $process = Start-Process -FilePath $Installer -ArgumentList '/S' -Wait -PassThru
    $facts.installerExitCode = $process.ExitCode
    if ($process.ExitCode -ne 0) {
        throw "installer exit code $($process.ExitCode)"
    }

    $installed = Join-Path $env:LOCALAPPDATA 'Open Mobile Emulator\ome.exe'
    if (-not (Test-Path -LiteralPath $installed -PathType Leaf)) {
        $discovered = Get-ChildItem -LiteralPath $env:LOCALAPPDATA -Recurse -Filter 'ome.exe' `
            -File -Depth 3 -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($null -eq $discovered) {
            throw "installed app not found below $($env:LOCALAPPDATA)"
        }
        $installed = $discovered.FullName
    }

    $installDirectory = Split-Path -Parent $installed
    $facts.installedApp = $installed
    $facts.installedHelper = Test-Path -LiteralPath (Join-Path $installDirectory 'ome-setup.exe') -PathType Leaf
    $facts.installedQemu = Test-Path -LiteralPath (Join-Path $installDirectory 'qemu\bin\qemu-system-x86_64.exe') -PathType Leaf
    $uninstall = Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*' `
        -ErrorAction SilentlyContinue | Where-Object {
            $displayName = $_.PSObject.Properties['DisplayName']
            $null -ne $displayName -and $displayName.Value -ceq 'Open Mobile Emulator'
        } | Select-Object -First 1
    if ($null -ne $uninstall) {
        $displayVersion = $uninstall.PSObject.Properties['DisplayVersion']
        $installLocation = $uninstall.PSObject.Properties['InstallLocation']
        $facts.uninstallEntry = @{
            DisplayVersion = if ($null -ne $displayVersion) { $displayVersion.Value } else { $null }
            InstallLocation = if ($null -ne $installLocation) { $installLocation.Value } else { $null }
        }
    }
    $App = $installed
}
$facts | ConvertTo-Json -Compress
[Console]::Out.Flush()
$env:OME_HOME = $OmeHome
$env:OME_DOD_APP = $App
$env:OME_DOD_GAME_APK_DIR = $GameDir
$env:OME_DOD_OUTPUT = $Output
Set-Location (Join-Path $Repo 'host')
& $Node (Join-Path $Repo 'ci\dod\run-dod.mjs')
exit $LASTEXITCODE
