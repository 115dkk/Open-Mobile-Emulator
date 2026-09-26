# Guest test summary

- Guest: `default`
- Serial: `127.0.0.1:5555`
- Package: `com.epidgames.trickcalrevive`
- Recorded: 2026-09-26T11:25:21.4105624+09:00

| Step | Status | Evidence | Details |
|---|---|---|---|
| adb boot | Pass | `boot.txt` |  |
| native bridge properties | Pass | `bridge-props.txt` |  |
| guest CPU flags | Pass | `cpuinfo.txt` |  |
| APK install | Skipped | `install.txt` | SkipInstall selected |
| app launch | Skipped | `launch.txt` | SkipLaunch selected |
| screenshot | Pass | `screenshot-1.png` |  |
| top sample | Pass | `top.txt` |  |
| gfxinfo sample | Pass | `gfxinfo.txt` |  |
| meminfo sample | Pass | `meminfo.txt` |  |
| logcat tail | Pass | `logcat-tail.txt` |  |
| native bridge error scan | Pass | `logcat-bridge-errors.txt` |  |
| host CPU and memory sample | Pass | `host-sample.txt` |  |
