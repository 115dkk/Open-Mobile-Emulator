# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Entry point of the completion driver when the workflow starts it as the standard local user it
# created (m2-dod.yml, "Run fresh-home wizard"). Hosted runners run every job as an administrator
# with UAC off, which is not how anyone runs the product and which keeps WebView2 150+ from opening
# its remote-debugging endpoint (docs/evidence/M2/dod-ci.md, runs 16 to 19). A process started with
# another user's credentials may carry the caller's environment, so the profile paths are taken from
# the shell folders of this user's own loaded profile, and everything the driver needs comes in as
# parameters rather than inherited variables.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Repo,
    [Parameter(Mandatory)][string]$Node,
    [Parameter(Mandatory)][string]$OmeHome,
    [Parameter(Mandatory)][string]$App,
    [Parameter(Mandatory)][string]$GameDir,
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$env:USERPROFILE = [Environment]::GetFolderPath('UserProfile')
$env:LOCALAPPDATA = [Environment]::GetFolderPath('LocalApplicationData')
$env:APPDATA = [Environment]::GetFolderPath('ApplicationData')
$env:TEMP = Join-Path $env:LOCALAPPDATA 'Temp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Path $env:TEMP -Force | Out-Null
$env:USERNAME = [Environment]::UserName
$env:HOMEDRIVE = [IO.Path]::GetPathRoot($env:USERPROFILE).TrimEnd('\')
$env:HOMEPATH = $env:USERPROFILE.Substring($env:HOMEDRIVE.Length)
$env:PATH = (Split-Path -Parent $Node) + ';' + $env:PATH
$env:OME_HOME = $OmeHome
$env:OME_DOD_APP = $App
$env:OME_DOD_GAME_APK_DIR = $GameDir
$env:OME_DOD_OUTPUT = $Output
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$administrator = [Security.Principal.WindowsPrincipal]::new($identity).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
@{user=$identity.Name; administrator=$administrator; profile=$env:USERPROFILE; localAppData=$env:LOCALAPPDATA; session=(Get-Process -Id $PID).SessionId; interactive=[Environment]::UserInteractive; node=$Node} | ConvertTo-Json -Compress
Set-Location (Join-Path $Repo 'host')
& $Node (Join-Path $Repo 'ci\dod\run-dod.mjs')
exit $LASTEXITCODE
