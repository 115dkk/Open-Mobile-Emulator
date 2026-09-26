# M1 completion checklist (2026-09-26)

Each line of the M1 completion criteria in `CLAUDE.md` section 5, with its
status on 2026-09-26 and the file that carries the evidence. The second table
lists what was still unverified when this file was written and how each item is
verified; results are appended to the third section as they come in.

## Completion criteria

| Criterion | Status | Evidence |
|---|---|---|
| Custom QEMU builds from a reproducible script | done 2026-09-25 (`v11.1.1`, one patch, pinned MSYS2 packages) | `custom-qemu-build.md` |
| Source offer (R4) is produced with the build | done (`qemu-source-offer-v11.1.1-c3d48b7d1e89.tar.gz`, 84,031 members, SHA-256 recorded) | `custom-qemu-build.md`; the CI job `qemu-source-offer` is M3 work |
| Custom QEMU runs | fixed 2026-09-26 evening: `ui/sdl2.c:127` calls `eglGetCurrentDisplay` through libepoxy and the bundle had no `libEGL.dll`, so the SDL GL display crashed at start-up. The build now pins `mingw-w64-ucrt-x86_64-angleproject` and copies `libEGL.dll`, `libGLESv2.dll` and their dependencies; the rebuilt binary stayed alive 3 of 3 times with `-display sdl,gl=on` from a PATH of `C:\Windows\System32;C:\Windows` only, then booted the Bliss guest (34 s, virgl, 60 Hz) and started the game, whose loading screens presented at 57.8 to 58.2 fps with audio reaching the host. The M0 and M1 scenario evidence was collected on the distribution QEMU 11.1.0 | `custom-qemu-build.md`, `custom-qemu-cdb/summary.txt`, `run-custom-qemu-boot/` |
| Bliss boots with WHPX and virgl GL | done (16 s to adb, Mesa virgl on the RTX 2080 SUPER) on the distribution QEMU; on 2026-09-26 evening also on the custom build with ANGLE bundled (34 s to `sys.boot_completed`, virgl, 60 Hz) | `whpx-boot.md`, `run-custom-qemu-boot/` |
| The game passes the M0 scenario under virgl | partly: install, download, battle, 10-minute session and Google sign-in were done under virgl; the voiced story and the quit-and-relaunch step were done under the software entry only | `../M0/metrics.md`, `../M0/findings-20260926.md` |
| Battle frames and host CPU at least as good as the M0 harness | done: 58.5 fps at 24 % host CPU against 12 to 13 fps at 28 to 34 % | `../M0/metrics.md`, comparison table |
| Audio works | guest side only: AAudio track active in `dumpsys audio`; host output through `dsound` unverified | `../M0/audio-flinger-story.txt` |
| Mouse absolute coordinates work | QMP `input-send-event` verified (tap landed at the requested point); the SDL window path (host mouse to `usb-tablet`) has no recorded measurement, although the user drove the guest with the mouse on 2026-09-26 | `whpx-boot.md`, `virgl-04-qmp-tap-cursor-on-probe.png` |
| Keyboard works | QMP `send-key` verified through GRUB and the installer; the SDL path is evidenced by the Google account the user typed into the QEMU window on 2026-09-26 | `../M0/google-account-added.txt`, `../M0/google-account.md` |
| adb works | done (loopback forward, no guest setting) | `whpx-boot.md` |
| Data survives a reboot | done: the installed system, the game's 5.1 GB data, the guest account and the Google account survived QEMU restarts and the host's own reboot on 2026-09-26 | `../M0/findings-20260926.md` |
| Verified on an Intel integrated GPU and a discrete GPU, or listed as unverified | discrete done (RTX 2080 SUPER); no Intel GPU on this host, listed | `../../KNOWN_LIMITATIONS.md` |

Related M1 work items outside the completion list: QMP `query-status`,
`screendump`, `system_powerdown` and `input-send-event` latency (`whpx-boot.md`,
step 5); kernel command-line experiments (`../../../guest/kernel-cmdline.md`).

## Unverified at the start of 2026-09-26 afternoon, and how each is checked

| Item | Why it matters | Verification |
|---|---|---|
| Voiced story under virgl | Scenario step 6 was measured only on the software entry | Play a story episode with voice on the running virgl guest; sample `top`, `meminfo`, host CPU, SurfaceFlinger latency and `dumpsys audio`; screenshot |
| Host audio output | `dsound` acceptance by QEMU is not the same as sound coming out | Read the peak meter of QEMU's audio session on the host's default render device (WASAPI `IAudioMeterInformation`) during a voiced line and during silence |
| 10-minute session under virgl | The 37-minute session was on the software entry | `ps` elapsed time of the game process on the current virgl boot plus the crash buffer |
| Quit and relaunch under virgl | Scenario step 6, last item | In-game quit, relaunch, screenshot of the lobby with the same account |
| Mouse through the SDL window | `usb-tablet` absolute coordinates are what M2 users will use | Post `WM_MOUSEMOVE` at a known client coordinate of the QEMU window and read `ABS_X`/`ABS_Y` from the guest's tablet device with `getevent`; expected value is `x * 32767 / (client width - 1)` |
| Keyboard through the SDL window | Same path as above for keys | Direct artifact only if focus can be taken safely; otherwise the typed Google sign-in stands as evidence, since QMP `send-key` and the SDL window feed the same `usb-kbd` device |
| Software entry (`nomodeset HWACCEL=0`) with `edid=off` | The launcher's virgl device string changed after the last software boot | Switch `grubenv`, reboot, confirm boot time, host window, QMP `screendump`, adb; switch back |
| Unity sees 2 cores on a 4-vCPU guest | Possible loss of worker threads | Root cause found today: `/system/etc/cpuinfo.arm64.txt` lists two processors, and the translator serves it as `/proc/cpuinfo` to ARM code. Bind-mount a four-processor copy, relaunch the game, read `SystemInfo ... Cores` from the Unity log, and replay stage 1-3 for an A/B |
| Custom QEMU `0xC0000005` | Release-path binary | Run the custom binary under `cdb` with the 2026-09-25 repro line (`-S -m 256 -device virtio-vga-gl -display sdl,gl=on`, pflash, no disk) and capture the first-chance exception if it recurs |

