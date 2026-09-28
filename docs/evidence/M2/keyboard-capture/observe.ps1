# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([int]$Seconds = 40)
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class OB {
 public delegate IntPtr Proc(int code, IntPtr w, IntPtr l);
 [StructLayout(LayoutKind.Sequential)] public struct KB { public uint vk, scan, flags, time; public UIntPtr extra; }
 [StructLayout(LayoutKind.Sequential)] public struct MSG { public IntPtr h; public uint m; public IntPtr w, l; public uint t; public int x, y; }
 [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SetWindowsHookEx(int id, Proc p, IntPtr mod, uint tid);
 [DllImport("user32.dll")] public static extern bool UnhookWindowsHookEx(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr CallNextHookEx(IntPtr h, int code, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern bool PeekMessage(out MSG m, IntPtr h, uint a, uint b, uint r);
 [DllImport("user32.dll")] public static extern bool TranslateMessage(ref MSG m);
 [DllImport("user32.dll")] public static extern IntPtr DispatchMessage(ref MSG m);
 static Proc keep; static IntPtr hook;
 public static System.Collections.Generic.List<string> Seen = new System.Collections.Generic.List<string>();
 static IntPtr Cb(int code, IntPtr w, IntPtr l) {
  if (code >= 0) { var k = (KB)Marshal.PtrToStructure(l, typeof(KB)); Seen.Add(DateTime.UtcNow.ToString("HH:mm:ss.fff") + " msg=0x" + w.ToInt64().ToString("x") + " vk=0x" + k.vk.ToString("x") + " scan=0x" + k.scan.ToString("x") + " flags=0x" + k.flags.ToString("x")); }
  return CallNextHookEx(hook, code, w, l);
 }
 public static string Run(int seconds) {
  keep = Cb; hook = SetWindowsHookEx(13, keep, IntPtr.Zero, 0);
  if (hook == IntPtr.Zero) return "install failed " + Marshal.GetLastWin32Error();
  var end = DateTime.UtcNow.AddSeconds(seconds); MSG m;
  while (DateTime.UtcNow < end) { while (PeekMessage(out m, IntPtr.Zero, 0, 0, 1)) { TranslateMessage(ref m); DispatchMessage(ref m); } System.Threading.Thread.Sleep(5); }
  UnhookWindowsHookEx(hook); return "installed, " + Seen.Count + " events";
 }
}
'@
"observer start $([DateTime]::UtcNow.ToString('HH:mm:ss.fff')) pid=$PID"
[OB]::Run($Seconds)
[OB]::Seen
