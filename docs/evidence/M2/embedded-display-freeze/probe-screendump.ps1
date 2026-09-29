# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([string]$Tag = 'sd', [int]$At = 14, [string]$Display = '', [string]$Exe = '', [string]$DpiEnv = 'permonitorv2', [switch]$Kick)
# Starts the installer ISO boot (fresh EFI vars, -snapshot), leaves the SDL window untouched, and at -At seconds
# asks QEMU over QMP for a screendump PNG of the guest framebuffer (independent of the window), plus a screen copy
# of the window rect. Tells "guest not at GRUB" apart from "window not presenting".
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class SD {
 [StructLayout(LayoutKind.Sequential)] public struct R {public int L,T,Rt,B;}
 public delegate bool E(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int c);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out R r);
 [StructLayout(LayoutKind.Sequential)] public struct P {public int X,Y;}
 [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(P p);
 [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
 [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h,int a,out int v,int size);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h,int i);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder s,int c);
 public static string Title(IntPtr h) {var s=new StringBuilder(256); GetWindowText(h,s,256); return s.ToString();}
 public static string Class(IntPtr h) {var s=new StringBuilder(256); GetClassName(h,s,256); return s.ToString();}
 public static List<IntPtr> All() {var a=new List<IntPtr>(); EnumWindows((h,p)=>{a.Add(h); return true;},IntPtr.Zero);return a;}
 public static uint Pid(IntPtr h) {uint p; GetWindowThreadProcessId(h,out p);return p;}
}
'@ -ReferencedAssemblies ('System.Runtime', 'System.Collections')
[void][SD]::SetProcessDpiAwarenessContext([IntPtr](-4))
if (tasklist | Select-String 'qemu-system|^ome\.exe') { throw 'processes still running' }
$exe = 'C:\Open Mobile Emulator\host\target\debug\qemu\bin\qemu-system-x86_64.exe'
$line = (Get-Content 'C:\Users\32170336\AppData\Local\Temp\claude\C--Open-Mobile-Emulator\7758e147-b2c0-5152-846b-245148981fae\scratchpad\dod-home-1\logs\qemu-bliss-16-9-7-android-13-20260929-145827006.cmd.log' -TotalCount 1).Trim()
$args = $line.Substring($line.IndexOf('.exe" ') + 6) + ' -snapshot'
$freshVars = "$PSScriptRoot\probe-click\fresh-efivars-$Tag.fd"
Copy-Item 'C:\Open Mobile Emulator\host\target\debug\qemu\share\qemu\edk2-i386-vars.fd' $freshVars -Force
$args = [regex]::Replace($args, 'if=pflash,format=raw,file=[^ ]*efivars\.fd', "if=pflash,format=raw,file=$freshVars")
if ($Display) { $args = $args -replace '-device virtio-vga-gl,edid=off -display sdl,show-cursor=on,gl=on,activate-on-click=off', $Display }
if ($Exe) { $exe = $Exe }
if ($DpiEnv -eq 'unaware') { Remove-Item Env:SDL_WINDOWS_DPI_AWARENESS -ErrorAction SilentlyContinue } else { $env:SDL_WINDOWS_DPI_AWARENESS = $DpiEnv }
"display args: $([regex]::Match($args, '-device virtio[^ ]* -display [^ ]*').Value) exe=$exe dpi=$DpiEnv"
$q = Start-Process -FilePath $exe -ArgumentList $args -PassThru -RedirectStandardError "$PSScriptRoot\probe-click\$Tag-stderr.txt" -RedirectStandardOutput "$PSScriptRoot\probe-click\$Tag-stdout.txt"
$t0 = Get-Date; $sdl = $null
while (-not $sdl -and ((Get-Date) - $t0).TotalSeconds -lt 15) {
  $sdl = @([SD]::All() | Where-Object { [SD]::Pid($_) -eq $q.Id -and [SD]::Class($_) -eq 'SDL_app' -and [SD]::IsWindowVisible($_) } | Select-Object -First 1)
  if ($sdl.Count -eq 0) { $sdl = $null; Start-Sleep -Milliseconds 50 }
}
"window_at=$([int](((Get-Date) - $t0).TotalMilliseconds))ms"
function Qmp([string]$json) {
  $c = [System.Net.Sockets.TcpClient]::new('127.0.0.1', 4444); $s = $c.GetStream(); $s.ReadTimeout = 8000
  $r = [System.IO.StreamReader]::new($s); $w = [System.IO.StreamWriter]::new($s); $w.AutoFlush = $true
  $null = $r.ReadLine(); $w.WriteLine('{"execute":"qmp_capabilities"}'); $null = $r.ReadLine()
  $w.WriteLine($json); $reply = $r.ReadLine(); $c.Close(); $reply
}
while (((Get-Date) - $t0).TotalSeconds -lt $At) { Start-Sleep -Milliseconds 200 }
$dump = "$PSScriptRoot\probe-click\$Tag-screendump-${At}s.png"
$dumpJson = ($dump -replace '\\', '\\')
"screendump reply: $(Qmp ('{"execute":"screendump","arguments":{"filename":"' + $dumpJson + '","format":"png"}}'))"
"status reply: $(Qmp '{"execute":"query-status"}')"
if ($sdl) {
  $sdl = $sdl[0]; $r = [SD+R]::new(); [void][SD]::GetWindowRect($sdl, [ref]$r)
  $b = [Drawing.Bitmap]::new(($r.Rt - $r.L), ($r.B - $r.T)); $g = [Drawing.Graphics]::FromImage($b)
  try { $g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size); $b.Save("$PSScriptRoot\probe-click\$Tag-window-${At}s.png", [Drawing.Imaging.ImageFormat]::Png) } finally { $g.Dispose(); $b.Dispose() }
  "window rect $($r.L),$($r.T) $($r.Rt - $r.L)x$($r.B - $r.T) captured"
  $pt = [SD+P]::new(); $pt.X = [int](($r.L + $r.Rt) / 2); $pt.Y = [int](($r.T + $r.B) / 2)
  $top = [SD]::GetAncestor([SD]::WindowFromPoint($pt), 2)
  $tp = [SD]::Pid($top); $tn = (Get-Process -Id $tp -ErrorAction SilentlyContinue).ProcessName
  $cloaked = 0; $hr = [SD]::DwmGetWindowAttribute($sdl, 14, [ref]$cloaked, 4)
  "dwm cloaked=$cloaked (hr=$hr) iconic=$([SD]::IsIconic($sdl)) style=0x$('{0:X}' -f [SD]::GetWindowLongPtrW($sdl,-16).ToInt64()) ex=0x$('{0:X}' -f [SD]::GetWindowLongPtrW($sdl,-20).ToInt64())"
  "window at center of QEMU rect: hwnd=$top class=$([SD]::Class($top)) process=$tn title=$([SD]::Title($top)) (QEMU hwnd=$sdl)"
}
if ($Kick) {
  function Lit([int]$x, [int]$y, [int]$w, [int]$h) { $b = [Drawing.Bitmap]::new($w, $h); $g = [Drawing.Graphics]::FromImage($b); try { $g.CopyFromScreen($x, $y, 0, 0, $b.Size); $t = 0; $l = 0; for ($yy = 0; $yy -lt $h; $yy += 16) { for ($xx = 0; $xx -lt $w; $xx += 16) { $c = $b.GetPixel($xx, $yy); $t++; if (($c.R + $c.G + $c.B) -gt 60) { $l++ } } }; $l / $t } finally { $g.Dispose(); $b.Dispose() } }
  $r = [SD+R]::new(); [void][SD]::GetWindowRect($sdl, [ref]$r)
  "before kick lit={0:P0}" -f (Lit $r.L $r.T ($r.Rt - $r.L) ($r.B - $r.T))
  "kick reply: $(Qmp '{"execute":"input-send-event","arguments":{"events":[{"type":"key","data":{"down":true,"key":{"type":"qcode","data":"down"}}},{"type":"key","data":{"down":false,"key":{"type":"qcode","data":"down"}}}]}}')"
  Start-Sleep -Milliseconds 1500
  "after kick lit={0:P0}" -f (Lit $r.L $r.T ($r.Rt - $r.L) ($r.B - $r.T))
  $dump2 = "$PSScriptRoot\probe-click\$Tag-screendump-after-kick.png"
  "screendump after kick: $(Qmp ('{"execute":"screendump","arguments":{"filename":"' + ($dump2 -replace '\', '\\') + '","format":"png"}}'))"
  $b = [Drawing.Bitmap]::new(($r.Rt - $r.L), ($r.B - $r.T)); $g = [Drawing.Graphics]::FromImage($b)
  try { $g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size); $b.Save("$PSScriptRoot\probe-click\$Tag-window-after-kick.png", [Drawing.Imaging.ImageFormat]::Png) } finally { $g.Dispose(); $b.Dispose() }
}
Stop-Process -Id $q.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 800
"stderr: $((Get-Content "$PSScriptRoot\probe-click\$Tag-stderr.txt" | Where-Object { $_ -notmatch "doesn't support requested feature" }) -join ' | ')"
