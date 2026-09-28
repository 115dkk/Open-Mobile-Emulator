# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
param([string]$Scratch)
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class KI {
 [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {public ushort vk,scan; public uint flags,time; public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Explicit)] public struct UNION {[FieldOffset(0)] public KEYBDINPUT ki; [FieldOffset(0)] public MOUSEINPUT mi;}
 [StructLayout(LayoutKind.Sequential)] public struct INPUT {public uint type; public UNION u;}
 [DllImport("user32.dll",SetLastError=true)] public static extern uint SendInput(uint n,INPUT[] inputs,int size);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int cmd);
 public static uint Tap(ushort scan, bool extended) {
  uint f = 8u | (extended ? 1u : 0u);
  var d = new INPUT {type=1,u=new UNION {ki=new KEYBDINPUT {scan=scan,flags=f}}};
  var u = new INPUT {type=1,u=new UNION {ki=new KEYBDINPUT {scan=scan,flags=f|2u}}};
  uint a = SendInput(1,new[]{d},Marshal.SizeOf<INPUT>()); System.Threading.Thread.Sleep(120);
  uint b = SendInput(1,new[]{u},Marshal.SizeOf<INPUT>()); return a+b;
 }
}
'@
$obs = Start-Process -FilePath 'pwsh' -ArgumentList @('-NoProfile','-File',"$Scratch\observe.ps1",'-Seconds','32') -PassThru -RedirectStandardOutput "$Scratch\probeF-observer.txt" -RedirectStandardError "$Scratch\probeF-observer-err.txt"
$env:OME_HOME = "$Scratch\probe-home"
$p = Start-Process -FilePath 'C:\Open Mobile Emulator\host\target\debug\ome.exe' -WorkingDirectory 'C:\Open Mobile Emulator\host' -PassThru -RedirectStandardError "$Scratch\probeF-stderr.txt" -RedirectStandardOutput "$Scratch\probeF-stdout.txt"
Start-Sleep -Seconds 12
$root = (Get-Process -Id $p.Id).MainWindowHandle
$prev = [KI]::GetForegroundWindow()
"$([DateTime]::UtcNow.ToString('HH:mm:ss.fff')) A other foreground F13 0x64: $([KI]::Tap(0x64, $false))"; Start-Sleep -Seconds 1
[void][KI]::ShowWindow($root, 9); [void][KI]::SetForegroundWindow($root); Start-Sleep -Milliseconds 500
"$([DateTime]::UtcNow.ToString('HH:mm:ss.fff')) B app foreground=$([KI]::GetForegroundWindow() -eq $root) F14 0x65: $([KI]::Tap(0x65, $false))"; Start-Sleep -Seconds 1
"$([DateTime]::UtcNow.ToString('HH:mm:ss.fff')) B2 app foreground DOWN ext: $([KI]::Tap(0x50, $true))"; Start-Sleep -Seconds 1
[void][KI]::SetForegroundWindow($prev); Start-Sleep -Milliseconds 500
"$([DateTime]::UtcNow.ToString('HH:mm:ss.fff')) C other foreground=$([KI]::GetForegroundWindow() -eq $prev) F15 0x66: $([KI]::Tap(0x66, $false))"; Start-Sleep -Seconds 2
Stop-Process -Id $p.Id -Force -Confirm:$false -ErrorAction SilentlyContinue
$obs.WaitForExit(40000) | Out-Null
"== app stderr (hook lines)"
Get-Content "$Scratch\probeF-stderr.txt" | Select-String '\[input\] hook' | ForEach-Object { $_.Line }
"== external observer"
Get-Content "$Scratch\probeF-observer.txt"
Get-Content "$Scratch\probeF-observer-err.txt" | Select-Object -First 5
