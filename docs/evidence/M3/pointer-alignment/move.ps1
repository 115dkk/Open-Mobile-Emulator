# SPDX-License-Identifier: GPL-2.0-or-later
param([int]$QemuPid,[string]$Points)
Add-Type @'
using System; using System.Text; using System.Runtime.InteropServices;
public class M {
 [StructLayout(LayoutKind.Sequential)] public struct R {public int L,T,Rt,B;}
 public delegate bool E(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(E cb,IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out R r);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out R r);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int c);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f,int dx,int dy,uint d,UIntPtr e);
}
'@
[M]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null
$found=[IntPtr]::Zero
[M]::EnumWindows({param($h,$p) $q=0;[M]::GetWindowThreadProcessId($h,[ref]$q)|Out-Null; $s=[Text.StringBuilder]::new(64);[M]::GetClassName($h,$s,64)|Out-Null; if($q -eq $QemuPid -and $s.ToString() -eq 'SDL_app' -and [M]::IsWindowVisible($h)){$script:found=$h}; $true},[IntPtr]::Zero)|Out-Null
$r=New-Object M+R; [M]::GetWindowRect($found,[ref]$r)|Out-Null
"window $($r.L),$($r.T) $($r.Rt-$r.L)x$($r.B-$r.T)"
foreach($pt in $Points.Split(';')){ $f=$pt.Split(','); $x=[int]($r.L+[double]$f[0]*($r.Rt-$r.L)); $y=[int]($r.T+[double]$f[1]*($r.B-$r.T))
 [M]::SetCursorPos($x-3,$y)|Out-Null; Start-Sleep -Milliseconds 300; [M]::SetCursorPos($x,$y)|Out-Null; Start-Sleep -Milliseconds 1500; "pt $pt -> screen $x,$y" }
