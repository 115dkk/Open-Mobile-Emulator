# M0: Bliss OS 16.9.7 guest installation record

Recorded 2026-09-25 after the WHPX reboot. Every step below was driven over QMP
(`send-key` for keystrokes, `screendump` for the screenshots in this folder);
nothing was typed into the SDL window. Host, harness and ISO are in
`environment.md`.

## Launch

```
launcher/New-GuestDisk.ps1 -Name default -SizeGB 32 -BootInstaller
```
creates `%LOCALAPPDATA%\OpenMobileEmulator\vm\default\{disk.qcow2,efivars.fd,vm.json}`
and runs `Start-Guest.ps1 -Name default -Cdrom`. Exact QEMU command lines are in
`%LOCALAPPDATA%\OpenMobileEmulator\logs\qemu-default-<timestamp>.log`.

## Boot attempt 1: `-cpu max` hangs after the init banner (fixed by a named model)

| Attempt | `-cpu` | GRUB entry | Result |
|---|---|---|---|
| 1 | `max` (launcher default at the time) | default entry (timed out; `Live PC-Mode (Default)`) | Init banner printed, then no progress for 10 min. adb never came up. Cursor kept blinking, so the kernel was alive. `boot-attempt-1-default-entry-stuck-banner.png` |
| 2 | `max` | `Installation` (selected by hand) | Same banner, same hang for 90 s. |
| 3 | `Skylake-Client-v4` | `Installation` | Installer dialog after 16 s. Everything below ran from this boot. |

Why `max` is wrong under WHPX, from the QEMU 11.1 source (`target/i386/cpu.c`,
`target/i386/whpx/whpx-all.c`):

- WHPX intercepts the CPUID leaves in `cpuidExitList` (0, 1, 6, 7, 0xb, 0xd, ...,
  0x80000001, ...) and answers them from QEMU's CPU model with `cpu_x86_cpuid`,
  so the guest sees the `-cpu` model, not the host.
- `accel_uses_host_cpuid()` is true only for KVM and HVF. Under WHPX the `max`
  model therefore falls back to the TCG definition: vendor `AuthenticAMD`,
  model-id `QEMU TCG CPU version ...`, and TCG's feature set, on an Intel host.
- The same run logged `failed to get xsave state: No error` once per vCPU when QMP
  `system_reset` synchronised registers, and every reset or `cont` afterwards
  ended in `WHPX: Unexpected VP exit code 4` (`WHvRunVpExitReasonUnrecoverableException`)
  with the VM left in `paused`. `boot-attempt-1-paused-info-registers.txt` holds
  `info registers` from that state (RIP `0xfffcc413`, 32-bit reset-vector area).
  A cold restart of QEMU was required.

With `Skylake-Client-v4` QEMU only warns that the host lacks `tsc-deadline` and
`arat`, and the guest boots. Whether guest-initiated reboots (`reboot` from the
installer or Android) survive under WHPX with this model is checked below.

**Launcher change:** `Start-Guest.ps1` default `-Cpu` moves from `max` to
`Skylake-Client-v4`, and the comment marked UNVERIFIED on `kernel-irqchip=off`
is resolved (boots).

## GRUB menu of the ISO (screens `install-01` to `install-04`)

Main menu: `Live`, `Live (Default) w/ FFMPEG`, `Live PC-Mode (Default)`,
`Live PC-Mode (Default) w/ FFMPEG`, `Installation`, `VM Options ->`,
`Debugging ->`, `Advanced options ->`. The timeout selected a Live entry, so the
launcher has to interrupt it (a cursor key within about 5 s of the menu appearing).

`VM Options ->` contains four live entries. Their kernel parameters, read with `e`:

| Entry | Parameters appended after `quiet` |
|---|---|
| `Live - QEMU/KVM - Virgl - SW-FFMPEG` | `HWC=drm_minigbm GRALLOC=minigbm_arcvm` |
| `Live - Vbox/VMWare - No HW Acceleration` | `nomodeset HWACCEL=0` |
| `Live - Debug QEMU/KVM - Virgl - SW-FFMPEG` | (debug variant of the first) |
| `Live - Debug Vbox/VMWare - No HW Acceleration` | (debug variant of the second) |

