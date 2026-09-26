# M0 metrics (step 7)

Harness: see `environment.md`. Samples come from `launcher/Invoke-GuestTest.ps1
-SkipInstall -SkipLaunch` (`top -m 10 -n 3`, `dumpsys meminfo`, host CPU/memory)
and are kept per run under `run-<name>/`. The frame rate comes from
`dumpsys SurfaceFlinger --latency` on the game's BLAST surface layer (column 2,
present timestamps; 127-frame ring buffer), because `dumpsys gfxinfo` counts
only the Android view frames of a Unity game (three frames for a whole battle).
Guest CPU is `top`'s process figure on 4 vCPUs (400 % = all four). Host CPU is
`\Processor(_Total)\% Processor Time` for the whole PC with other apps running.
Game version 10644 (`Ver 1.6.44-Production`), Unity 6000.3.13f1 IL2CPP arm64-v8a
under libndk_translation 0.2.3. Recorded 2026-09-26.

## Scenario, software-rendering harness (`nomodeset HWACCEL=0`, SwiftShader, 1024x768)

Second boot of the day; the data download had already happened under virgl
(next section), so the first row is the asset verification that a fresh boot
repeats. Times are host clock.

| Scenario step | Result | Guest CPU (`top`) | meminfo PSS (MB) | Host CPU % / RAM used GB | QEMU working set GB | Presented frames | Screenshot | Notes |
|---|---|---|---|---|---|---|---|---|
| First run and resource download | pass (verification of the 5.1 GB set took about 4 min; four worker threads at 68 % each) | 3.2 GB RES during verification | | | | | `step-07-nomodeset-title-loading.png` | download itself measured under virgl below |
| Guest account creation | pass, account number shown on the title | | | | | | `step-08` to `step-12` | terms: two mandatory boxes only; Play Games sign-in opened the system "Add account" page, Back returned |
| Voiced story (prologue episode, AUTO) | pass, voice track active (`audio-flinger-story.txt`, `audio-flinger-episode-auto.txt`: AAudio player `started`, 1 active track) | 323 % | 2,582 | 33.7 / 22.8 | 6.17 | not measured | `step-14` to `step-22` | rest of the episode skipped through the story menu |
| Tutorial battle 1-1 | pass, 3 stars | 336 % (setup), 320 % (wave) | 2,902 | 28.5 / 22.8 | 6.17 | 10 fps static (p50 95 ms), 12 fps setup with animations (p50 83 ms), 13.3 fps wave (p50 71 ms, p90 83, max 83) | `step-31` to `step-37` | `run-nomodeset-battle/sf-latency-*.txt` |
| 10-minute continuous session | pass: one process (pid 3174) from 10:37:44 launch to the 11:15:09 quit, 37 min, no crash | | | | | | | logcat kept in `logcat-virgl-firstrun-full.txt` for the earlier boot; this boot's crash buffer holds only `media.swcodec` aborts |
| Quit and relaunch | pass: in-game quit ended the process; relaunch showed the same account number and went straight to the lobby | | | | | | `step-39` to `step-42` | Play Games sign-in page appeared again on launch, Back returned |

## Scenario, GPU harness (virgl, `HWC=drm_minigbm GRALLOC=minigbm_arcvm`, 1280x800)

First boot of the day (install, first launch, download) and a third boot for the
battle comparison. The stage is 1-2 in the comparison because 1-1 cannot be
replayed after a perfect clear; same stage type, one more enemy wave.

