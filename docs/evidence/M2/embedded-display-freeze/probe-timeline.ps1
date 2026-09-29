# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([Parameter(Mandatory)][string]$Scratch, [string]$App = 'C:\Open Mobile Emulator\host\target\debug\ome.exe', [string]$Tag = 'timeline', [switch]$HeadingClick, [int]$Until = 40)
# Dense timeline of the embedded stage with the driver's own main-window capture (native.ps1 -Shot, no QemuPid), no keys.
New-Item -ItemType Directory -Force "$Scratch\probe-timeline" | Out-Null
if (tasklist | Select-String 'qemu-system|^ome\.exe') { throw 'processes still running' }
& 'C:\Program Files\Git\usr\bin\rm.exe' -rf "$Scratch\probe-home-nokeys\vm"
$env:OME_HOME = "$Scratch\probe-home-nokeys"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9335'
$p = Start-Process -FilePath $App -WorkingDirectory (Split-Path $App) -PassThru -RedirectStandardError "$Scratch\probe-timeline\$Tag-stderr.txt" -RedirectStandardOutput "$Scratch\probe-timeline\$Tag-stdout.txt"
Start-Sleep -Seconds 10
Set-Location 'C:\Open Mobile Emulator\host'
$walker = if ($HeadingClick) { "$Scratch\probe-walk-heading.mjs" } else { "$Scratch\probe-walk-only.mjs" }
$qemuPid = (node $walker 9335 2>&1 | Select-Object -Last 1).Trim()
$t0 = Get-Date
"qemu pid=$qemuPid tag=$Tag headingClick=$([bool]$HeadingClick)"
$i = 0
while (((Get-Date) - $t0).TotalSeconds -lt $Until) {
  $t = [int]((Get-Date) - $t0).TotalSeconds
  $file = "$Scratch\probe-timeline\$Tag-{0:d2}-t{1:d2}s.png" -f $i, $t
  $out = pwsh -NoProfile -File 'C:\Open Mobile Emulator\ci\dod\native.ps1' -AppPid $p.Id -Shot $file 2>&1 | Out-String
  "t=${t}s shot=$i size=$(if (Test-Path $file) { (Get-Item $file).Length } else { 'none' })"
  $i++
}
Stop-Process -Id $p.Id -Force -Confirm:$false -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3
"leftover: $((tasklist | Select-String 'qemu-system|^ome\.exe' | ForEach-Object { $_.Line }) -join ';')"
