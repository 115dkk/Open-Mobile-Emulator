# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([Parameter(Mandatory)][string]$Tag, [Parameter(Mandatory)][ValidateSet('zero-then-size','size-at-once','size-after-4s','convert-at-1s','convert-at-3s','hide-then-settle','none','resize-only','style-only','owner-only','convert-then-kick','convert-no-framechanged','resize-only-nocopybits','convert-nocopybits','move-only','resize-at-6s','zorder-only')][string]$Mode, [ValidateSet('unaware','system','permonitorv2')][string]$DpiEnv = 'permonitorv2')
# Does the owned-popup conversion freeze QEMU's presentation during firmware/GRUB? Starts the developer guest
# (-snapshot, killed during GRUB), converts the SDL window like the product does, and measures the share of
# non-black pixels in the popup at 6, 9 and 12 s. GRUB's menu has a photo background, so a live popup is far
# from black; a frozen one stays black.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class OW {
 [StructLayout(LayoutKind.Sequential)] public struct R {public int L,T,Rt,B;}
 public delegate bool E(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int c);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h,int i);
 [DllImport("user32.dll")] public static extern IntPtr SetWindowLongPtrW(IntPtr h,int i,IntPtr v);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int he,uint flags);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref System.Drawing.Point p);
 [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);
 [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {public ushort vk,scan; public uint flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Explicit)] public struct UNION {[FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi;}
 [StructLayout(LayoutKind.Sequential)] public struct INPUT {public uint type; public UNION u;}
 [DllImport("user32.dll",SetLastError=true)] public static extern uint SendInput(uint n,INPUT[] inputs,int size);
 public static uint Jiggle() { var a=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {x=1,y=0,flags=1}}}; var b=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {x=-1,y=0,flags=1}}}; return SendInput(2,new[]{a,b},Marshal.SizeOf<INPUT>()); }
 public static string Class(IntPtr h) {var s=new StringBuilder(256); GetClassName(h,s,256); return s.ToString();}
 public static List<IntPtr> All() {var a=new List<IntPtr>(); EnumWindows((h,p)=>{a.Add(h); return true;},IntPtr.Zero);return a;}
 public static uint Pid(IntPtr h) {uint p; GetWindowThreadProcessId(h,out p);return p;}
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
 public static void StyleOnly(IntPtr guest) {
  long style = GetWindowLongPtrW(guest, -16).ToInt64(); style &= ~(0x00C00000L | 0x00040000L | 0x00020000L | 0x00010000L | 0x00080000L); style |= 0x80000000L; SetWindowLongPtrW(guest, -16, new IntPtr(style));
  long ex = GetWindowLongPtrW(guest, -20).ToInt64(); ex &= ~0x00040000L; ex |= 0x08000000L | 0x00000080L; SetWindowLongPtrW(guest, -20, new IntPtr(ex));
  SetWindowPos(guest, IntPtr.Zero, 0, 0, 0, 0, 0x0020 | 0x0002 | 0x0001 | 0x0004 | 0x0010);
 }
 public static void OwnerOnly(IntPtr guest, IntPtr owner) { SetWindowLongPtrW(guest, -8, owner); SetWindowPos(guest, IntPtr.Zero, 0, 0, 0, 0, 0x0002 | 0x0001 | 0x0004 | 0x0010); }
 public static void ConvertNoFrameChanged(IntPtr guest, IntPtr owner, int x, int y, int w, int h) {
  long style = GetWindowLongPtrW(guest, -16).ToInt64(); style &= ~(0x00C00000L | 0x00040000L | 0x00020000L | 0x00010000L | 0x00080000L); style |= 0x80000000L; SetWindowLongPtrW(guest, -16, new IntPtr(style));
  long ex = GetWindowLongPtrW(guest, -20).ToInt64(); ex &= ~0x00040000L; ex |= 0x08000000L | 0x00000080L; SetWindowLongPtrW(guest, -20, new IntPtr(ex));
  SetWindowLongPtrW(guest, -8, owner); SetWindowPos(guest, IntPtr.Zero, x, y, w, h, 0x0040 | 0x0010);
 }
 public static void MakeOwnedPopupNoCopy(IntPtr guest, IntPtr owner, int x, int y, int w, int h) {
  long style = GetWindowLongPtrW(guest, -16).ToInt64(); style &= ~(0x00C00000L | 0x00040000L | 0x00020000L | 0x00010000L | 0x00080000L); style |= 0x80000000L; SetWindowLongPtrW(guest, -16, new IntPtr(style));
  long ex = GetWindowLongPtrW(guest, -20).ToInt64(); ex &= ~0x00040000L; ex |= 0x08000000L | 0x00000080L; SetWindowLongPtrW(guest, -20, new IntPtr(ex));
  SetWindowLongPtrW(guest, -8, owner); SetWindowPos(guest, IntPtr.Zero, x, y, w, h, 0x0020 | 0x0040 | 0x0010 | 0x0100);
 }
 public static void MakeOwnedPopup(IntPtr guest, IntPtr owner, int x, int y, int w, int h) {
  long style = GetWindowLongPtrW(guest, -16).ToInt64(); style &= ~(0x00C00000L | 0x00040000L | 0x00020000L | 0x00010000L | 0x00080000L); style |= 0x80000000L; SetWindowLongPtrW(guest, -16, new IntPtr(style));
  long ex = GetWindowLongPtrW(guest, -20).ToInt64(); ex &= ~0x00040000L; ex |= 0x08000000L | 0x00000080L; SetWindowLongPtrW(guest, -20, new IntPtr(ex));
  SetWindowLongPtrW(guest, -8, owner); SetWindowPos(guest, IntPtr.Zero, x, y, w, h, 0x0020 | 0x0040 | 0x0010);
 }
}
'@ -ReferencedAssemblies ([System.Drawing.Point].Assembly.Location, 'System.Runtime', 'System.Collections')
[void][OW]::SetProcessDpiAwarenessContext([IntPtr](-4))
if ($env:OME_PROBE_WAKE -eq '1') { $sent = [OW]::Jiggle(); $prev = [OW]::SetThreadExecutionState([uint32]2147483651); "wake: jiggle sent=$sent previous_state=0x$('{0:X}' -f $prev)"; Start-Sleep -Seconds 2 }
function NonBlackShare([int]$x, [int]$y, [int]$w, [int]$h) {
  $b = [Drawing.Bitmap]::new($w, $h); $g = [Drawing.Graphics]::FromImage($b)
  try {
    $g.CopyFromScreen($x, $y, 0, 0, $b.Size)
    $total = 0; $lit = 0
    for ($yy = 0; $yy -lt $h; $yy += 16) { for ($xx = 0; $xx -lt $w; $xx += 16) { $c = $b.GetPixel($xx, $yy); $total++; if (($c.R + $c.G + $c.B) -gt 60) { $lit++ } } }
    if ($total -eq 0) { 0 } else { $lit / $total }
  } finally { $g.Dispose(); $b.Dispose() }
}
if (tasklist | Select-String 'qemu-system|^ome\.exe') { throw 'processes still running' }
$exe = 'C:\Open Mobile Emulator\host\target\debug\qemu\bin\qemu-system-x86_64.exe'
# The installer ISO boot of completion round 1: its GRUB menu (photo background) stays for about 30 s.
$line = (Get-Content 'C:\Users\32170336\AppData\Local\Temp\claude\C--Open-Mobile-Emulator\7758e147-b2c0-5152-846b-245148981fae\scratchpad\dod-home-1\logs\qemu-bliss-16-9-7-android-13-20260929-145827006.cmd.log' -TotalCount 1).Trim()
$args = $line.Substring($line.IndexOf('.exe" ') + 6) + ' -snapshot'
# Fresh EFI variable store per run: the round-1 store makes the firmware try the empty ESP first (slow).
$freshVars = "$PSScriptRoot\probe-click\fresh-efivars-$Tag-$Mode.fd"
Copy-Item 'C:\Open Mobile Emulator\host\target\debug\qemu\share\qemu\edk2-i386-vars.fd' $freshVars -Force
$args = [regex]::Replace($args, 'if=pflash,format=raw,file=[^ ]*efivars\.fd', "if=pflash,format=raw,file=$freshVars")
if ($DpiEnv -eq 'unaware') { Remove-Item Env:SDL_WINDOWS_DPI_AWARENESS -ErrorAction SilentlyContinue } else { $env:SDL_WINDOWS_DPI_AWARENESS = $DpiEnv }
$form = [Windows.Forms.Form]::new(); $form.Text = 'owner host'; $form.StartPosition = 'Manual'; $form.Location = [Drawing.Point]::new(60, 60)
# Small owner form far from the popup so an unowned QEMU window is never covered when the owner is activated.
$form.ClientSize = [Drawing.Size]::new(500, 300); $form.BackColor = [Drawing.Color]::DarkGreen; $form.Show(); [Windows.Forms.Application]::DoEvents()
$gx = 900; $gy = 500; $gw = 1280; $gh = 720
$q = Start-Process -FilePath $exe -ArgumentList $args -PassThru -RedirectStandardError "$env:TEMP\probe-freeze-$Tag-err.txt" -RedirectStandardOutput "$env:TEMP\probe-freeze-$Tag-out.txt"
$t0 = Get-Date; $sdl = $null
while (-not $sdl -and ((Get-Date) - $t0).TotalSeconds -lt 15) {
  [Windows.Forms.Application]::DoEvents()
  $sdl = @([OW]::All() | Where-Object { [OW]::Pid($_) -eq $q.Id -and [OW]::Class($_) -eq 'SDL_app' -and [OW]::IsWindowVisible($_) } | Select-Object -First 1)
  if ($sdl.Count -eq 0) { $sdl = $null; Start-Sleep -Milliseconds 20 }
}
if (-not $sdl) { Stop-Process -Id $q.Id -Force; throw 'no SDL window' }
$sdl = $sdl[0]
$found = [int](((Get-Date) - $t0).TotalMilliseconds)
foreach ($w in [OW]::All()) { if ([OW]::Pid($w) -eq $q.Id -and [OW]::Class($w) -ne 'SDL_app' -and [OW]::IsWindowVisible($w)) { [void][OW]::ShowWindow($w, 6) } }
switch ($Mode) {
  'zero-then-size' { [OW]::MakeOwnedPopup($sdl, $form.Handle, $gx, $gy, 0, 0); [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, $gx, $gy, $gw, $gh, 0x0010) }
  'size-at-once' { [OW]::MakeOwnedPopup($sdl, $form.Handle, $gx, $gy, $gw, $gh) }
  'size-after-4s' { while (((Get-Date) - $t0).TotalSeconds -lt 4) { [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 50 }; [OW]::MakeOwnedPopup($sdl, $form.Handle, $gx, $gy, $gw, $gh) }
  'resize-only-nocopybits' { [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, $gx, $gy, $gw, $gh, 0x0010 -bor 0x0100) }
  'convert-nocopybits' { [OW]::MakeOwnedPopupNoCopy($sdl, $form.Handle, $gx, $gy, $gw, $gh) }
  'zorder-only' { $r0 = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r0); $gx = $r0.L; $gy = $r0.T; $gw = $r0.Rt - $r0.L; $gh = $r0.B - $r0.T }
  'move-only' { [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, $gx, $gy, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010); $r0 = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r0); $gw = $r0.Rt - $r0.L; $gh = $r0.B - $r0.T }
  'resize-at-6s' { $r0 = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r0); $gx = $r0.L; $gy = $r0.T; $gw = $r0.Rt - $r0.L; $gh = $r0.B - $r0.T }
  'resize-only' { [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, $gx, $gy, $gw, $gh, 0x0010) }
  'style-only' { [OW]::StyleOnly($sdl); $r0 = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r0); $gx = $r0.L; $gy = $r0.T; $gw = $r0.Rt - $r0.L; $gh = $r0.B - $r0.T }
  'owner-only' { [OW]::OwnerOnly($sdl, $form.Handle); $r0 = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r0); $gx = $r0.L; $gy = $r0.T; $gw = $r0.Rt - $r0.L; $gh = $r0.B - $r0.T }
  'convert-then-kick' { [OW]::MakeOwnedPopup($sdl, $form.Handle, $gx, $gy, $gw, $gh) }
  'convert-no-framechanged' { [OW]::ConvertNoFrameChanged($sdl, $form.Handle, $gx, $gy, $gw, $gh) }
  'none' { $r0 = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r0); $gx = $r0.L; $gy = $r0.T; $gw = $r0.Rt - $r0.L; $gh = $r0.B - $r0.T; "untouched window at $gx,$gy ${gw}x$gh" }
  'convert-at-1s' { while (((Get-Date) - $t0).TotalSeconds -lt 1) { [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 20 }; [OW]::MakeOwnedPopup($sdl, $form.Handle, $gx, $gy, $gw, $gh) }
  'convert-at-3s' { while (((Get-Date) - $t0).TotalSeconds -lt 3) { [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 20 }; [OW]::MakeOwnedPopup($sdl, $form.Handle, $gx, $gy, $gw, $gh) }
  'hide-then-settle' {
    [void][OW]::ShowWindow($sdl, 0)
    $last = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$last); $stableSince = Get-Date; $t1 = Get-Date
    do {
      Start-Sleep -Milliseconds 100; [Windows.Forms.Application]::DoEvents()
      $now = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$now)
      if (($now.Rt - $now.L) -ne ($last.Rt - $last.L) -or ($now.B - $now.T) -ne ($last.B - $last.T)) { $last = $now; $stableSince = Get-Date }
    } while (((Get-Date) - $stableSince).TotalMilliseconds -lt 1500 -and ((Get-Date) - $t1).TotalSeconds -lt 8)
    $converted = [int](((Get-Date) - $t0).TotalMilliseconds)
    [OW]::MakeOwnedPopup($sdl, $form.Handle, $gx, $gy, $gw, $gh)
    "converted at ${converted}ms (hidden until then; last hidden size $($last.Rt - $last.L)x$($last.B - $last.T))"
  }
}
[Windows.Forms.Application]::DoEvents(); [void][OW]::SetForegroundWindow($form.Handle)
function Resync { if ($Mode -in 'none','style-only','owner-only','move-only','resize-at-6s','zorder-only') { $r = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r); $script:gx = $r.L; $script:gy = $r.T; $script:gw = $r.Rt - $r.L; $script:gh = $r.B - $r.T; return 0 }; $r = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r); if ($r.L -ne $gx -or $r.T -ne $gy -or ($r.Rt - $r.L) -ne $gw -or ($r.B - $r.T) -ne $gh) { $f = if ($Mode -like '*nocopybits') { 0x0010 -bor 0x0100 } else { 0x0010 }; [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, $gx, $gy, $gw, $gh, $f); return 1 } return 0 }
$resyncs = 0; $samples = @()
if ($Mode -eq 'resize-at-6s') { while (((Get-Date) - $t0).TotalSeconds -lt 6) { [Windows.Forms.Application]::DoEvents(); Resync | Out-Null; Start-Sleep -Milliseconds 100 }; [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, 900, 500, 1280, 720, 0x0004 -bor 0x0010); $gx = 900; $gy = 500; $gw = 1280; $gh = 720; "resized once at 6s" }
if ($Mode -eq 'zorder-only') { while (((Get-Date) - $t0).TotalSeconds -lt 6) { [Windows.Forms.Application]::DoEvents(); Resync | Out-Null; Start-Sleep -Milliseconds 100 }; [void][OW]::SetWindowPos($sdl, [IntPtr]1, 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0010); Start-Sleep -Milliseconds 300; [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0010); "z-order bottom then top at 6s" }
if ($Mode -eq 'move-only') { while (((Get-Date) - $t0).TotalSeconds -lt 6) { [Windows.Forms.Application]::DoEvents(); Resync | Out-Null; Start-Sleep -Milliseconds 100 }; [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, 950, 550, 0, 0, 0x0001 -bor 0x0004 -bor 0x0010); $gx = 950; $gy = 550; "moved again at 6s" }
if ($Mode -eq 'convert-then-kick') { while (((Get-Date) - $t0).TotalSeconds -lt 6) { [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 100 }; [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, $gx, $gy, $gw + 2, $gh + 2, 0x0010); Start-Sleep -Milliseconds 200; [void][OW]::SetWindowPos($sdl, [IntPtr]::Zero, $gx, $gy, $gw, $gh, 0x0010); "kicked at 6s" }
foreach ($at in 8, 14, 20) {
  while (((Get-Date) - $t0).TotalSeconds -lt $at) { [Windows.Forms.Application]::DoEvents(); $resyncs += Resync; Start-Sleep -Milliseconds 100 }
  [void][OW]::SetForegroundWindow($form.Handle); Start-Sleep -Milliseconds 300
  $samples += ('{0}s={1:P0}' -f $at, (NonBlackShare $gx $gy $gw $gh))
  if ($at -eq 14) {
    $b = [Drawing.Bitmap]::new($gw, $gh); $g = [Drawing.Graphics]::FromImage($b); try { $g.CopyFromScreen($gx, $gy, 0, 0, $b.Size); $b.Save("$PSScriptRoot\probe-click\freeze-$Tag-$Mode-14s-screen.png", [Drawing.Imaging.ImageFormat]::Png) } finally { $g.Dispose(); $b.Dispose() }
    $r = [OW+R]::new(); [void][OW]::GetWindowRect($sdl, [ref]$r)
    $pw = [Drawing.Bitmap]::new(($r.Rt - $r.L), ($r.B - $r.T)); $pg = [Drawing.Graphics]::FromImage($pw); $hdc = $pg.GetHdc(); $ok = [OW]::PrintWindow($sdl, $hdc, 2); $pg.ReleaseHdc($hdc); $pg.Dispose()
    $pw.Save("$PSScriptRoot\probe-click\freeze-$Tag-$Mode-14s-printwindow.png", [Drawing.Imaging.ImageFormat]::Png); $pw.Dispose()
    "printwindow(PW_RENDERFULLCONTENT) ok=$ok rect=$($r.L),$($r.T) $($r.Rt - $r.L)x$($r.B - $r.T)"
  }
}
"$Tag dpi=$DpiEnv mode=$Mode window_at=${found}ms resyncs=$resyncs lit: $($samples -join ' ')"
Stop-Process -Id $q.Id -Force -ErrorAction SilentlyContinue
$form.Close(); Start-Sleep -Milliseconds 800
