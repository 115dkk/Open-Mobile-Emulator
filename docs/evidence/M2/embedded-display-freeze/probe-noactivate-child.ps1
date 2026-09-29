# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([Parameter(Mandatory)][long]$Owner, [int]$X, [int]$Y, [int]$W = 400, [int]$H = 300, [int]$Seconds = 25, [int]$Capture = 0)
# Child process: creates one raw Win32 window, converts it into a NOACTIVATE owned popup of the foreign owner,
# and pumps messages for -Seconds. Prints its HWND on the first line. -Capture 1 calls SetCapture on WM_LBUTTONDOWN
# and ReleaseCapture on WM_LBUTTONUP, like SDL does.
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class NC {
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
 [DllImport("user32.dll")] public static extern IntPtr SetCapture(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ReleaseCapture();
 [DllImport("kernel32.dll")] public static extern IntPtr GetModuleHandleW(string n);
 static WndProc keep; public static bool capture;
 public static IntPtr Proc(IntPtr h, uint m, UIntPtr w, IntPtr l) {
  if (capture && m == 0x0201) SetCapture(h);
  if (capture && m == 0x0202) ReleaseCapture();
  return DefWindowProcW(h, m, w, l);
 }
 public static void Register() { keep = Proc; var c = new WNDCLASSEXW(); c.cbSize = (uint)Marshal.SizeOf<WNDCLASSEXW>(); c.lpfnWndProc = keep; c.hInstance = GetModuleHandleW(null); c.lpszClassName = "OmeProbeRawChild"; c.hbrBackground = (IntPtr)8; RegisterClassExW(ref c); }
 public static IntPtr Raw(IntPtr owner, int x, int y, int w, int h) { return CreateWindowExW(0, "OmeProbeRawChild", "raw child", 0x00CF0000u, x, y, w, h, owner, IntPtr.Zero, GetModuleHandleW(null), IntPtr.Zero); }
 public static void Pump() { MSG m; while (PeekMessageW(out m, IntPtr.Zero, 0, 0, 1)) { TranslateMessage(ref m); DispatchMessageW(ref m); } }
 public static void Convert(IntPtr guest, IntPtr owner, int x, int y, int w, int h) {
  long style = GetWindowLongPtrW(guest, -16).ToInt64(); style &= ~(0x00C00000L | 0x00040000L | 0x00020000L | 0x00010000L | 0x00080000L); style |= 0x80000000L; SetWindowLongPtrW(guest, -16, new IntPtr(style));
  long exs = GetWindowLongPtrW(guest, -20).ToInt64(); exs &= ~0x00040000L; exs |= 0x08000000L | 0x00000080L; SetWindowLongPtrW(guest, -20, new IntPtr(exs));
  SetWindowLongPtrW(guest, -8, owner); SetWindowPos(guest, IntPtr.Zero, x, y, w, h, 0x0020 | 0x0040 | 0x0010);
 }
}
'@
[void][NC]::SetProcessDpiAwarenessContext([IntPtr](-4))
[NC]::capture = [bool]$Capture
[NC]::Register()
$h = [NC]::Raw([IntPtr]$Owner, $X, $Y, $W, $H)
[void][NC]::ShowWindow($h, 5); [NC]::Pump(); Start-Sleep -Milliseconds 300; [NC]::Pump()
[NC]::Convert($h, [IntPtr]$Owner, $X, $Y, $W, $H); [NC]::Pump()
"HWND=$([long]$h)"
$end = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $end) { [NC]::Pump(); Start-Sleep -Milliseconds 10 }