## Results (2026-09-26 afternoon and evening)

| Item | Result | Evidence |
|---|---|---|
| 10-minute session under virgl | Passed. The game process (pid 3213) started 16:10:38 on the virgl boot with `edid=off` and 8 GiB and was still the same process at 18:05 (1 h 54 min) and through the battles below; the boot's crash buffer holds one `media.swcodec` abort (the known codec crash at 16:11:35) and nothing from the game | `run-virgl-session/ps-game.txt`, `logcat-crash.txt` |
| Host audio output | Passed, with a finding. QEMU's audio session on the host's default render endpoint showed 0.15 to 0.27 peak for the guest's ringtone previews and 0.06 to 0.10 for the game once the guest media volume was raised; at Bliss's default media index 5 of 15 (-33 dB) the game arrived as 0.000 to 0.002, in practice silence | `run-virgl-session/host-audio-ringtone-preview.txt`, `host-audio-game.txt`, `run-virgl-battle14/host-audio-battle.txt`; procedure in `whpx-boot.md` |
| Mouse through the SDL window | Passed. Three real cursor positions over the QEMU window arrived on the guest tablet as exactly `floor(x * 32767 / 1280)`, `floor(y * 32767 / 800)`. Posted `WM_MOUSEMOVE` messages are ignored | `sdl-mouse-absolute.txt` |
| Keyboard through the SDL window | Evidenced by the typed Google sign-in of 2026-09-26; no separate measurement | `../M0/google-account-added.txt` |
| Battles 1-4 and 1-5 under virgl (not in the M0 list; played while looking for a story entry point) | Passed, 3 stars each. 1-4 wave 1: 58.3 fps, p50 17.1 ms, p90 20.9, p99 26.7; game process 132 % of a vCPU, guest MemFree 0.33 GB with 4.8 GB cached | `run-virgl-battle14/` |
| Quit and relaunch under virgl | Passed. In-game 종료 ended the process within 3 s; `monkey` relaunch reached the title with the same account number in about 100 s and the lobby after one tap; no translator abort, the known `media.swcodec` abort on the intro video | `run-virgl-session/quit-relaunch.txt`, `screenshot-relaunch-*.png` |
| Voiced story under virgl | Not reachable today. The theater (극장) is still locked at player level 6, stages 1-4 and 1-5 carry no story scene, and the world list has no story tab; the prologue that was measured on the software entry cannot be replayed until the theater unlocks. Voice through the same AAudio path is covered by the battle audio measurements above. Stays open as the one M1 scenario item to redo when the game's progression allows | `run-virgl-session/` |
| Software entry with `edid=off` | Passed. `nomodeset HWACCEL=0` booted to `sys.boot_completed` in 34 s with `virtio-vga-gl,edid=off`: efifb 1024x768 from the firmware GOP, SwiftShader GLES, QMP `screendump` works (no GL scanout on this entry), adb up; switched back to the virgl entry afterwards (34 s, virgl, 60 Hz) | `run-nomodeset-edidoff/` |
| Unity sees 2 cores | Root-caused and measured: `/system/etc/cpuinfo.arm64.txt` has two processor blocks and libnativebridge bind-mounts it over `/proc/cpuinfo` for ARM apps. A four-processor file made Unity report `Cores = 4` but changed neither the thread inventory nor the frame rate (both runs 48 to 54 fps in the same replay); the stock file stays | `cpuinfo-experiment.txt`, `run-virgl-battle13-cores4/`, `run-virgl-battle13-cores2-control/` |
| Custom QEMU `0xC0000005` | Reproduced 10 of 10 under cdb and 9 of 9 without it, in 1 to 3 s, with any `-display sdl,gl=on` (VGA or virtio-vga-gl, with or without pflash); `-display sdl` without GL refuses `virtio-vga-gl` as expected and does not crash. Root cause: `ui/sdl2.c:127` calls `eglGetCurrentDisplay` through libepoxy, no `libEGL.dll` in the bundle, null call. Fixed by bundling ANGLE; the rebuilt binary stays alive 3 of 3 from a minimal PATH | `custom-qemu-cdb/`, `custom-qemu-build.md` |
| Exit of the guest QEMU at 19:28 | The distribution QEMU instance started at 19:03 (pid 15240) was closed by the user while a full QEMU compile was loading the host; not a fault. Its stderr ended with about 400 `failed to get xsave state` lines, which is what the distribution snapshot prints while QEMU reads vCPU state on the way out. Restarted at 19:53 (34 s to boot, game data intact) | `%LOCALAPPDATA%\OpenMobileEmulator\logs\qemu-default-20260926-190346538.stderr.log` |
