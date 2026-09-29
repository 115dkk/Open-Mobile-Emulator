# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([Parameter(Mandatory)][string]$Scratch, [switch]$Paused, [switch]$Mixed, [switch]$ProcessAware, [string]$SdlDpi = '', [int]$AttachAfterSeconds = 3, [string]$Tag = 'reparent')
# Standalone QEMU (x1 command line, fresh efivars) re-parented into a plain WinForms window.
# -Paused: start with -S, attach as soon as the SDL window exists, then QMP cont. Otherwise attach after $AttachAfterSeconds.
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class RP {
 [StructLayout(LayoutKind.Sequential)] public struct R {public int L,T,Rt,B;}
 public delegate bool E(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int c);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern int SetThreadDpiHostingBehavior(int b);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
 [DllImport("user32.dll")] public static extern IntPtr SetParent(IntPtr c,IntPtr p);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h,int i);
 [DllImport("user32.dll")] public static extern IntPtr SetWindowLongPtrW(IntPtr h,int i,IntPtr v);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int he,uint flags);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h,int a,out R r,int size);
 public static string Class(IntPtr h) {var s=new StringBuilder(256); GetClassName(h,s,256); return s.ToString();}
 public static List<IntPtr> All() {var a=new List<IntPtr>(); EnumWindows((h,p)=>{a.Add(h); return true;},IntPtr.Zero);return a;}
 public static uint Pid(IntPtr h) {uint p; GetWindowThreadProcessId(h,out p);return p;}
 public static void MakeChild(IntPtr child, IntPtr parent, int w, int h) {
  long style = GetWindowLongPtrW(child, -16).ToInt64();
  style &= ~(0x80000000L | 0x00C00000L | 0x00040000L | 0x00020000L | 0x00010000L | 0x00080000L); // POPUP CAPTION THICKFRAME MINBOX MAXBOX SYSMENU
  style |= 0x40000000L; // WS_CHILD
  SetWindowLongPtrW(child, -16, new IntPtr(style));
  long ex = GetWindowLongPtrW(child, -20).ToInt64(); ex &= ~0x00040000L; SetWindowLongPtrW(child, -20, new IntPtr(ex));
  SetParent(child, parent);
  SetWindowPos(child, IntPtr.Zero, 0, 0, w, h, 0x0020 | 0x0004 | 0x0010);
 }
}
'@
if ($ProcessAware) { "process pmv2=$([RP]::SetProcessDpiAwarenessContext([IntPtr](-4)))" } else { [void][RP]::SetThreadDpiAwarenessContext([IntPtr](-4)) }
if ($Mixed) { "mixed hosting previous=$([RP]::SetThreadDpiHostingBehavior(2))" }
New-Item -ItemType Directory -Force "$Scratch\probe-reparent" | Out-Null
if (tasklist | Select-String 'qemu-system|^ome\.exe') { throw 'processes still running' }
$line = (Get-Content (Get-ChildItem "$Scratch\dod-home-x1\logs\*cmd.log").FullName -Raw).Trim()
$split = $line.IndexOf('" ')
$exe = $line.Substring(1, $split - 1)
$args = $line.Substring($split + 2)
$fresh = "$Scratch\probe-reparent\$Tag-efivars.fd"
Copy-Item 'C:\Open Mobile Emulator\host\target\debug\qemu\share\qemu\edk2-i386-vars.fd' $fresh -Force
$args = $args.Replace("$Scratch\dod-home-x1\vm\bliss-16-9-7-android-13\efivars.fd", $fresh)
$qmpPort = [int]([regex]::Match($args, 'tcp:127\.0\.0\.1:(\d+),server')).Groups[1].Value
if ($Paused) { $args = "-S $args" }
$form = [Windows.Forms.Form]::new(); $form.Text = 'reparent host'; $form.StartPosition = 'Manual'; $form.Location = [Drawing.Point]::new(100, 100)
$form.ClientSize = [Drawing.Size]::new(1744, 1216); $form.BackColor = [Drawing.Color]::DarkGreen; $form.Show(); [Windows.Forms.Application]::DoEvents()
if ($SdlDpi) { $env:SDL_WINDOWS_DPI_AWARENESS = $SdlDpi; "SDL_WINDOWS_DPI_AWARENESS=$SdlDpi" }
$q = Start-Process -FilePath $exe -ArgumentList $args -PassThru -RedirectStandardError "$Scratch\probe-reparent\$Tag-qemu-stderr.txt" -RedirectStandardOutput "$Scratch\probe-reparent\$Tag-qemu-stdout.txt"
$t0 = Get-Date
$sdl = $null
while (-not $sdl -and ((Get-Date) - $t0).TotalSeconds -lt 15) {
  [Windows.Forms.Application]::DoEvents()
  $sdl = @([RP]::All() | Where-Object { [RP]::Pid($_) -eq $q.Id -and [RP]::Class($_) -eq 'SDL_app' -and [RP]::IsWindowVisible($_) } | Select-Object -First 1)
  if ($sdl.Count -eq 0) { $sdl = $null; Start-Sleep -Milliseconds 50 }
}
"sdl window after $([int](((Get-Date) - $t0).TotalMilliseconds)) ms: $($sdl[0]) formDpi=$([RP]::GetDpiForWindow($form.Handle)) sdlDpi=$([RP]::GetDpiForWindow($sdl[0]))"
foreach ($w in [RP]::All()) { if ([RP]::Pid($w) -eq $q.Id -and [RP]::Class($w) -ne 'SDL_app' -and [RP]::IsWindowVisible($w)) { [void][RP]::ShowWindow($w, 6); "minimized other qemu window class=$([RP]::Class($w))" } }
if (-not $Paused) { while (((Get-Date) - $t0).TotalSeconds -lt $AttachAfterSeconds) { [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 50 } }
[RP]::MakeChild($sdl[0], $form.Handle, 1744, 1216); [Windows.Forms.Application]::DoEvents()
"attached at $([int](((Get-Date) - $t0).TotalMilliseconds)) ms paused=$([bool]$Paused) sdlDpiAfter=$([RP]::GetDpiForWindow($sdl[0]))"
if ($Paused) {
  $client = [Net.Sockets.TcpClient]::new('127.0.0.1', $qmpPort); $stream = $client.GetStream(); $reader = [IO.StreamReader]::new($stream); $writer = [IO.StreamWriter]::new($stream); $writer.AutoFlush = $true
  $null = $reader.ReadLine(); $writer.WriteLine('{"execute":"qmp_capabilities"}'); $null = $reader.ReadLine(); $writer.WriteLine('{"execute":"cont"}'); "cont -> $($reader.ReadLine())"
  $client.Close()
}
$tAttach = Get-Date
function Shot([string]$label) {
  [Windows.Forms.Application]::DoEvents()
  $r = [RP+R]::new(); [void][RP]::DwmGetWindowAttribute($form.Handle, 9, [ref]$r, 16)
  $file = "$Scratch\probe-reparent\$Tag-$label.png"
  $b = [Drawing.Bitmap]::new(($r.Rt - $r.L), ($r.B - $r.T)); $g = [Drawing.Graphics]::FromImage($b)
  try { $g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size); $b.Save($file, [Drawing.Imaging.ImageFormat]::Png) } finally { $g.Dispose(); $b.Dispose() }
  "$label size=$((Get-Item $file).Length)"
}
foreach ($t in 4, 8, 12, 16, 22) { while (((Get-Date) - $tAttach).TotalSeconds -lt $t) { [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 100 }; Shot "t${t}s" }
Stop-Process -Id $q.Id -Force -Confirm:$false -ErrorAction SilentlyContinue
$form.Close()
Start-Sleep -Seconds 2
"leftover: $((tasklist | Select-String 'qemu-system' | ForEach-Object { $_.Line }) -join ';')"
