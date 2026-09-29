# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([Parameter(Mandatory)][string]$Scratch, [Parameter(Mandatory)][string]$Variant, [string]$App = 'C:\Open Mobile Emulator\host\target\debug\ome.exe', [string]$Tag = 'variant')
# Variant: 'none' = nothing at 3 s; 'fg' = SetForegroundWindow(main) only at 3 s; 'driver' = native.ps1 capture at 3 s (its Focus routine).
# Later captures always go through native.ps1 (-QemuPid crop), the method the completion driver uses.
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class FW {
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
}
'@
New-Item -ItemType Directory -Force "$Scratch\probe-variants" | Out-Null
if (tasklist | Select-String 'qemu-system|^ome\.exe') { throw 'processes still running' }
& 'C:\Program Files\Git\usr\bin\rm.exe' -rf "$Scratch\probe-home-nokeys\vm"
$env:OME_HOME = "$Scratch\probe-home-nokeys"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9335'
$p = Start-Process -FilePath $App -WorkingDirectory (Split-Path $App) -PassThru -RedirectStandardError "$Scratch\probe-variants\$Tag-stderr.txt" -RedirectStandardOutput "$Scratch\probe-variants\$Tag-stdout.txt"
Start-Sleep -Seconds 10
$main = (Get-Process -Id $p.Id).MainWindowHandle
Set-Location 'C:\Open Mobile Emulator\host'
$qemuPid = (node "$Scratch\probe-walk-only.mjs" 9335 2>&1 | Select-Object -Last 1).Trim()
$t0 = Get-Date
"qemu pid=$qemuPid variant=$Variant app=$App"
function NativeShot([string]$label) {
  $file = "$Scratch\probe-variants\$Tag-$label.png"
  $out = pwsh -NoProfile -File 'C:\Open Mobile Emulator\ci\dod\native.ps1' -AppPid $p.Id -QemuPid $qemuPid -Shot $file 2>&1 | Out-String
  $ok = Test-Path $file
  "$label native size=$(if ($ok) { (Get-Item $file).Length } else { 'none: ' + $out.Substring(0, [Math]::Min(120, $out.Length)) })"
}
Start-Sleep -Seconds ([Math]::Max(0, 3 - ((Get-Date) - $t0).TotalSeconds))
switch ($Variant) {
  'fg' { [void][FW]::ShowWindow($main, 9); "SetForegroundWindow=$([FW]::SetForegroundWindow($main)) fg=$([FW]::GetForegroundWindow() -eq $main)" }
  'driver' { NativeShot 't3s' }
  default { 'no action at 3 s' }
}
foreach ($t in 9, 12, 15) { Start-Sleep -Seconds ([Math]::Max(0, $t - ((Get-Date) - $t0).TotalSeconds)); NativeShot "t${t}s" }
Stop-Process -Id $p.Id -Force -Confirm:$false -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3
"leftover: $((tasklist | Select-String 'qemu-system|^ome\.exe' | ForEach-Object { $_.Line }) -join ';')"
