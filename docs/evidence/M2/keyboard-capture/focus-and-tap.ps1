# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([int]$AppPid)
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class KI {
 [StructLayout(LayoutKind.Sequential)] public struct R {public int L,T,Rt,B;}
 [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {public ushort vk,scan; public uint flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Explicit)] public struct UNION {[FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi;}
 [StructLayout(LayoutKind.Sequential)] public struct INPUT {public uint type; public UNION u;}
 [DllImport("user32.dll",SetLastError=true)] public static extern uint SendInput(uint n,INPUT[] inputs,int size);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int he,uint flags);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 public static uint Tap(ushort scan, bool extended) {
  uint f = 8u | (extended ? 1u : 0u);
  var d = new INPUT {type=1,u=new UNION {ki=new KEYBDINPUT {scan=scan,flags=f}}};
  var u = new INPUT {type=1,u=new UNION {ki=new KEYBDINPUT {scan=scan,flags=f|2u}}};
  uint a = SendInput(1,new[]{d},Marshal.SizeOf<INPUT>()); System.Threading.Thread.Sleep(120);
  uint b = SendInput(1,new[]{u},Marshal.SizeOf<INPUT>()); return a+b;
 }
 public static bool Focus(IntPtr root) {
  ShowWindow(root, 9); SetForegroundWindow(root);
  for (int attempt = 0; attempt < 3 && GetForegroundWindow() != root; attempt++) {
   SetWindowPos(root, new IntPtr(-1), 0,0,0,0, 0x43);
   try {
    System.Threading.Thread.Sleep(250);
    R r; GetWindowRect(root, out r); SetCursorPos(r.L + Math.Min(400, (r.Rt - r.L) / 2), r.T + 30);
    var down = new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {flags=2}}};
    var up = new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {flags=4}}};
    SendInput(2, new[]{down, up}, Marshal.SizeOf<INPUT>());
    System.Threading.Thread.Sleep(180);
   } finally { SetWindowPos(root, new IntPtr(-2), 0,0,0,0, 0x53); }
  }
  return GetForegroundWindow() == root;
 }
}
'@
$root = (Get-Process -Id $AppPid).MainWindowHandle
$ok = [KI]::Focus($root)
"foreground=$ok"
if (-not $ok) { "app is not foreground; no keys sent"; exit 2 }
foreach ($k in @(@('F13',0x64,$false), @('DOWN',0x50,$true), @('HOME',0x47,$true), @('ENTER',0x1c,$false), @('F12',0x58,$false), @('A',0x1e,$false))) {
 if ([KI]::GetForegroundWindow() -ne $root) { "foreground lost before $($k[0]); stopping"; exit 3 }
 "$($k[0]): $([KI]::Tap([uint16]$k[1], [bool]$k[2]))"; Start-Sleep -Milliseconds 300
}
