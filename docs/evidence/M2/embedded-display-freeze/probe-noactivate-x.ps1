# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Cross-process variant: the NOACTIVATE owned popup lives in a child pwsh process; this process owns the owner
# window and injects the clicks. Two children: without and with SetCapture on button down (SDL captures).
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class NX {
 [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {public ushort vk,scan; public uint flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Explicit)] public struct UNION {[FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi;}
 [StructLayout(LayoutKind.Sequential)] public struct INPUT {public uint type; public UNION u;}
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll",SetLastError=true)] public static extern uint SendInput(uint n,INPUT[] inputs,int size);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref System.Drawing.Point p);
 public static uint Mouse(uint flags) { var d = new INPUT { type = 0, u = new UNION { mi = new MOUSEINPUT { flags = flags } } }; return SendInput(1, new[] { d }, Marshal.SizeOf<INPUT>()); }
}
public class OwnerForm : System.Windows.Forms.Form {
 public static IntPtr Target; public static int Answered;
 protected override void WndProc(ref System.Windows.Forms.Message m) {
  if (m.Msg == 0x0021 && Target != IntPtr.Zero && m.WParam == Target) { Answered++; m.Result = (IntPtr)3; return; }
  base.WndProc(ref m);
 }
}
'@ -ReferencedAssemblies ([System.Drawing.Point].Assembly.Location, [System.Windows.Forms.Form].Assembly.Location, [System.Windows.Forms.Message].Assembly.Location, 'System.Runtime', 'System.ComponentModel.Primitives')
"process pmv2=$([NX]::SetProcessDpiAwarenessContext([IntPtr](-4)))"
$scratch = 'C:\Users\32170336\AppData\Local\Temp\claude\C--Open-Mobile-Emulator\7758e147-b2c0-5152-846b-245148981fae\scratchpad'
$owner = [OwnerForm]::new(); $owner.Text = 'owner'; $owner.StartPosition = 'Manual'; $owner.Location = [Drawing.Point]::new(60, 60)
$owner.ClientSize = [Drawing.Size]::new(1200, 600); $owner.BackColor = [Drawing.Color]::DarkGreen; $owner.Show(); [Windows.Forms.Application]::DoEvents()
$origin = [Drawing.Point]::new(0, 0); [void][NX]::ClientToScreen($owner.Handle, [ref]$origin)
function Fg([long]$h) { $f = [NX]::GetForegroundWindow(); if ($f -eq $owner.Handle) { 'owner' } elseif ($f.ToInt64() -eq $h) { 'POPUP' } else { "other:$f" } }
function Case([string]$name, [long]$h, [int]$x, [int]$y) {
  [void][NX]::SetForegroundWindow($owner.Handle); Start-Sleep -Milliseconds 400; [Windows.Forms.Application]::DoEvents()
  [NX]::SetCursorPos($x - 16, $y - 16) | Out-Null; Start-Sleep -Milliseconds 150; [NX]::SetCursorPos($x, $y) | Out-Null; Start-Sleep -Milliseconds 300
  $hover = Fg $h
  $sd=[NX]::Mouse(2); Start-Sleep -Milliseconds 400; [Windows.Forms.Application]::DoEvents(); $down = Fg $h
  $su=[NX]::Mouse(4); Start-Sleep -Milliseconds 400; [Windows.Forms.Application]::DoEvents(); $up = Fg $h
  "$name hover=$hover afterDown=$down afterUp=$up sent=$sd/$su"
}
function Child([string]$tag, [int]$x, [int]$y, [int]$capture) {
  # The owner thread must keep pumping while the child creates and converts an owned window (cross-thread
  # window operations send messages to the owner), so never block on the child's stdout: poll a file.
  $log = "$scratch\probe-noactivate-$tag.txt"
  Remove-Item $log -ErrorAction SilentlyContinue
  $p = Start-Process -FilePath 'pwsh' -ArgumentList "-NoProfile -File `"$scratch\probe-noactivate-child.ps1`" -Owner $($owner.Handle.ToInt64()) -X $x -Y $y -Seconds 30 -Capture $capture" -PassThru -RedirectStandardOutput $log -RedirectStandardError "$log.err"
  $deadline = (Get-Date).AddSeconds(20); $hwnd = 0
  while ($hwnd -eq 0 -and (Get-Date) -lt $deadline) {
    [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 100
    $line = Get-Content $log -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($line -match '^HWND=(\d+)$') { $hwnd = [long]$Matches[1] }
  }
  if ($hwnd -eq 0) { throw "child $tag did not report: $(Get-Content "$log.err" -ErrorAction SilentlyContinue | Select-Object -First 3)" }
  [pscustomobject]@{ Process = $p; Hwnd = $hwnd }
}
$a = Child 'a' ($origin.X + 20) ($origin.Y + 20) 0
$b = Child 'b' ($origin.X + 500) ($origin.Y + 20) 1
Start-Sleep -Milliseconds 500; [Windows.Forms.Application]::DoEvents()
[void][NX]::SetForegroundWindow($owner.Handle); Start-Sleep -Milliseconds 300
"A=$($a.Hwnd) (no capture) B=$($b.Hwnd) (SetCapture on down) foreground=$(Fg 0)"
Case 'X-A-other-process' $a.Hwnd ($origin.X + 220) ($origin.Y + 170)
Case 'X-B-other-process-capture' $b.Hwnd ($origin.X + 700) ($origin.Y + 170)
Case 'X-A-again' $a.Hwnd ($origin.X + 220) ($origin.Y + 170)
[OwnerForm]::Target = [IntPtr]$a.Hwnd
Case 'X-A-OWNER-answers-MA_NOACTIVATE' $a.Hwnd ($origin.X + 220) ($origin.Y + 170)
[OwnerForm]::Target = [IntPtr]$b.Hwnd
Case 'X-B-capture-OWNER-answers' $b.Hwnd ($origin.X + 700) ($origin.Y + 170)
"owner answered $([OwnerForm]::Answered) time(s)"
foreach ($c in $a, $b) { try { $c.Process.Kill() } catch {} }
$owner.Close()