The linux line is `linux $kd/kernel root=/dev/ram0 $src $@` with `initrd $kd/initrd.img`.
These two parameter sets are the ISO authors' own choices for virgl and for
software rendering; `guest/kernel-cmdline.md` now cites them as verified.

## Installer flow as observed (Bliss 16.9.7, UEFI)

| # | Screen | What was chosen | Screenshot |
|---|---|---|---|
| 1 | `Choose Partition`: "UEFI System detected! Please select a partition as EFI System Partition (ESP)". Empty disk shows only `Create/Modify partitions`, `Restart the installer`, `Open command-line shell`, `Skip (Not Recommended)`. | Plain Enter on the first item gave `Error: This is not an EFI System Partition` with a 3 s countdown and returned to the menu. Pressing the `c` hotkey and then Enter opened partitioning. | `install-05`, `install-06` |
| 2 | `Confirm`: redirect to cfdisk, or switch to cgdisk? | `No, continue to cfdisk` (default) | `install-07` |
| 3 | cfdisk `Select label type` | `gpt` | `install-08` |
| 4 | cfdisk `New`: the size field is prefilled with the whole free size (`32G`); typing appends. | Cleared with Backspace, typed `512M`. | |
| 5 | cfdisk `Type` list; `Linux filesystem` is preselected, `EFI System` is the first entry. | `EFI System` for partition 1 | `install-09` |
| 6 | cfdisk `New` on the remaining free space | default size (31.5G), type left as `Linux filesystem` | |
| 7 | cfdisk `Write` asks to type `yes`; `Quit` | `yes`; quit | `install-10` |
| 8 | Installer restarts itself ("Restarting the installer") and lists `vda1 unknown 0.50GB`, `vda2 unknown 31.50GB`. | `vda1` as ESP | `install-11` |
| 9 | `Choose filesystem` for vda1: `Do not re-format`, `fat32 (CAREFUL)` | `fat32` | `install-12` |
| 10 | `Question`: label for the formatted drive, default `ESP` | Enter (kept) | |
| 11 | `Confirm` format vda1 as fat32, default button `No` | `Yes` | `install-13` |
| 12 | `Choose Partition`: "Please select a partition to install BlissOS-16.9.7", now with `Setup userdata partition` as an extra action | `vda2` | `install-14` |
| 13 | `Choose filesystem` for vda2: `Do not re-format`, `ext4`, `ntfs`, `fat32`, `exfat`, `f2fs` | `ext4` | `install-15` |
| 14 | `Question`: label, default `BlissOS` | Enter (kept) | |
| 15 | `Confirm` format vda2 as ext4, default `No` | `Yes` | `install-16` |
| 16 | `Confirm`: "Would you like the installer to prepare for OTA update? This operation will double the system size" (default `Yes`) | `No` (single-install test disk, GUEST_INSTALL step 8) | `install-17` |
| 17 | `Choose EFI Boot`: `Grub2 EFI Bootloader`, `rEFInd Boot Manager`, or `Skip` | `Grub2 EFI Bootloader` | `install-18` |
| 18 | `Installing BlissOS-16.9.7 to vda2`, "Expect to write 2307771 KB" | (progress) | `install-19` |

No "make /system writable" prompt appeared before the copy started. The copy
took about one minute and ended in `Congratulations! BlissOS-16.9.7 is installed
successfully.` with `Run BlissOS-16.9.7` and `Reboot` (`install-20`).

## Guest-initiated reboot fails under WHPX (CPU model independent)

`Reboot` was chosen on purpose to test a guest reset with the named CPU model.
Within 3 s the VM went `running` -> `paused`, QMP stopped accepting connections,
and QEMU's stderr (`install-boot-qemu-stderr-skylake.log`) shows the same
sequence as with `-cpu max`:

