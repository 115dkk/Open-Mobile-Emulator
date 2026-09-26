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
| `system_powerdown` | Delivered, but Android treats the ACPI power button as a key press and does not shut down; `Stop-Guest.ps1` waited 30 s and force-killed. `Stop-Guest.ps1` now sends `adb reboot -p` first and keeps ACPI as the fallback; tested on the virgl guest: "Requested power-off through adb", QEMU gone and "stopped normally" after 1.5 s, no force-kill. |
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
reset. The message comes from `whpx_get_xsave_state`, which QEMU calls when it
reads a vCPU's full register state; what triggered those reads in this run was
not identified. The guest kept running and rendering throughout. The failure is
fatal only when the state is written back on a reset, which the launcher avoids.

### Input through QMP under virgl

`input-send-event` with `abs` axes (0..32767, scaled from the guest's 1280x800)
and `btn` events: a swipe from (640,700) to (640,200) in ten steps opened the
notification shade from the lock screen, and the guest cursor sat at the end
point (`virgl-03-qmp-swipe-opened-shade.png`). A single press/release at
(200,100) put the cursor exactly there over the probe activity
(`virgl-04-qmp-tap-cursor-on-probe.png`). `dumpsys input` lists `QEMU QEMU USB
Keyboard` (`KEYBOARD | ALPHAKEY | EXTERNAL`); the tablet is the pointer seen above.
The arm64 probe APK started in 357 ms under virgl with the same
`ndk_translation 0.2.3` log lines as under SwiftShader.

Audio: the guest reports `Devices: speaker(2)` in `dumpsys audio` and
`ro.hardware.audio.primary=x86`; whether sound reaches the `dsound` backend has
not been checked by ear yet.

### Audio and the SDL mouse path, measured on 2026-09-26

Audio reaches the host. The peak meter of QEMU's audio session on the host's
default render endpoint (WASAPI `IAudioMeterInformation`, read with pycaw)
went from 0.000 to 0.15 to 0.27 while the guest's ringtone picker played
previews, and to 0.06 to 0.10 while the game played its lobby music and a
battle (`run-virgl-session/host-audio-ringtone-preview.txt`,
`host-audio-game.txt`, `run-virgl-battle14/host-audio-battle.txt`). Two
observations for the product:

- Bliss 16.9.7 boots with the media stream at index 5 of 15, which AudioFlinger
  applies as -33 dB to every media track (`G db` column in `dumpsys
  media.audio_flinger`). At that setting the game's music arrived on the host
  as 0.000 to 0.002 peak, in practice silence. `cmd media_session volume
  --stream 3 --set 15` in the guest brought the track to 0 dB and the host peak
  to 0.06 to 0.10. The launcher or the M2 wizard should set the guest media
  volume once after the first boot; the guest's own controls (ALSA `Master`,
  HDA amplifier, game sliders) were already at maximum.
- The endpoint on this host is a virtual cable (`CABLE Input`), so the by-ear
  check depends on the user's routing; the session meter is the objective
  evidence.

Mouse through the SDL window: with the host cursor moved over the QEMU window
(`SetCursorPos`, three points) while `getevent` watched the guest's `QEMU QEMU
USB Tablet`, every report was exactly `floor(client_x * 32767 / 1280)`,
`floor(client_y * 32767 / 800)`: the SDL path scales window-client coordinates
by 32767/size, the same mapping as QMP `input-send-event`
(`sdl-mouse-absolute.txt`). Synthetic `WM_MOUSEMOVE` messages posted to the
window produced nothing, so host-side automation of the SDL window needs real
cursor movement or the QMP path.

Keyboard through the SDL window: no separate measurement. The Google account
was added on 2026-09-26 by typing the credentials into the QEMU window
(`../M0/google-account-added.txt`), and QMP `send-key` drives the same
`usb-kbd` device (step 5 above).

Host GPU coverage: only the NVIDIA GeForce RTX 2080 SUPER (driver 32.0.16.1664)
was available. This host's i5-12600KF has no integrated GPU, so the Intel entry of
the M1 completion criteria has to come from another machine and is listed in
`docs/KNOWN_LIMITATIONS.md`.
