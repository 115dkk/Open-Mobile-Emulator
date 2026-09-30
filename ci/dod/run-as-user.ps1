# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Entry point of the completion driver when the workflow starts it as the standard local user it
# created (m2-dod.yml, "Run fresh-home wizard"). Hosted runners run every job as an administrator
# with UAC off, which is not how anyone runs the product and which keeps WebView2 150+ from opening
# its remote-debugging endpoint (docs/evidence/M2/dod-ci.md, runs 16 to 19).
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
    [Parameter(Mandatory)][string]$App,
    [Parameter(Mandatory)][string]$GameDir,
    [Parameter(Mandatory)][string]$Output,
    [Parameter(Mandatory)][string]$UserRoot
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
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
}
$env:USERPROFILE = if ($profilePath) { $profilePath } else { $UserRoot }
$env:LOCALAPPDATA = Join-Path $env:USERPROFILE 'AppData\Local'
$env:APPDATA = Join-Path $env:USERPROFILE 'AppData\Roaming'
$env:TEMP = Join-Path $env:LOCALAPPDATA 'Temp'
$env:TMP = $env:TEMP
$facts.profileUsed = $env:USERPROFILE
$facts | ConvertTo-Json -Compress
[Console]::Out.Flush()
foreach ($directory in @($env:LOCALAPPDATA, $env:APPDATA, $env:TEMP)) { New-Item -ItemType Directory -Path $directory -Force | Out-Null }
$env:USERNAME = [Environment]::UserName
$env:HOMEDRIVE = [IO.Path]::GetPathRoot($env:USERPROFILE).TrimEnd('\')
$env:HOMEPATH = $env:USERPROFILE.Substring($env:HOMEDRIVE.Length)
$env:PATH = (Split-Path -Parent $Node) + ';' + $env:PATH
$env:OME_HOME = $OmeHome
$env:OME_DOD_APP = $App
$env:OME_DOD_GAME_APK_DIR = $GameDir
$env:OME_DOD_OUTPUT = $Output
Set-Location (Join-Path $Repo 'host')
& $Node (Join-Path $Repo 'ci\dod\run-dod.mjs')
exit $LASTEXITCODE