```
failed to get xsave state: Input/output error
failed to get xsave state: No error   (x3)
WHPX: Unexpected VP exit code 4
```

The QEMU process stayed alive and had to be killed. Conclusion: in QEMU 11.1.0
on this host, any reset of a WHPX vCPU (guest reboot or QMP `system_reset`)
fails in `whpx_get_xsave_state` and the vCPU comes back in an unrecoverable
state. The launcher now passes `-action reboot=shutdown` so that a guest reboot
ends QEMU cleanly instead, and the M2 supervisor will restart it. Fixing the
xsave path is a candidate patch for the custom build (`qemu-build/patches/`).

## First boot from the installed disk

Started with `Start-Guest.ps1 -Name default` (no `-Cdrom`, `Skylake-Client-v4`).
EDK2 found the installed GRUB (2.12-1) on `vda1` without any firmware-menu work.
Installed menu: `BlissOS-16.9.7 2024-10-11`, `... (Default) w/ FFMPEG`,
`... PC-Mode (Default)`, `... PC-Mode (Default) w/ FFMPEG`, `VM Options ->`,
`Debugging ->`, `Advanced options ->`, `BlissOS at hd0,gpt1 ->`. The first entry's
parameters are `quiet` only, with the kernel line
`linux $kd/kernel stack_depot_disable=on cgroup_disable=pressure root=/dev/ram0 $src $@`
and `src=/android-2024-10-11`.

