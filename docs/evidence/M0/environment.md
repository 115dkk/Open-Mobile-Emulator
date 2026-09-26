# M0 environment

Recorded 2026-09-25. Update when any row changes.

## Host

| Item | Value |
|---|---|
| CPU | Intel Core i5-12600KF (10C/16T) |
| RAM | 64 GB |
| GPU | NVIDIA GeForce RTX 2080 SUPER, driver 32.0.16.1664 |
| OS | Windows 11 Education 10.0.22621 |
| Hypervisor | Hyper-V enabled and running. `HypervisorPlatform` (WHPX) was disabled at the start; enablement with user consent is recorded in `whpx-enable.md` (dism.exe, exit 3010); the host rebooted the same evening and WHPX has been in use since |
| Tools | PowerShell 7.6.6, adb 37.0.1, NDK 27.2 / 28.2, build-tools 35 / 36, JDK 17 |

## Harness

Distribution QEMU for Windows (winget `SoftwareFreedomConservancy.QEMU` 11.1.0),
`-accel whpx,kernel-irqchip=off`, `-cpu Skylake-Client-v4`, q35, 6144 MiB, 4 vCPU,
standard VGA (`-device VGA`) with the guest's software rendering, `-action reboot=shutdown`.
See `launcher/Start-Guest.ps1 -Gpu std`; the exact command line of every start is in
`%LOCALAPPDATA%\OpenMobileEmulator\logs\qemu-default-<timestamp>.log`.
The launcher would prefer a custom build under `qemu-build\out\bin` once it exists, so
M0 sessions set `OME_QEMU_DIR` to `C:\Program Files\qemu` explicitly (runbook step 0).
Chosen over VirtualBox because Hyper-V is active on this host, which forces
VirtualBox onto the same WHPX path, and because the QEMU path carries over to M1.

Guest boot entry: installed GRUB `VM Options -> BlissOS-16.9.7 2024-10-11 - Vbox/VMWare -
No HW Acceleration` (`quiet nomodeset HWACCEL=0`). The plain default entry hangs on
standard VGA. Guest disk: `%LOCALAPPDATA%\OpenMobileEmulator\vm\default\disk.qcow2`,
32 GiB, GPT, vda1 512 MiB ESP (fat32), vda2 ext4. Details: `guest-install.md`.

Two harnesses were used on 2026-09-26, both on the same disk and the same QEMU
command line (`Start-Guest.ps1 -Name default -Gpu virgl`, `virtio-vga-gl`,
`sdl,gl=on`); only the GRUB entry differs, and the rows in `metrics.md` say which:

| Harness | GRUB entry (`/proc/cmdline` tail) | Guest GL | Display |
|---|---|---|---|
| M0 software rendering | `quiet nomodeset HWACCEL=0` | `Google SwiftShader 4.1.0.7`, OpenGL ES 3.0 | 1024x768, density 160 |
| M1 GPU path | `quiet HWC=drm_minigbm GRALLOC=minigbm_arcvm` | `Mesa 24.0.8 virgl (NVIDIA GeForce RTX 2080 SUPER)`, OpenGL ES 3.2 | 1280x800, density 160 |

The entry is switched without a keystroke by rewriting `/boot/grub/grubenv` on
vda2 from the running guest (`findings-20260926.md`, `nomodeset-boot-via-grubenv.txt`).

## Guest image

| Item | Value |
|---|---|
| File | `Bliss-v16.9.7-x86_64-OFFICIAL-gapps-20241011.iso` |
| Source | SourceForge `blissos-x86`, `Official/BlissOS16/Gapps/Generic/` |
| Size | 2,429,550,592 bytes |
| SHA-256 (published `.iso.sha256`) | `17137711fb42236640ac6fe4421fcb3bd15710065fcf75c20807c018f42e3751` |
| SHA-256 (computed locally, 2026-09-25, `sha256sum`) | `17137711fb42236640ac6fe4421fcb3bd15710065fcf75c20807c018f42e3751` (match) |

## Evidence files in this folder

- `environment.md` (this file)
- `whpx-enable.md` (R9 consent record and command output)
- `guest-install.md` (installation, boot attempts, WHPX findings, setup wizard, reboot test) with `install-*.png`, `installed-grub-*.png`, `first-boot-*.png`, `boot-attempt-1-*`
- `bridge-props.txt`, `cpuinfo.txt` (M0 step 4, done; copies from `run-20260925-215100/`)
- `arm64-probe-run.md`, `arm64-probe-screen.png` (self-built arm64-only APK under libndk_translation)
- `google-account.md` (M0 step 8, done 2026-09-26: GSF ID read, registered by the user, account added, Play Games sign-in verified; `google-account-added.txt`)
- `metrics.md` (M0 step 7, done 2026-09-26 for both harnesses; per-run samples in `run-*/`)
- `findings-20260926.md` (the game run in order: APK, boots, crashes, network, scenario, step 8)
- `step-01` to `step-48` screenshots (M0 step 6; game UI only, kept out of README and releases per R6)
- `run-virgl-*`, `run-nomodeset-*` (samples), `crash-ndk-translation-1/` (translator abort), `swcodec-gralloc-abort-virgl.txt` (codec abort), `game-process-maps.txt`, `audio-flinger-*.txt`, `nomodeset-boot-via-grubenv.txt`, `logcat-virgl-firstrun-full.txt`
