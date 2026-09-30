# SPDX-License-Identifier: GPL-2.0-or-later
# Runs the Get-GrubHighlight functions of ci/dod/native.ps1 (copied below at commit time) over raw RGB dumps of
# captured GRUB frames through a mocked bitmap, on Linux pwsh 7.5.2. Results, 2026-09-30: rows 0, 7, 4, 3, none, 0.
function Test-GrubBlue($c) { return ($c.B -ge 120) -and ($c.B -gt ($c.R + 50)) -and ($c.B -gt ($c.G + 20)) }
function Add-GrubEntry($entries, $cluster, $h) {
 $span = $cluster[-1] - $cluster[0]; $center = ($cluster[0] + $cluster[-1]) / 2
 if ($span -ge 6 -and $span -le 0.1 * $h -and $center -lt 0.9 * $h) { $entries.Add([double]$center) }
}
function Get-GrubHighlight($b) {
 $w = $b.Width; $h = $b.Height
 $x0 = [int](0.25 * $w); $x1 = [int](0.75 * $w)
 $barRows = New-Object System.Collections.Generic.List[int]
 for ($y = 0; $y -lt $h; $y += 3) {
  $n = 0; $m = 0
  for ($x = $x0; $x -lt $x1; $x += 4) { $m++; if (Test-GrubBlue $b.GetPixel($x, $y)) { $n++ } }
  if ($m -gt 0 -and ($n * 2) -gt $m) { $barRows.Add($y) }
 }
 $clusters = @(); $cur = $null
 foreach ($y in $barRows) {
  if ($null -ne $cur -and ($y - $cur[-1]) -le 6) { $cur += $y } else { if ($null -ne $cur) { $clusters += ,$cur }; $cur = @($y) }
 }
 if ($null -ne $cur) { $clusters += ,$cur }
 $big = $null; foreach ($c in $clusters) { if ($null -eq $big -or $c.Count -gt $big.Count) { $big = $c } }
 $bar = -1; $barHeight = 0
 if ($null -ne $big) { $bar = ($big[0] + $big[-1]) / 2; $barHeight = $big[-1] - $big[0] }
 $b0 = [int](0.19 * $w); $b1 = [int](0.24 * $w)
 $entries = New-Object System.Collections.Generic.List[double]; $cur = $null
 for ($y = 0; $y -lt $h; $y++) {
  $n = 0
  for ($x = $b0; $x -lt $b1; $x += 3) {
   $c = $b.GetPixel($x, $y)
   if ((Test-GrubBlue $c) -or ($c.R -gt 200 -and $c.G -gt 200 -and $c.B -gt 200)) { $n++; if ($n -ge 2) { break } }
  }
  if ($n -ge 2) {
   if ($null -ne $cur -and ($y - $cur[-1]) -le 3) { $cur += $y } else { if ($null -ne $cur) { Add-GrubEntry $entries $cur $h }; $cur = @($y) }
  }
 }
 if ($null -ne $cur) { Add-GrubEntry $entries $cur $h }
 $row = -1
 if ($bar -ge 0 -and $entries.Count -gt 0) {
  $best = [double]::MaxValue
  for ($i = 0; $i -lt $entries.Count; $i++) { $d = [Math]::Abs($entries[$i] - $bar); if ($d -lt $best) { $best = $d; $row = $i } }
  if ($best -gt 0.03 * $h) { $row = -1 }
 }
 return @{ bar = $bar; barHeight = $barHeight; entries = @($entries | ForEach-Object { [int]$_ }); row = $row }
}

class Px { [int]$R; [int]$G; [int]$B }
class MockBitmap {
 [int]$Width; [int]$Height; [byte[]]$Data
 MockBitmap([string]$path) {
  $bytes = [IO.File]::ReadAllBytes($path)
  $this.Width = [BitConverter]::ToInt32($bytes, 0); $this.Height = [BitConverter]::ToInt32($bytes, 4)
  $this.Data = $bytes
 }
 [Px] GetPixel([int]$x, [int]$y) {
  $o = 8 + ($y * $this.Width + $x) * 3
  $p = [Px]::new(); $p.R = $this.Data[$o]; $p.G = $this.Data[$o+1]; $p.B = $this.Data[$o+2]; return $p
 }
}
foreach ($case in $args) {
 $b = [MockBitmap]::new("px-$case.raw")
 $sw = [Diagnostics.Stopwatch]::StartNew()
 $r = Get-GrubHighlight $b
 "$case ($($b.Width)x$($b.Height)) $([int]$sw.ElapsedMilliseconds) ms -> " + (@{highlight=$r} | ConvertTo-Json -Depth 6 -Compress)
}
