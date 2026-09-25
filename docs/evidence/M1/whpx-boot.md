# M1: WHPX boot path on the distribution QEMU (2026-09-25 evening)

Host and guest: `docs/evidence/M0/environment.md`. QEMU: distribution 11.1.0
(`C:\Program Files\qemu`), pinned through `OME_QEMU_DIR` because the custom build
still has the undiagnosed start-up crash (`custom-qemu-build.md`). The full
installation and first-boot record is `docs/evidence/M0/guest-install.md`; this
file collects the M1 step 4 and step 5 items.

## Step 4: launcher arguments, verified one by one

| Spec item | Result |
|---|---|
| `-accel whpx,kernel-irqchip=off` | Boots Bliss 16.9.7 to Android. Never tried without `kernel-irqchip=off`. |
| `-machine q35 -m 6144 -smp 4` | As specified; guest sees 4 cores, 6,056,336 kB. |
| CPU model | `-cpu max` hangs after the init banner (under WHPX it is the TCG model with AMD vendor; QEMU answers CPUID from its own model). `Skylake-Client-v4` boots; guest `/proc/cpuinfo` shows `sse4_2 popcnt` plus `avx2 fma bmi2 aes xsave` (x86-64-v2 and more). QEMU warns that the host lacks `tsc-deadline` and `arat` for this model. No other named model was needed. |
| `-device virtio-vga-gl -display sdl,gl=on,show-cursor=on` | QMP `query-display-options` returns `gl: on`. With the guest's `nomodeset HWACCEL=0` entry the host window is correct at 1024x768 (SwiftShader inside the guest). With the ISO's virgl entry (`HWC=drm_minigbm GRALLOC=minigbm_arcvm`): see the virgl section below. |
| `-device virtio-net-pci -netdev user,...,hostfwd=tcp:127.0.0.1:5555-:5555` | adb over the forward works with no guest setup; slirp DHCP gives the guest 10.0.2.15; Google check-in succeeded through it. |
| `-usb -device usb-tablet -device usb-kbd` | GRUB, the text installer and cfdisk were driven with QMP `send-key`; taps went through `adb shell input` so far (QMP `input-send-event` measured only for latency). |
| `-audiodev dsound,id=snd0 -device intel-hda -device hda-duplex` | Accepted by QEMU; guest audio output not yet exercised. |
| `-drive file=guest.qcow2,if=virtio` + EDK2 pflash pair | Installed GRUB found on the ESP at first try; data persists across a cold restart. |
| Guest boot arguments | Standard VGA: only `nomodeset HWACCEL=0` boots. Details and the ISO's own VM entries in `guest/kernel-cmdline.md`. |
| Added by the launcher | `-action reboot=shutdown`, because a vCPU reset under WHPX ends in `failed to get xsave state` and `WHPX: Unexpected VP exit code 4` (guest reboot and QMP `system_reset` alike). A guest reboot therefore ends QEMU in about 0.5 s and the launcher starts it again. |

## Step 5: QMP

| Command | Result |
|---|---|
| `query-status` | Works throughout; during the first seconds of a GL display start the main loop is busy and answers late, so callers need timeouts of about 5 s there. |
| `screendump` (PPM, converted by the launcher) | Correct with `virtio-vga-gl` once Android is up. Garbled with `-device VGA` + `nomodeset` (guest 1280x800 into a 1024x768 GOP frame). Calling it repeatedly while GRUB draws on the GL display twice left QMP unresponsive (the QEMU process survived and the guest kept running); the launcher should not screendump during the firmware and GRUB phase in GL mode. |
| `system_powerdown` | Delivered, but Android treats the ACPI power button as a key press and does not shut down; `Stop-Guest.ps1` waited 30 s and force-killed. `Stop-Guest.ps1` now sends `adb reboot -p` first and keeps ACPI as the fallback. |
| `system_reset` | Do not use (WHPX reset failure above). |
| `input-send-event` latency (`Measure-OmeQmpLatency`, 50 iterations, guest at the setup wizard) | min 0.79 ms, average 1.02 ms, max 2.60 ms. Fine for M2 key mapping. |
| `send-key` | Drove GRUB (`down`, `home`, `e`, `esc`, `ret`, `f10`-free), the dialog installer and cfdisk including shifted characters (`shift`+`m`, `shift`+`w`). |

## virgl (M1 step 4, GPU path): works on this host

Boot of the installed GRUB entry `VM Options -> BlissOS-16.9.7 2024-10-11 -
QEMU/KVM - Virgl - SW-FFMPEG` (`quiet HWC=drm_minigbm GRALLOC=minigbm_arcvm`) with
`Start-Guest.ps1 -Name default -Gpu virgl` (`-device virtio-vga-gl -display
sdl,show-cursor=on,gl=on`, WHPX, `Skylake-Client-v4`). Command line:
`%LOCALAPPDATA%\OpenMobileEmulator\logs\qemu-default-20260925-221632173.log`.

| Item | Result |
|---|---|
| adb `device` / `sys.boot_completed=1` | 16 s / 19 s after the entry was chosen (SwiftShader boot: 26 s / 29 s) |
| SurfaceFlinger `GLES:` | `Mesa, virgl (NVIDIA GeForce RTX 2080 SUPER/PCIe/SSE2), OpenGL ES 3.2 Mesa 24.0.8` |
| `ro.hardware.egl` / `.hwcomposer` / `.gralloc` / `.vulkan` | `mesa` / `drm_minigbm` / `minigbm_arcvm` / `virtio` |
| `ro.opengles.version` | `196608` (ES 3.0 reported to apps) |
| Display | `wm size` 1280x800, density 160; HWC display `port=0 pnpId=RHT displayName="QEMU Monitor"`; composition type `DEVICE` |
| minigbm log | `Supported CAPSET IDs: 6` (virgl), two unsupported format combinations skipped, `VIRTGPU_PARAM_CREATE_GUEST_HANDLE`/`RESOURCE_SYNC`/`GUEST_VRAM` not enabled (informational) |
| logcat `*:E` for virgl, hwcomposer, drm, gralloc, SurfaceFlinger | none in the first minutes |
| QEMU stderr | CPU-model warnings and `failed to get xsave state` lines (see below), nothing from virglrenderer |
| QMP `screendump` | `QMP error: no surface`: with a GL scanout the console has no CPU-side surface in this build. Use `adb screencap` (correct, `m-*.png`) or a future host-side capture. |
| QMP `input-send-event` latency | min 1.26 ms, average 1.93 ms, max 3.44 ms (50 iterations) |
| GRUB navigation | Done blind (`send-key` only, no `screendump`) because screendumps during the firmware and GRUB phase on the GL display wedged QMP twice. The entry was reached with `home`, four `down`, Enter, `home`, Enter after a 13 s key-holding phase. |

`failed to get xsave state: No error` appeared 21 times in this run without any
reset. It is emitted whenever QEMU reads the full vCPU state under WHPX
(`whpx_get_registers(cpu, WHPX_LEVEL_FULL_STATE)`), so it is a warning about
QEMU's copy of the FPU/AVX state, not a guest fault; the guest kept running and
rendering. It becomes fatal only on a reset (write-back), which the launcher avoids.

Host GPU coverage: only the NVIDIA GeForce RTX 2080 SUPER (driver 32.0.16.1664)
was available. This host's i5-12600KF has no integrated GPU, so the Intel entry of
the M1 completion criteria has to come from another machine and is listed in
`docs/KNOWN_LIMITATIONS.md`.
