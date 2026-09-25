# Guest test summary

- Guest: `default`
- Serial: `127.0.0.1:5555`
- Package: `com.epidgames.trickcalrevive`
- Recorded: 2026-09-25T21:51:17.5700705+09:00

| Step | Status | Evidence | Details |
|---|---|---|---|
| adb boot | Pass | `boot.txt` |  |
| native bridge properties | Pass | `bridge-props.txt` |  |
| guest CPU flags | Pass | `cpuinfo.txt` |  |
| APK install | Skipped | `install.txt` | SkipInstall selected |
| app launch | Fail | `launch.txt` | monkey failed with exit code 252: args: [-p, com.epidgames.trickcalrevive, -c, android.intent.category.LAUNCHER, 1]   arg: "-p"   arg: "com.epidgames.trickcalrevive"   arg: "-c"   arg: "android.intent.category.LAUNCHER"   arg: "1"  data="com.epidgames.trickcalrevive"  data="android.intent.category.LAUNCHER" |
| screenshot | Pass | `screenshot-1.png` |  |
| top sample | Pass | `top.txt` |  |
| gfxinfo sample | Pass | `gfxinfo.txt` |  |
| meminfo sample | Pass | `meminfo.txt` |  |
| logcat tail | Pass | `logcat-tail.txt` |  |
| native bridge error scan | Pass | `logcat-bridge-errors.txt` |  |
| host CPU and memory sample | Pass | `host-sample.txt` |  |
