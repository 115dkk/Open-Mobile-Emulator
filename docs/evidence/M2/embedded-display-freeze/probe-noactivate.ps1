# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Does WS_EX_NOACTIVATE stop mouse-click activation on this host, without QEMU or SDL?
# Three owned popups over a WinForms owner: (A) WinForms form with NOACTIVATE at creation (WinForms sets capture
# on mouse down), (B) raw Win32 window with NOACTIVATE at creation and DefWindowProc only, (C) raw window created
# WITHOUT the style, then converted like the product does (SetWindowLongPtr + GWLP_HWNDPARENT + SWP_FRAMECHANGED).
# Each is clicked with SendInput; foreground is read 400 ms after the down and after the up.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies ([System.Windows.Forms.Form].Assembly.Location, [System.Windows.Forms.Message].Assembly.Location, [System.Drawing.Point].Assembly.Location, 'System.Runtime', 'System.ComponentModel.Primitives') -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class NA {
 [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {public ushort vk,scan; public uint flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Explicit)] public struct UNION {[FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi;}
 [StructLayout(LayoutKind.Sequential)] public struct INPUT {public uint type; public UNION u;}
 [StructLayout(LayoutKind.Sequential)] public struct MSG {public IntPtr hwnd; public uint message; public UIntPtr wParam; public IntPtr lParam; public uint time; public int x,y;}
 public delegate IntPtr WndProc(IntPtr h, uint m, UIntPtr w, IntPtr l);
 [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct WNDCLASSEXW { public uint cbSize, style; public WndProc lpfnWndProc; public int cbClsExtra, cbWndExtra; public IntPtr hInstance, hIcon, hCursor, hbrBackground; public string lpszMenuName, lpszClassName; public IntPtr hIconSm; }
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern ushort RegisterClassExW(ref WNDCLASSEXW c);
 [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr CreateWindowExW(uint ex, string cls, string name, uint style, int x, int y, int w, int h, IntPtr parent, IntPtr menu, IntPtr inst, IntPtr param);
 [DllImport("user32.dll")] public static extern IntPtr DefWindowProcW(IntPtr h, uint m, UIntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern bool PeekMessageW(out MSG m, IntPtr h, uint a, uint b, uint remove);
 [DllImport("user32.dll")] public static extern bool TranslateMessage(ref MSG m);
 [DllImport("user32.dll")] public static extern IntPtr DispatchMessageW(ref MSG m);
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
 [DllImport("kernel32.dll")] public static extern IntPtr GetModuleHandleW(string n);
 static WndProc keep;
 public static bool answerNoActivate;
 public static IntPtr Proc(IntPtr h, uint m, UIntPtr w, IntPtr l) { if (answerNoActivate && m == 0x0021) return (IntPtr)3; return DefWindowProcW(h, m, w, l); }
 public static void Register() { keep = Proc; var c = new WNDCLASSEXW(); c.cbSize = (uint)Marshal.SizeOf<WNDCLASSEXW>(); c.lpfnWndProc = keep; c.hInstance = GetModuleHandleW(null); c.lpszClassName = "OmeProbeRaw"; c.hbrBackground = (IntPtr)6; RegisterClassExW(ref c); }
 public static IntPtr Raw(uint ex, IntPtr owner, int x, int y, int w, int h) { return CreateWindowExW(ex, "OmeProbeRaw", "raw", 0x80000000u | 0x10000000u, x, y, w, h, owner, IntPtr.Zero, GetModuleHandleW(null), IntPtr.Zero); }
 public static void Pump() { MSG m; while (PeekMessageW(out m, IntPtr.Zero, 0, 0, 1)) { TranslateMessage(ref m); DispatchMessageW(ref m); } }
 public static void Convert(IntPtr guest, IntPtr owner, int x, int y, int w, int h) {
  long style = GetWindowLongPtrW(guest, -16).ToInt64(); style &= ~(0x00C00000L | 0x00040000L | 0x00020000L | 0x00010000L | 0x00080000L); style |= 0x80000000L; SetWindowLongPtrW(guest, -16, new IntPtr(style));
  long exs = GetWindowLongPtrW(guest, -20).ToInt64(); exs &= ~0x00040000L; exs |= 0x08000000L | 0x00000080L; SetWindowLongPtrW(guest, -20, new IntPtr(exs));
  SetWindowLongPtrW(guest, -8, owner); SetWindowPos(guest, IntPtr.Zero, x, y, w, h, 0x0020 | 0x0040 | 0x0010);
 }
 public static uint Mouse(uint flags) { var d = new INPUT { type = 0, u = new UNION { mi = new MOUSEINPUT { flags = flags } } }; return SendInput(1, new[] { d }, Marshal.SizeOf<INPUT>()); }
}
public class OwnerForm : System.Windows.Forms.Form {
 public static IntPtr Target; public static int Answered; public static int Seen; public static IntPtr LastWParam;
 protected override void WndProc(ref System.Windows.Forms.Message m) {
  if (m.Msg == 0x0021) { Seen++; LastWParam = m.WParam; if (Target != IntPtr.Zero && m.WParam == Target) { Answered++; m.Result = (IntPtr)3; return; } }
  base.WndProc(ref m);
 }
}
public class NaForm : System.Windows.Forms.Form {
 protected override System.Windows.Forms.CreateParams CreateParams { get { var p = base.CreateParams; p.ExStyle |= 0x08000000 | 0x00000080; return p; } }
 protected override bool ShowWithoutActivation { get { return true; } }
}
'@
"process pmv2=$([NA]::SetProcessDpiAwarenessContext([IntPtr](-4)))"
$owner = [OwnerForm]::new(); $owner.Text = 'owner'; $owner.StartPosition = 'Manual'; $owner.Location = [Drawing.Point]::new(60, 60)
$owner.ClientSize = [Drawing.Size]::new(1500, 700); $owner.BackColor = [Drawing.Color]::DarkGreen; $owner.Show(); [Windows.Forms.Application]::DoEvents()
$origin = [Drawing.Point]::new(0, 0); [void][NA]::ClientToScreen($owner.Handle, [ref]$origin)
function Pump { [Windows.Forms.Application]::DoEvents(); [NA]::Pump() }
function Fg([IntPtr]$h) { $f = [NA]::GetForegroundWindow(); if ($f -eq $owner.Handle) { 'owner' } elseif ($f -eq $h) { 'POPUP' } else { "other:$f" } }
function Case([string]$name, [IntPtr]$h, [int]$x, [int]$y) {
  [void][NA]::SetForegroundWindow($owner.Handle); Start-Sleep -Milliseconds 400; Pump
  [NA]::SetCursorPos($x - 16, $y - 16) | Out-Null; Start-Sleep -Milliseconds 150; Pump; [NA]::SetCursorPos($x, $y) | Out-Null; Start-Sleep -Milliseconds 300; Pump
  $hover = Fg $h
  $sd=[NA]::Mouse(2); Start-Sleep -Milliseconds 400; Pump; $down = Fg $h
  $su=[NA]::Mouse(4); Start-Sleep -Milliseconds 400; Pump; $up = Fg $h
  "$name hover=$hover afterDown=$down afterUp=$up sent=$sd/$su"
}
# (A) WinForms popup with NOACTIVATE at creation
$a = [NaForm]::new(); $a.FormBorderStyle = 'None'; $a.StartPosition = 'Manual'; $a.Owner = $owner
$a.Location = [Drawing.Point]::new(0, 0); $a.Size = [Drawing.Size]::new(400, 300); $a.BackColor = [Drawing.Color]::Orange; $a.Show(); Pump
[void][NA]::SetWindowPos($a.Handle, [IntPtr]::Zero, $origin.X + 20, $origin.Y + 20, 400, 300, 0x0010 -bor 0x0004)
# (B) raw window with NOACTIVATE at creation
[NA]::Register()
$b = [NA]::Raw(0x08000000 -bor 0x00000080, $owner.Handle, $origin.X + 500, $origin.Y + 20, 400, 300)
[void][NA]::ShowWindow($b, 4); Pump
# (C) raw window created plainly, then converted like the product does
$c = [NA]::Raw(0, $owner.Handle, $origin.X + 980, $origin.Y + 20, 400, 300)
[void][NA]::ShowWindow($c, 4); Pump
[NA]::Convert($c, $owner.Handle, $origin.X + 980, $origin.Y + 20, 400, 300); Pump
"A=$($a.Handle) B=$b C=$c"
Case 'A-winforms-noactivate-at-create' $a.Handle ($origin.X + 220) ($origin.Y + 170)
Case 'B-raw-noactivate-at-create' $b ($origin.X + 700) ($origin.Y + 170)
Case 'C-raw-converted-after-create' $c ($origin.X + 1180) ($origin.Y + 170)
Case 'A-again' $a.Handle ($origin.X + 220) ($origin.Y + 170)
# (D) raw NOACTIVATE window with NO owner (the on-screen-keyboard case)
$d = [NA]::CreateWindowExW(0x08000000 -bor 0x00000080, 'OmeProbeRaw', 'raw-unowned', [uint32]2147483648, $origin.X + 20, $origin.Y + 350, 400, 300, [IntPtr]::Zero, [IntPtr]::Zero, [NA]::GetModuleHandleW($null), [IntPtr]::Zero)
[void][NA]::ShowWindow($d, 4); Pump
Case 'D-raw-noactivate-unowned' $d ($origin.X + 220) ($origin.Y + 500)
# (E) raw owned + converted, but the window procedure answers WM_MOUSEACTIVATE with MA_NOACTIVATE
[NA]::answerNoActivate = $true
$e = [NA]::Raw(0, $owner.Handle, $origin.X + 500, $origin.Y + 350, 400, 300)
[void][NA]::ShowWindow($e, 4); Pump
[NA]::Convert($e, $owner.Handle, $origin.X + 500, $origin.Y + 350, 400, 300); Pump
Case 'E-owned-proc-MA_NOACTIVATE' $e ($origin.X + 700) ($origin.Y + 500)
Case 'C-again-now-proc-answers' $c ($origin.X + 1180) ($origin.Y + 170)
# (F) raw owned + converted, DefWindowProc only; the OWNER answers MA_NOACTIVATE for this popup
[NA]::answerNoActivate = $false
$f = [NA]::Raw(0, $owner.Handle, $origin.X + 980, $origin.Y + 350, 400, 300)
[void][NA]::ShowWindow($f, 4); Pump
[NA]::Convert($f, $owner.Handle, $origin.X + 980, $origin.Y + 350, 400, 300); Pump
[OwnerForm]::Target = $f
Case 'F-owned-OWNER-answers-MA_NOACTIVATE' $f ($origin.X + 1180) ($origin.Y + 500)
"owner answered $([OwnerForm]::Answered) time(s); owner saw WM_MOUSEACTIVATE $([OwnerForm]::Seen) time(s), last wParam=$([OwnerForm]::LastWParam) (owner=$($owner.Handle) popupF=$f)"
[OwnerForm]::Target = [IntPtr]::Zero
Case 'F-again-owner-not-answering' $f ($origin.X + 1180) ($origin.Y + 500)
$owner.Close()