| Scenario step | Result | Guest CPU (`top`) | meminfo PSS (MB) | Host CPU % / RAM used GB | QEMU working set GB | Presented frames | Screenshot | Notes |
|---|---|---|---|---|---|---|---|---|
| Install and first launch | pass (`run-virgl-firstlaunch/`: install-multiple Success, Unity initialised, bridge error scan empty) | | | | 1.7 (idle guest) | | `step-01`, `step-02` | intro video black (codec abort), tap skipped it |
| Resource download | pass: 5.1 GB prompt, 0.9 GB before the first process death, the rest in about 3 min (about 22 MB/s through slirp) | | | 100 / 24.2 (three samples during download plus decompression) | | | `step-03`, `step-04` | `run-virgl-download/`; the sample's `monkey` relaunch killed the game, see `findings-20260926.md` |
| Post-download loading | pass after reboot; one thread busy | 48 % | 1,122 | 21.1 / 22.8 | 6.31 | | `step-06` | `run-virgl-loading-stall/`, `gfxinfo` 3 frames total |
| Guest account, story, quit and relaunch | done once, under the software harness (above); the account carried over to this boot | | | | | | `step-43`, `step-44` | |
| Battle 1-2 | pass | 88 % | 2,010 | 18.9 / 22.8 | 6.38 | 38.1 fps setup (p50 27 ms, p90 33.5, max 45.8), 38.5 fps wave (p50 26.5 ms, p90 30.7, max 46) | `step-45`, `step-46` | `run-virgl-battle/sf-latency-*.txt` |
| Stage list screen (animated map) with `-m 8192` (fourth boot, 15:36); stage 1-3 was locked, so the taps meant for it did nothing and no battle ran | measured, not a battle | 60 to 64 % | 1,901 | 14.3 / n.a. | 7.17 | 38.2 fps and 37.8 fps in two 10 s windows (p50 26.2 to 26.7 ms, p90 28.6 to 30.8, max 45.2) | `step-49`, `step-50` (both show the stage list) | `run-virgl-8g-battle/`; guest MemFree 1.46 GB, MemAvailable 4.85 GB |
| Title, lobby and stage list screen with `-m 8192` and `-global virtio-vga-gl.edid=off` (seventh boot, 16:10; guest mode 1280x800 at 60 Hz); stage 1-3 still locked, no battle ran | measured, not a battle | 92 % | | | | 58.4 fps title (p50 16.8 ms), 57.9 fps lobby (p50 16.7 ms, p99 35.9), 58.5 fps and 58.4 fps stage list (p50 16.9 to 17 ms, p90 20, max 24.2) | `step-51`, `step-52` (both show the stage list) | `run-virgl-60hz-battle/sf-latency-*.txt` (`setup` and `wave` files are the stage list); MemFree 1.55 GB |
| Battle 1-3, same boot (16:30 to 16:38; the reward node between 1-2 and 1-3 had to be claimed first, then the stage tutorial skipped) | pass, victory, Lv.3 to Lv.4 | 120 to 140 % | 2,049 | 24.1 / n.a. | 7.22 | 58.5 fps active wave with no overlay (p50 17 ms, p90 20.6, p99 28, max 28); 54.3 fps and 57.5 fps with the stage tutorial dialogue over the field | `step-53` to `step-55` | `run-virgl-60hz-battle/sf-latency-battle13-*.txt`; MemFree 1.53 GB |

## Comparison (M1 completion criterion: battle frames and host CPU at least as good as the M0 harness)

| Measure | Software harness (M0), battle 1-1 | virgl, 6 GiB, 75 Hz EDID, battle 1-2 | virgl, 8 GiB, 75 Hz, stage list screen | virgl, 8 GiB, 60 Hz (`edid=off`), battle 1-3 |
|---|---|---|---|---|
| Presented frame rate | 12 to 13 fps | 38 fps | 38 fps | 58.5 fps |
| Frame time p50 / p90 / p99 | 71 to 95 / 83 to 107 / 83 to 119 ms | 27 / 31 / 46 ms | 26 / 29 / 45 ms | 17 / 21 / 28 ms |
| Game process CPU (`top`, 4 vCPU) | 320 to 336 % | 88 % | 60 to 64 % | 120 to 140 % |
| Host CPU (whole PC) | 28 to 34 % | 19 % | 14 % | 24 % |
| Game PSS | 2.6 to 2.9 GB | 2.0 GB | 1.9 GB | 2.05 GB |
| Guest free memory | 80 to 115 MB free (6 GiB) | 282 MB free (6 GiB) | 1.46 GB free (8 GiB) | 1.53 GB free (8 GiB) |

The third column is not a battle: stage 1-3 was still locked on that boot and
the scripted taps stayed on the stage list, noticed only when the screenshots
were reviewed. The fourth column is the real 1-3 battle on the 60 Hz mode.

virgl wins on every row. Raising the guest to 8 GiB (launcher default since
2026-09-26, user approval up to 16 GiB) lifted free memory from about 100 MB to
1.46 GB; its CPU figures are not comparable with the battle columns because
that run stayed on the stage list, and the 60 Hz battle's higher CPU share is
the expected cost of presenting 58 frames instead of 38. The frame rate at
8 GiB stayed at 38 fps with a steady 26.5 ms frame interval. That ceiling
was the guest's 75 Hz display mode from QEMU's generated EDID: the game's frame
fits in one 60 Hz vsync but not in one 75 Hz vsync, so it landed on every
second 75 Hz vsync. With the EDID disabled the guest driver falls back to a
60 Hz mode and the game presents 58 fps everywhere measured, on a 59 Hz host
monitor. Details in `findings-20260926.md`.

## Idle baseline (2026-09-25, `run-20260925-215100`, no game installed)

Taken about two minutes after the first boot, setup wizard on screen. See
`run-20260925-215100/top.txt`, `host-sample.txt`; gfxinfo and meminfo for the game
package were empty because it is not installed.

| Measure | Value |
|---|---|
| Guest `top` summary | 400% cpu total, 396% idle, 4% sys; 6,056,336 kB total, 570,304 kB free, 3,969,716 kB cached |
| Host CPU (three samples, `\Processor(_Total)\% Processor Time`) | 24.8 / 18.5 / 29.5 %, average 24.3 % (whole PC, other apps running) |
| Host RAM | 25.15 GB used of 63.78 GB |
| QEMU working set | about 1.7 GB two minutes after boot |
