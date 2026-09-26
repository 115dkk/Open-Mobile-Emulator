# M0 runbook (after the WHPX reboot)

Every command runs from the repository root in PowerShell 7. Data lives under
`%LOCALAPPDATA%\OpenMobileEmulator` (`OME_HOME`). Evidence goes to
`docs/evidence/M0/`. Stop and report at the first stop condition in CLAUDE.md 5 M0.

## 0. Host

M0 runs on the distribution QEMU. The launcher prefers a custom build under
`qemu-build\out\bin` when one exists, so pin the choice for the whole session:

```powershell
$env:OME_QEMU_DIR = 'C:\Program Files\qemu'
pwsh -File launcher/Check-Host.ps1
```
Expect: `HypervisorPlatform Enabled`, `Reboot pending No`, QEMU 11.1.0 found under
`C:\Program Files\qemu` (source `env`), firmware found, adb found, "Ready for M0: yes".
The feature reads Enabled as soon as `dism.exe` finishes, so the "Reboot pending"
row is what tells you whether the enable has actually taken effect. If it still
says Yes, reboot first. Enable history is in `docs/evidence/M0/whpx-enable.md`.

## 1. Artifact

```powershell
pwsh -File launcher/Get-Artifacts.ps1 -VerifyOnly
```
The ISO was downloaded and hash-verified on 2026-09-25; this only re-verifies.

## 2. Guest disk and Bliss install (user at the keyboard)

```powershell
pwsh -File launcher/New-GuestDisk.ps1 -Name default -SizeGB 32 -BootInstaller
```
Follow `docs/GUEST_INSTALL.md` inside the QEMU window: Install, GPT, EFI partition
plus ext4 root, GRUB EFI, then reboot into the disk:

```powershell
pwsh -File launcher/Stop-Guest.ps1 -Name default
pwsh -File launcher/Start-Guest.ps1 -Name default -Gpu virgl
```
At the installed GRUB menu pick `VM Options ->` and then, for the M0 software
rendering harness, `... Vbox/VMWare - No HW Acceleration` (`nomodeset HWACCEL=0`);
for the M1 GPU path pick `... QEMU/KVM - Virgl - SW-FFMPEG`. The plain default
entry hangs. GRUB remembers the last choice (`savedefault`), so later boots need no
keystroke until a different entry is chosen. `-Gpu virgl` is used for both because
`-Gpu std` shows a garbled host window with the nomodeset entry (the guest still
renders in software with that entry; see `docs/evidence/M0/guest-install.md`).
First boot takes under a minute to adb; the setup wizard was completed on
2026-09-25 without a Google account, and adb over the loopback forward works
without any guest setting (see `docs/GUEST_INSTALL.md`).

Two behaviours to expect, both recorded in `docs/evidence/M0/guest-install.md`:

- The launcher's CPU model is `Skylake-Client-v4`. Do not pass `-Cpu max`: under
  WHPX it is the TCG model and Bliss hangs after the init banner.
- A reboot from inside the guest (installer `Reboot`, Android restart, `adb reboot`)
  ends the QEMU process because the launcher passes `-action reboot=shutdown`;
  a real reset wedges QEMU 11.1.0 under WHPX. Run `Start-Guest.ps1` again.
  Do not use QMP `system_reset` for the same reason.

## 3. Bridge and CPU evidence (M0 step 4)

```powershell
pwsh -File launcher/Invoke-GuestTest.ps1 -Name default -SkipInstall
```
Check `bridge-props.txt`: `ro.dalvik.vm.native.bridge=libndk_translation.so` and
`arm64-v8a` in `ro.product.cpu.abilist`; `cpuinfo.txt` must show `sse4_2` and
`popcnt`. If the bridge is off, find the Bliss native-bridge toggle
(`docs/GUEST_INSTALL.md`), enable it, reboot the guest, rerun.

## 4. APK from the user's own phone (M0 step 5)

Connect the phone with USB debugging on, accept the RSA prompt, then:

```powershell
pwsh -File launcher/Get-GameApk.ps1 -Package com.epidgames.trickcalrevive
```
The script compares the signer SHA-1 with `compat/com.epidgames.trickcalrevive.json`
and refuses on mismatch. APKs land in `OME_HOME\apks\com.epidgames.trickcalrevive\`.

## 5. Install, run, measure (M0 steps 6 and 7)

```powershell
$apks = @(Get-ChildItem "$env:LOCALAPPDATA\OpenMobileEmulator\apks\com.epidgames.trickcalrevive\*.apk" | ForEach-Object FullName)
& ./launcher/Invoke-GuestTest.ps1 -Name default -Apk $apks -SettleSec 120
```
Call the script in-process (`&`), not through `pwsh -File`: `-File` flattens the
array into separate positional arguments and the second path lands in
`-BootTimeoutSec` (seen 2026-09-26).
Then play the scenario by hand (first run and resource download, guest account,
tutorial battle, one voiced story episode, 10-minute session, quit and relaunch),
taking a screenshot at each step:

```powershell
Import-Module ./launcher/OME.Common.psm1
Save-OmeQmpScreenshot -OutFile docs/evidence/M0/step-<n>-<name>.png
```
Re-run `Invoke-GuestTest.ps1 -SkipInstall -SkipLaunch` during battle and during
story to get `top`, `meminfo` and host samples; copy the numbers into
`docs/evidence/M0/metrics.md`. Always pass `-SkipLaunch` while the game runs: the
`monkey` relaunch made Android kill the game on 2026-09-26 (foreground-service
timeout, `findings-20260926.md`). `dumpsys gfxinfo` counts only the Android view
frames of a Unity game (three frames for a whole battle), so the frame rate comes
from SurfaceFlinger instead:

```powershell
$layer = (adb -s 127.0.0.1:5555 shell dumpsys SurfaceFlinger --list | Select-String 'SurfaceView\[com.epidgames.trickcalrevive.*\(BLAST\)').Line.Trim()
adb -s 127.0.0.1:5555 shell dumpsys SurfaceFlinger --latency-clear
Start-Sleep 10
adb -s 127.0.0.1:5555 shell dumpsys SurfaceFlinger --latency "`"$layer`""   # column 2 = present time (ns) per frame
```
Under a virgl scanout `Save-OmeQmpScreenshot` answers `no surface`; take
screenshots with `adb -s 127.0.0.1:5555 exec-out screencap -p > file.png`.
Watch `logcat-bridge-errors.txt` for `dlopen failed`, `SIGILL`, `ndk_translation`
(stop condition).

To repeat the scenario on the other GRUB entry without touching the menu,
rewrite the saved default from the running guest (needs `adb root`):

```powershell
adb -s 127.0.0.1:5555 shell mount -t ext4 -o rw,noatime /dev/block/vda2 /data/local/tmp/root
# push a 1024-byte grubenv ("# GRUB Environment Block\ndefault=<entry>\n" padded with '#')
# over /data/local/tmp/root/boot/grub/grubenv, sync, then Stop-Guest / Start-Guest
```
Entry strings are listed in `docs/evidence/M0/findings-20260926.md`.

## 6. Google sign-in path (M0 step 8)

Follow `docs/GOOGLE_ACCOUNT.md`; record the result there and in the evidence folder.

## 7. Wrap up

Fill `docs/evidence/M0/metrics.md`, update `environment.md` if anything changed,
run `pwsh -File ci/Invoke-AllChecks.ps1`, commit per sub-step.
