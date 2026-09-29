# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Holds the display awake for the completion run. The driver starts this helper and kills it at the end.
# A sleeping monitor stops DWM composition, so every screen capture would show stale content
# (docs/evidence/M2/embedded-display-freeze.md, 2026-09-30).
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class KA {
 [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);
 [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {public ushort vk,scan; public uint flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Explicit)] public struct UNION {[FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi;}
 [StructLayout(LayoutKind.Sequential)] public struct INPUT {public uint type; public UNION u;}
 [DllImport("user32.dll",SetLastError=true)] public static extern uint SendInput(uint n,INPUT[] inputs,int size);
 public static uint Jiggle() { var a=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {x=1,y=0,flags=1}}}; var b=new INPUT {type=0,u=new UNION {mi=new MOUSEINPUT {x=-1,y=0,flags=1}}}; return SendInput(2,new[]{a,b},Marshal.SizeOf<INPUT>()); }
}
'@
# ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED
$previous = [KA]::SetThreadExecutionState([uint32]2147483651 -bor [uint32]1)
$sent = [KA]::Jiggle()
Write-Output ('{{"keepAwake":true,"previousState":"0x{0:X}","jiggleSent":{1}}}' -f $previous, $sent)
while ($true) { Start-Sleep -Seconds 30 }
