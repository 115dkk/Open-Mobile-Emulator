# SPDX-License-Identifier: GPL-2.0-or-later
param([int]$QemuPid)
. (Join-Path $PSScriptRoot "move.ps1") -QemuPid $QemuPid -Points "0.5,0.5" | Out-Null
foreach($pt in @('0.02,0.02','0.1,0.1','0.5,0.5','0.9,0.9','0.98,0.98','0.25,0.75','0.75,0.25')){ $f=$pt.Split(','); $x=[int]($r.L+[double]$f[0]*($r.Rt-$r.L)); $y=[int]($r.T+[double]$f[1]*($r.B-$r.T))
 [M]::SetCursorPos($x-2,$y)|Out-Null; Start-Sleep -Milliseconds 100; [M]::SetCursorPos($x,$y)|Out-Null; Start-Sleep -Milliseconds 300
 $v = & $env:OMEADB -s 127.0.0.1:5555 shell "getevent -pl /dev/input/event2" | Select-String 'ABS_[XY] ' | ForEach-Object { ($_ -split 'value ')[1].Split(',')[0] }
 "$pt client=($($x-$r.L),$($y-$r.T)) abs=$($v -join ',')" }
