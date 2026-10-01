# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
#Requires -Version 7
# ADR-0010 spike launcher.
#   -Step install : headless helper boot (ISO kernel + initrd.img + appended OME cpio) that installs
#                   into a fresh disk.qcow2; waits for QEMU to exit; prints the serial log.
#   -Step boot    : boots the installed disk with direct kernel boot and the product's display
#                   arguments, waits for sys.boot_completed over adb, gathers facts, powers down.
param(
  [ValidateSet('install', 'boot', 'grub')] [string]$Step = 'install',
  [string]$GrubDisk = '',
  [int]$TimeoutSec = 900,
  [switch]$SeaBios,
  [string]$Tag = 'a',
  [int]$AdbPort = 5556,
  [int]$QmpPort = 4446
)
$ErrorActionPreference = 'Stop'
$sp = 'C:\Users\32170336\AppData\Local\Temp\claude\C--Open-Mobile-Emulator\27d99c8c-b9c9-593b-9f42-b33a98337195\scratchpad\spike-adr10'
$qemuDir = 'C:\Open Mobile Emulator\host\target\debug\qemu'
$qemu = "$qemuDir\bin\qemu-system-x86_64.exe"
$qemuImg = "$qemuDir\bin\qemu-img.exe"
$code = "$qemuDir\share\qemu\edk2-x86_64-code.fd"
$varsTemplate = "$qemuDir\share\qemu\edk2-i386-vars.fd"
$iso = 'C:\Users\32170336\AppData\Local\OpenMobileEmulator\artifacts\Bliss-v16.9.7-x86_64-OFFICIAL-gapps-20241011.iso'
$disk = "$sp\disk.qcow2"
$vars = "$sp\efivars.fd"
$kernel = "$sp\iso\kernel"
$initrd = "$sp\iso\initrd.img"
$initrdInstall = "$sp\initrd-install.img"
$adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"

function Q([string]$s) { '"' + $s + '"' }
function Log([string]$s) { $line = "{0:HH:mm:ss.fff} {1}" -f (Get-Date), $s; Write-Host $line; Add-Content -Path "$sp\spike-$Step-$Tag.txt" -Value $line }

$common = @('-machine', 'q35', '-accel', 'whpx,kernel-irqchip=off', '-cpu', 'Skylake-Client-v4', '-rtc', 'base=utc', '-parallel', 'none', '-action', 'reboot=shutdown')
$fw = if ($SeaBios) { @() } else { @('-drive', (Q "if=pflash,format=raw,readonly=on,file=$code"), '-drive', (Q "if=pflash,format=raw,file=$vars")) }

function Start-Qemu([string[]]$argv, [string]$stderrPath) {
  $cmd = ($argv -join ' ')
  Log "qemu $cmd"
  return Start-Process -FilePath $qemu -ArgumentList $cmd -PassThru -NoNewWindow -RedirectStandardError $stderrPath
}

function Send-Qmp([int]$port, [string[]]$commands) {
  $client = [System.Net.Sockets.TcpClient]::new('127.0.0.1', $port)
  $stream = $client.GetStream()
  $reader = [System.IO.StreamReader]::new($stream)
  $writer = [System.IO.StreamWriter]::new($stream)
  $writer.AutoFlush = $true
  $null = $reader.ReadLine()
  $writer.WriteLine('{"execute":"qmp_capabilities"}')
  $null = $reader.ReadLine()
  foreach ($c in $commands) { $writer.WriteLine($c); Log ("qmp " + $reader.ReadLine()) }
  $client.Close()
}

if ($Step -eq 'install') {
  if (Test-Path $disk) { Remove-Item $disk -Force }
  & $qemuImg create -f qcow2 $disk 32G | Out-Null
  Copy-Item $varsTemplate $vars -Force
  $serial = "$sp\install-serial-$Tag.log"
  if (Test-Path $serial) { Remove-Item $serial -Force }
  $argv = $common + $fw + @(
    '-m', '2048', '-smp', '2',
    '-drive', (Q "file=$disk,if=virtio,format=qcow2"),
    '-drive', (Q "file=$iso,media=cdrom,if=none,id=cd0,readonly=on"), '-device', 'ide-cd,drive=cd0,bus=ide.0',
    '-kernel', (Q $kernel), '-initrd', (Q $initrdInstall),
    '-append', (Q 'root=/dev/ram0 console=ttyS0 OME_INSTALL=1 OME_DISK=/dev/vda OME_SRC=ome'),
    '-display', 'none', '-monitor', 'none', '-serial', (Q "file:$serial"))
  $t0 = Get-Date
  $p = Start-Qemu $argv "$sp\install-stderr-$Tag.txt"
  Log "pid=$($p.Id)"
  $done = $p.WaitForExit($TimeoutSec * 1000)
  if (-not $done) { Log "timeout after $TimeoutSec s; killing"; Stop-Process -Id $p.Id -Force; Start-Sleep 2 }
  Log ("exit={0} elapsed={1:N1}s" -f $p.ExitCode, ((Get-Date) - $t0).TotalSeconds)
  Log ("disk bytes " + (Get-Item $disk).Length)
  Log "---- serial (OME lines)"
  if (Test-Path $serial) { Get-Content $serial | Where-Object { $_ -match 'OME-INSTALL' } | ForEach-Object { Log $_ } }
  Log "---- serial tail"
  if (Test-Path $serial) { Get-Content $serial -Tail 15 | ForEach-Object { Log $_ } }
  Log "---- stderr"
  Get-Content "$sp\install-stderr-$Tag.txt" | ForEach-Object { Log $_ }
  exit 0
}

