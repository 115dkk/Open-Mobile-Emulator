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
pwsh -File launcher/Start-Guest.ps1 -Name default -Gpu std
```
First boot takes minutes. Finish the setup wizard without a Google account.
Turn on adb over network in the guest (see `docs/GUEST_INSTALL.md`).

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
$apks = Get-ChildItem "$env:LOCALAPPDATA\OpenMobileEmulator\apks\com.epidgames.trickcalrevive\*.apk" | ForEach-Object FullName
pwsh -File launcher/Invoke-GuestTest.ps1 -Name default -Apk $apks -SettleSec 120
```
Then play the scenario by hand (first run and resource download, guest account,
tutorial battle, one voiced story episode, 10-minute session, quit and relaunch),
taking a screenshot at each step:

```powershell
Import-Module ./launcher/OME.Common.psm1
Save-OmeQmpScreenshot -OutFile docs/evidence/M0/step-<n>-<name>.png
```
Re-run `Invoke-GuestTest.ps1 -SkipInstall` during battle and during story to get
`top`, `gfxinfo`, `meminfo` samples; copy the numbers into `docs/evidence/M0/metrics.md`.
Watch `logcat-bridge-errors.txt` for `dlopen failed`, `SIGILL`, `ndk_translation`
(stop condition).

## 6. Google sign-in path (M0 step 8)

Follow `docs/GOOGLE_ACCOUNT.md`; record the result there and in the evidence folder.

## 7. Wrap up

Fill `docs/evidence/M0/metrics.md`, update `environment.md` if anything changed,
run `pwsh -File ci/Invoke-AllChecks.ps1`, commit per sub-step.