| Attempt | Entry | Result |
|---|---|---|
| 4 | first entry (timed out to default) | Console shows the init banner's last line and a blinking cursor; after 2 min QEMU used 16% of one core, the qcow2 grew by 0.4 MB in 10 s, and `info usernet` showed every forwarded adb SYN stuck in `SYN_SENT` (the guest never answered on 10.0.2.15, so no DHCP had happened). `info registers` put CPU0 in the kernel idle loop. Treated as hung in early userspace. |
| 5 | `VM Options -> BlissOS ... - Vbox/VMWare - No HW Acceleration` (`nomodeset HWACCEL=0`, the ISO's own software-rendering set) | Android up. `adb connect 127.0.0.1:5555` reported `device` 42 s after Enter, `sys.boot_completed=1` shortly after; the Google setup wizard (`WelcomeActivity`, "Hi there") was on screen (`first-boot-01-setup-wizard-welcome.png`, taken with `adb screencap`). |

So with standard VGA the default entry (modesetting on `bochs-drm`) hangs before
networking and `nomodeset HWACCEL=0` boots. `nomodeset` is the M0 harness setting
from now on; the entry can be made the GRUB default later (`guest/kernel-cmdline.md`).

**Host display in this mode is garbled.** The QEMU window and QMP `screendump`
show a 1024x768 frame whose top 560 rows are green/blue stripes and whose bottom
still holds the GRUB background (`first-boot-00-host-display-nomodeset-garbled.png`).
The guest reports `wm size` 1280x800 at density 160, so Android is drawing a
1280x800 frame into the 1024x768 GOP framebuffer left by GRUB, and the pitch does
not match. `adb screencap` is correct, so M0 evidence uses adb for screenshots and
`adb shell input` for taps. Aligning the GOP mode with the guest (GRUB
`gfxpayload`/`video=` or a 1280x800 firmware mode) is an M0 follow-up; M1 moves to
`virtio-vga-gl` where the guest drives the mode itself.

adb needed no guest-side setup: `ro.adb.secure=0`, the daemon listens on TCP 5555
out of the box (`service.adb.tcp.port` is empty, so the port comes from the image's
init), and the launcher's `hostfwd` reaches it. This resolves the UNVERIFIED items
in `docs/GUEST_INSTALL.md` "Network ADB".

## M0 step 4 evidence (bridge and CPU)

`launcher/Invoke-GuestTest.ps1 -Name default -SkipInstall -SettleSec 5` wrote
`run-20260925-215100/`; `bridge-props.txt` and `cpuinfo.txt` in this folder are
copies of its samples.

| Check | Value |
|---|---|
| `ro.dalvik.vm.native.bridge` | `libndk_translation.so` |
| `ro.product.cpu.abilist` | `x86_64,arm64-v8a,x86,armeabi-v7a,armeabi` |
| `ro.product.cpu.abilist64` | `x86_64,arm64-v8a` |
| `ro.dalvik.vm.isa.arm64` / `.arm` | `x86_64` / `x86` |
| `persist.sys.nativebridge` | `1` (no toggle had to be touched) |
| `ro.enable.native.bridge.exec` | empty (binfmt_misc for standalone ARM executables is off) |
| `ro.ndk_translation.version` | `0.2.3` |
| Build | `Android-x86/bliss_x86_64/x86_64:13/TQ3A.230901.001.C1/152:userdebug/test-keys`, SDK 33 |
| `/proc/cpuinfo` model name | `Intel Core Processor (Skylake, IBRS, no TSX)`, 4 cores |
| `/proc/cpuinfo` flags (x86-64-v2 subset) | `ssse3 sse4_1 sse4_2 popcnt cx16 lahf_lm` present, plus `avx avx2 fma bmi1 bmi2 aes xsave` |
| Display | `wm size` 1280x800, density 160 |
| QMP `input-send-event` latency (`Measure-OmeQmpLatency`, 50 iterations, WHPX) | min 0.79 ms, avg 1.02 ms, max 2.60 ms |

## Translator smoke with the self-built arm64-only APK

`tests/fixtures/build/arm64-probe.apk` (arm64-v8a JNI library only) installed with
`adb install` (`primaryCpuAbi=arm64-v8a`) and launched with `am start`. Result
(`arm64-probe-run.md`, `arm64-probe-screen.png`):

```
I ndk_translation: Initialized NDK translation (aarch64), version 0.2.3
I OMEProbe: native arm64-v8a lib loaded;SUPPORTED_ABIS=x86_64,arm64-v8a,x86,armeabi-v7a,armeabi;os.arch=aarch64;CPU_ABI=arm64-v8a
```

The activity displayed in 451 ms (cold start). This is the same mechanism a Unity
arm64 game uses (JNI library loaded through the native bridge), so the harness is
ready for the game APK. The static `hello_arm64` executable could not be run through
the image's binfmt_misc runner (segfault; details in `arm64-probe-run.md`).

## Setup wizard, driven with `adb shell input tap` (screens `first-boot-01` to `-07`)

| Screen (`mCurrentFocus`) | Choice |
|---|---|
| `setupwizard/.user.WelcomeActivity` "Hi there", English (United States) | START |
| `setupwizard/.network.CompatCheckinAndEarlyUpdate` "Getting your tablet ready" | waited (about 20 s; the guest has slirp networking) |
| `com.google.android.apps.restore/...UsbD2dMigrateFlowActivity` "Copy apps & data" | Don't copy |
| `gms/.auth.uiflows.minutemaid.MinuteMaidActivity` "Sign in" (the Google page loaded, so the uncertified guest is offered sign-in) | Skip, then Skip in "Skip account setup?" |
| `gms/.setupservices.GoogleServicesActivity` "Google services" | Free up space left on; Use location, Allow scanning, Send usage and diagnostic data turned off; MORE, MORE, ACCEPT |
| `settings/.password.SetupChooseLockGeneric` "Protect your tablet" | Not now, SKIP ANYWAY |
| `android/...ResolverActivity` "Select a Home app": Taskbar for Bliss OS, Launcher3, Smart Dock | Launcher3, Always |

Afterwards `settings get global device_provisioned` and
`settings get secure user_setup_complete` both read `1` and Launcher3
(`QuickstepLauncher`) is in the foreground. No Wi-Fi screen appeared (the guest sees
a wired `eth0`), and no Bliss-specific screen appeared.