# boot (direct kernel boot of the spike disk) or grub (an existing GRUB-installed disk, for timing)
$serial = "$sp\$Step-serial-$Tag.log"
if (Test-Path $serial) { Remove-Item $serial -Force }
if ($Step -eq 'grub') {
  if (-not (Test-Path $GrubDisk)) { throw "grub step needs -GrubDisk <qcow2>" }
  $bootDisk = $GrubDisk
  $bootVars = "$sp\efivars-grub.fd"
  $varsSource = Join-Path (Split-Path $GrubDisk) 'efivars.fd'
  if (Test-Path $varsSource) { Copy-Item $varsSource $bootVars -Force } else { Copy-Item $varsTemplate $bootVars -Force }
  $fw = @('-drive', (Q "if=pflash,format=raw,readonly=on,file=$code"), '-drive', (Q "if=pflash,format=raw,file=$bootVars"))
  $bootArgs = @()
} else {
  $bootDisk = $disk
  $bootArgs = @('-kernel', (Q $kernel), '-initrd', (Q $initrd), '-append', (Q 'root=/dev/ram0 SRC=/ome quiet console=ttyS0 console=tty0'))
}
$argv = $common + $fw + @(
  '-m', '8192', '-smp', '4',
  '-drive', (Q "file=$bootDisk,if=virtio,format=qcow2")) + $bootArgs + @(
  '-device', 'virtio-vga-gl,edid=off', '-display', 'sdl,show-cursor=on,gl=on',
  '-device', 'virtio-net-pci,netdev=n0', '-netdev', "user,id=n0,hostfwd=tcp:127.0.0.1:$AdbPort-:5555",
  '-usb', '-device', 'usb-tablet', '-device', 'usb-kbd',
  '-qmp', "tcp:127.0.0.1:$QmpPort,server=on,wait=off",
  '-serial', (Q "file:$serial"))
$t0 = Get-Date
$p = Start-Qemu $argv "$sp\boot-stderr-$Tag.txt"
Log "pid=$($p.Id)"
$dev = "127.0.0.1:$AdbPort"
$booted = $false
while (((Get-Date) - $t0).TotalSeconds -lt $TimeoutSec) {
  if ($p.HasExited) { Log "qemu exited early code=$($p.ExitCode)"; break }
  Start-Sleep -Seconds 3
  $null = & $adb connect $dev 2>&1
  $v = (& $adb -s $dev shell getprop sys.boot_completed 2>$null) -join ''
  if ($v.Trim() -eq '1') { $booted = $true; break }
}
Log ("boot_completed={0} elapsed={1:N1}s" -f $booted, ((Get-Date) - $t0).TotalSeconds)
if ($booted) {
  foreach ($c in @('getprop ro.build.display.id', 'getprop ro.dalvik.vm.native.bridge', 'getprop ro.boot.hardware', 'cat /proc/cmdline',
      'ls /sys/firmware/efi 2>&1 | head -3', 'mount | grep -E " / | /system | /data | /mnt" | head -12', 'df -h /data /system 2>&1 | head -5',
      'ls -la /mnt/ 2>&1 | head', 'cat /proc/mounts | grep -E "loop|vda" | head -12', 'wm size', 'ls /sdcard/ome-spike.txt 2>&1', 'id')) {
    $out = (& $adb -s $dev shell $c 2>&1) -join "`n"
    Log "adb> $c`n$out"
  }
  if ($Tag -eq 'a') { $null = & $adb -s $dev shell "echo spike-a > /sdcard/ome-spike.txt"; Log "wrote /sdcard/ome-spike.txt" }
}
$t1 = Get-Date
if ($booted) {
  # The product's power-off hook: `adb reboot -p`, then QMP, then force.
  $off = (& $adb -s $dev reboot -p 2>&1) -join ' '
  Log "adb reboot -p: $off"
  if (-not $p.WaitForExit(60000)) { Log "no exit 60 s after adb reboot -p" }
}
if (-not $p.HasExited) {
  Log "powerdown via qmp"
  try { Send-Qmp $QmpPort @('{"execute":"system_powerdown"}') } catch { Log "qmp failed: $_" }
  if (-not $p.WaitForExit(30000)) { Log "no exit after 30 s; killing"; Stop-Process -Id $p.Id -Force }
}
Log ("exit={0} shutdown={1:N1}s" -f $p.ExitCode, ((Get-Date) - $t1).TotalSeconds)
$null = & $adb disconnect $dev 2>&1
Log "---- serial (last 25)"
if (Test-Path $serial) { Get-Content $serial -Tail 25 | ForEach-Object { Log $_ } }
Log "---- stderr"
Get-Content "$sp\boot-stderr-$Tag.txt" | ForEach-Object { Log $_ }
