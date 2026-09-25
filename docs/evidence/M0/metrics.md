# M0 metrics (step 7)

Harness: see `environment.md`. Samples come from `launcher/Invoke-GuestTest.ps1`
(`top -m 10 -n 3`, `dumpsys gfxinfo`, `dumpsys meminfo`, host CPU/memory) and are
kept per run under `run-<timestamp>/`.

## Game scenario (pending)

The game APK comes from the user's own phone (`launcher/Get-GameApk.ps1`,
runbook step 4). The phone was not connectable during the 2026-09-25 session, so
the scenario rows below stay empty until the user gives the signal.

| Scenario step | Guest CPU (`top`) | gfxinfo (frames, janky, 90th/95th/99th ms) | meminfo PSS (MB) | Host CPU % / RAM used | Screenshot | Notes |
|---|---|---|---|---|---|---|
| First run and resource download | | | | | | |
| Guest account creation | | | | | | |
| Tutorial battle | | | | | | |
| Voiced story episode 1 | | | | | | |
| 10-minute continuous session | | | | | | |
| Quit and relaunch | | | | | | |

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
