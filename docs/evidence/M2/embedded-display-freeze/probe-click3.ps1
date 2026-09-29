# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([Parameter(Mandatory)][string]$Tag, [Parameter(Mandatory)][string]$QemuArgs, [string]$Env = '')
# Minimal QEMU (no disk, SeaBIOS/EDK2 only) as an owned NOACTIVATE popup; one LEFT click; foreground after down/up.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class OW {
 [StructLayout(LayoutKind.Sequential)] public struct R {public int L,T,Rt,B;}
 [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {public ushort vk,scan; public uint flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Explicit)] public struct UNION {[FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi;}
 [StructLayout(LayoutKind.Sequential)] public struct INPUT {public uint type; public UNION u;}
 public delegate bool E(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int c);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h,int i);
 [DllImport("user32.dll")] public static extern IntPtr SetWindowLongPtrW(IntPtr h,int i,IntPtr v);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int he,uint flags);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll",SetLastError=true)] public static extern uint SendInput(uint n,INPUT[] inputs,int size);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref System.Drawing.Point p);
 public static string Class(IntPtr h) {var s=new StringBuilder(256); GetClassName(h,s,256); return s.ToString();}
 public static List<IntPtr> All() {var a=new List<IntPtr>(); EnumWindows((h,p)=>{a.Add(h); return true;},IntPtr.Zero);return a;}
 public static uint Pid(IntPtr h) {uint p; GetWindowThreadProcessId(h,out p);return p;}
 public static void MakeOwnedPopup(IntPtr guest, IntPtr owner, int x, int y, int w, int h) {
  long style = GetWindowLongPtrW(guest, -16).ToInt64(); style &= ~(0x00C00000L | 0x00040000L | 0x00020000L | 0x00010000L | 0x00080000L); style |= 0x80000000L; SetWindowLongPtrW(guest, -16, new IntPtr(style));
  long ex = GetWindowLongPtrW(guest, -20).ToInt64(); ex &= ~0x00040000L; ex |= 0x08000000L | 0x00000080L; SetWindowLongPtrW(guest, -20, new IntPtr(ex));
  SetWindowLongPtrW(guest, -8, owner); SetWindowPos(guest, IntPtr.Zero, x, y, w, h, 0x0020 | 0x0040 | 0x0010);
 }
 public static uint Mouse(uint flags) { var d=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {flags=flags}}}; return SendInput(1,new[]{d},Marshal.SizeOf<INPUT>()); }
}
'@ -ReferencedAssemblies ([System.Drawing.Point].Assembly.Location, 'System.Runtime', 'System.Collections')
[void][OW]::SetProcessDpiAwarenessContext([IntPtr](-4))
$scratch = 'C:\Users\32170336\AppData\Local\Temp\claude\C--Open-Mobile-Emulator\7758e147-b2c0-5152-846b-245148981fae\scratchpad'
$out = "$scratch\probe-click"; New-Item -ItemType Directory -Force $out | Out-Null
if (tasklist | Select-String 'qemu-system|^ome\.exe') { throw 'processes still running' }
$exe = 'C:\Open Mobile Emulator\host\target\debug\qemu\bin\qemu-system-x86_64.exe'
$env:SDL_WINDOWS_DPI_AWARENESS = 'permonitorv2'
foreach ($pair in ($Env -split ';' | Where-Object { $_ })) { $k, $v = $pair -split '=', 2; Set-Item "Env:$k" $v }
$form = [Windows.Forms.Form]::new(); $form.Text = 'owner host'; $form.StartPosition = 'Manual'; $form.Location = [Drawing.Point]::new(60, 60)
$form.ClientSize = [Drawing.Size]::new(1400, 800); $form.BackColor = [Drawing.Color]::DarkGreen; $form.Show(); [Windows.Forms.Application]::DoEvents()
$q = Start-Process -FilePath $exe -ArgumentList $QemuArgs -PassThru -RedirectStandardError "$out\$Tag-stderr.txt" -RedirectStandardOutput "$out\$Tag-stdout.txt"
$t0 = Get-Date; $sdl = $null
while (-not $sdl -and ((Get-Date) - $t0).TotalSeconds -lt 15 -and -not $q.HasExited) {
  [Windows.Forms.Application]::DoEvents()
  $sdl = @([OW]::All() | Where-Object { [OW]::Pid($_) -eq $q.Id -and [OW]::Class($_) -eq 'SDL_app' -and [OW]::IsWindowVisible($_) } | Select-Object -First 1)
  if ($sdl.Count -eq 0) { $sdl = $null; Start-Sleep -Milliseconds 50 }
}
if (-not $sdl) { if (-not $q.HasExited) { Stop-Process -Id $q.Id -Force }; "$Tag NO SDL WINDOW: $(Get-Content "$out\$Tag-stderr.txt" | Select-Object -First 2)"; $form.Close(); exit }
$sdl = $sdl[0]
foreach ($w in [OW]::All()) { if ([OW]::Pid($w) -eq $q.Id -and [OW]::Class($w) -ne 'SDL_app' -and [OW]::IsWindowVisible($w)) { [void][OW]::ShowWindow($w, 6) } }
$origin = [Drawing.Point]::new(0, 0); [void][OW]::ClientToScreen($form.Handle, [ref]$origin)
$gx = $origin.X + 60; $gy = $origin.Y + 40; $gw = 1024; $gh = 640
[OW]::MakeOwnedPopup($sdl, $form.Handle, $gx, $gy, $gw, $gh); [Windows.Forms.Application]::DoEvents()
[void][OW]::SetForegroundWindow($form.Handle)
Start-Sleep -Seconds 3; [Windows.Forms.Application]::DoEvents()
[void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, $gx, $gy, $gw, $gh, 0x0010)
[void][OW]::SetForegroundWindow($form.Handle); Start-Sleep -Milliseconds 400
function Fg { $f = [OW]::GetForegroundWindow(); if ($f -eq $form.Handle) { 'owner' } elseif ($f -eq $sdl) { 'GUEST' } else { "other:$f" } }
$cx = $gx + [int]($gw / 2); $cy = $gy + [int]($gh / 2)
[OW]::SetCursorPos($cx - 16, $cy - 16) | Out-Null; Start-Sleep -Milliseconds 150; [OW]::SetCursorPos($cx, $cy) | Out-Null; Start-Sleep -Milliseconds 600
$hover = Fg
$sd=[OW]::Mouse(2); Start-Sleep -Milliseconds 600; $down = Fg
$su=[OW]::Mouse(4); Start-Sleep -Milliseconds 600; $up = Fg
"$Tag hover=$hover afterDown=$down afterUp=$up sent=$sd/$su"
Stop-Process -Id $q.Id -Force -ErrorAction SilentlyContinue
$form.Close(); Start-Sleep -Milliseconds 800
