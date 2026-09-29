# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([Parameter(Mandatory)][string]$Scratch, [string]$App = 'C:\Open Mobile Emulator\host\target\debug\ome.exe', [string]$Tag = 'cmp', [int]$Until = 30)
# Same run, alternating capture methods: native.ps1 main window (DWM bounds) vs native.ps1 -QemuPid (GetWindowRect crop of SDL_app).
New-Item -ItemType Directory -Force "$Scratch\probe-compare" | Out-Null
if (tasklist | Select-String 'qemu-system|^ome\.exe') { throw 'processes still running' }
& 'C:\Program Files\Git\usr\bin\rm.exe' -rf "$Scratch\probe-home-nokeys\vm"
$env:OME_HOME = "$Scratch\probe-home-nokeys"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9335'
$p = Start-Process -FilePath $App -WorkingDirectory (Split-Path $App) -PassThru -RedirectStandardError "$Scratch\probe-compare\$Tag-stderr.txt" -RedirectStandardOutput "$Scratch\probe-compare\$Tag-stdout.txt"
Start-Sleep -Seconds 10
Set-Location 'C:\Open Mobile Emulator\host'
$qemuPid = (node "$Scratch\probe-walk-only.mjs" 9335 2>&1 | Select-Object -Last 1).Trim()
$t0 = Get-Date
"qemu pid=$qemuPid"
$i = 0
while (((Get-Date) - $t0).TotalSeconds -lt $Until) {
  $t = [int]((Get-Date) - $t0).TotalSeconds
  $fm = "$Scratch\probe-compare\$Tag-{0:d2}-main-t{1:d2}s.png" -f $i, $t
  $om = pwsh -NoProfile -File 'C:\Open Mobile Emulator\ci\dod\native.ps1' -AppPid $p.Id -Shot $fm 2>&1 | Out-String
  $fq = "$Scratch\probe-compare\$Tag-{0:d2}-qemu-t{1:d2}s.png" -f $i, $t
  $oq = pwsh -NoProfile -File 'C:\Open Mobile Emulator\ci\dod\native.ps1' -AppPid $p.Id -QemuPid $qemuPid -Shot $fq 2>&1 | Out-String
  $cropM = if ($om -match '"crop":(\{[^}]*\})') { $Matches[1] } else { $om.Substring(0, [Math]::Min(100, $om.Length)) }
  $cropQ = if ($oq -match '"crop":(\{[^}]*\})') { $Matches[1] } else { $oq.Substring(0, [Math]::Min(100, $oq.Length)) }
  "t=${t}s main=$(if (Test-Path $fm) { (Get-Item $fm).Length } else { 'none' }) crop=$cropM | qemu=$(if (Test-Path $fq) { (Get-Item $fq).Length } else { 'none' }) crop=$cropQ"
  $i++
}
Stop-Process -Id $p.Id -Force -Confirm:$false -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3
"leftover: $((tasklist | Select-String 'qemu-system|^ome\.exe' | ForEach-Object { $_.Line }) -join ';')"
