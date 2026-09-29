# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([Parameter(Mandatory)][string]$Scratch, [Parameter(Mandatory)][string]$Kick, [string]$App = 'C:\Open Mobile Emulator\host\target\debug\ome.exe', [string]$Tag = 'kick')
# Detects a frozen embedded frame (identical SDL-rect captures at 3 s and 6 s), applies one kick to the SDL child, then captures again.
# Kick: 'hideshow' = ShowWindow(SW_HIDE) then ShowWindow(SW_SHOW); 'resize' = SetWindowPos 1 px smaller then back; 'redraw' = RedrawWindow.
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class KK {
 [StructLayout(LayoutKind.Sequential)] public struct R {public int L,T,Rt,B;}
 public delegate bool E(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h,E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int c);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int he,uint flags);
 [DllImport("user32.dll")] public static extern bool RedrawWindow(IntPtr h,IntPtr rect,IntPtr rgn,uint flags);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out R r);
 public static string Class(IntPtr h) {var s=new StringBuilder(256); GetClassName(h,s,256); return s.ToString();}
 public static List<IntPtr> All() {var a=new List<IntPtr>(); EnumWindows((h,p)=>{a.Add(h); return true;},IntPtr.Zero);return a;}
 public static List<IntPtr> Children(IntPtr h) {var a=new List<IntPtr>(); EnumChildWindows(h,(h2,p)=>{a.Add(h2); return true;},IntPtr.Zero);return a;}
 public static uint Pid(IntPtr h) {uint p; GetWindowThreadProcessId(h,out p);return p;}
}
'@
[void][KK]::SetThreadDpiAwarenessContext([IntPtr](-4))
New-Item -ItemType Directory -Force "$Scratch\probe-kick" | Out-Null
if (tasklist | Select-String 'qemu-system|^ome\.exe') { throw 'processes still running' }
& 'C:\Program Files\Git\usr\bin\rm.exe' -rf "$Scratch\probe-home-nokeys\vm"
$env:OME_HOME = "$Scratch\probe-home-nokeys"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9335'
$p = Start-Process -FilePath $App -WorkingDirectory (Split-Path $App) -PassThru -RedirectStandardError "$Scratch\probe-kick\$Tag-stderr.txt" -RedirectStandardOutput "$Scratch\probe-kick\$Tag-stdout.txt"
Start-Sleep -Seconds 10
$main = (Get-Process -Id $p.Id).MainWindowHandle
Set-Location 'C:\Open Mobile Emulator\host'
$qemuPid = (node "$Scratch\probe-walk-only.mjs" 9335 2>&1 | Select-Object -Last 1).Trim()
$t0 = Get-Date
"qemu pid=$qemuPid kick=$Kick"
function NativeShot([string]$label) {
  $file = "$Scratch\probe-kick\$Tag-$label.png"
  $null = pwsh -NoProfile -File 'C:\Open Mobile Emulator\ci\dod\native.ps1' -AppPid $p.Id -QemuPid $qemuPid -Shot $file 2>&1
  if (Test-Path $file) { (Get-Item $file).Length } else { -1 }
}
$sizes = @{}
foreach ($t in 3, 6) { Start-Sleep -Seconds ([Math]::Max(0, $t - ((Get-Date) - $t0).TotalSeconds)); $sizes[$t] = NativeShot "t${t}s"; "t=${t}s size=$($sizes[$t])" }
$frozen = ($sizes[3] -eq $sizes[6]) -and ($sizes[3] -lt 400000)
"frozen=$frozen"
$sdl = @([KK]::Children($main) | Where-Object { [KK]::Class($_) -eq 'SDL_app' } | Select-Object -First 1)
if ($sdl.Count -eq 1 -and $frozen) {
  $h = $sdl[0]
  switch ($Kick) {
    'hideshow' { "hide=$([KK]::ShowWindow($h, 0)) show=$([KK]::ShowWindow($h, 5))" }
    'resize' { $r = [KK+R]::new(); [void][KK]::GetWindowRect($h, [ref]$r); $w = $r.Rt - $r.L; $hh = $r.B - $r.T; "resize -1=$([KK]::SetWindowPos($h, [IntPtr]::Zero, 0,0, $w-1, $hh-1, 0x0002 -bor 0x0004 -bor 0x0010)) back=$([KK]::SetWindowPos($h, [IntPtr]::Zero, 0,0, $w, $hh, 0x0002 -bor 0x0004 -bor 0x0010))" }
    'resizeredraw' { $r = [KK+R]::new(); [void][KK]::GetWindowRect($h, [ref]$r); $w = $r.Rt - $r.L; $hh = $r.B - $r.T
      $a = [KK]::SetWindowPos($h, [IntPtr]::Zero, 0,0, $w-2, $hh-2, 0x0002 -bor 0x0004 -bor 0x0010); $b = [KK]::RedrawWindow($h, [IntPtr]::Zero, [IntPtr]::Zero, 0x0001 -bor 0x0004 -bor 0x0100)
      Start-Sleep -Milliseconds 400
      $c = [KK]::SetWindowPos($h, [IntPtr]::Zero, 0,0, $w, $hh, 0x0002 -bor 0x0004 -bor 0x0010); $d = [KK]::RedrawWindow($h, [IntPtr]::Zero, [IntPtr]::Zero, 0x0001 -bor 0x0004 -bor 0x0100)
      "resize-2=$a redraw=$b back=$c redraw=$d" }
    'redraw' { "redraw=$([KK]::RedrawWindow($h, [IntPtr]::Zero, [IntPtr]::Zero, 0x0001 -bor 0x0004 -bor 0x0400 -bor 0x0080))" }
    default { 'no kick' }
  }
  foreach ($t in 9, 12, 15, 18) { Start-Sleep -Seconds ([Math]::Max(0, $t - ((Get-Date) - $t0).TotalSeconds)); "t=${t}s size=$(NativeShot "t${t}s")" }
} else { "no kick applied (sdl=$($sdl.Count) frozen=$frozen)"; foreach ($t in 9, 12) { Start-Sleep -Seconds ([Math]::Max(0, $t - ((Get-Date) - $t0).TotalSeconds)); "t=${t}s size=$(NativeShot "t${t}s")" } }
Stop-Process -Id $p.Id -Force -Confirm:$false -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3
"leftover: $((tasklist | Select-String 'qemu-system|^ome\.exe' | ForEach-Object { $_.Line }) -join ';')"
