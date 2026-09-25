# arm64-probe.apk run (2026-09-25, guest default, Skylake-Client-v4, nomodeset HWACCEL=0)

## logcat lines

```
09-25 21:55:02.252  1050  1076 I ActivityManager: Start proc 5650:org.ome.arm64probe/u0a286 for next-top-activity {org.ome.arm64probe/org.ome.arm64probe.MainActivity}
09-25 21:55:02.294  5650  5650 I ndk_translation: Initialized NDK translation (aarch64), version 0.2.3
09-25 21:55:02.418  5650  5650 I OMEProbe: native arm64-v8a lib loaded;SUPPORTED_ABIS=x86_64,arm64-v8a,x86,armeabi-v7a,armeabi;os.arch=aarch64;CPU_ABI=arm64-v8a
09-25 21:55:02.687  1050  1065 I ActivityTaskManager: Displayed org.ome.arm64probe/.MainActivity: +451ms
09-25 21:55:30.688  5714  5714 F libc    : Fatal signal 11 (SIGSEGV), code 128 (SI_KERNEL), fault addr 0x0 in tid 5714 (ndk_translation), pid 5714 (ndk_translation)
09-25 21:55:30.784  5718  5718 F DEBUG   : Cmdline: /system/bin/ndk_translation_program_runner_binfmt_misc_arm64 /data/local/tmp/hello_arm64 /data/local/tmp/hello_arm64
09-25 21:55:30.784  5718  5718 F DEBUG   : pid: 5714, tid: 5714, name: ndk_translation  >>> /system/bin/ndk_translation_program_runner_binfmt_misc_arm64 <<<
09-25 21:55:30.942  5723  5723 F libc    : Fatal signal 11 (SIGSEGV), code 128 (SI_KERNEL), fault addr 0x0 in tid 5723 (ndk_translation), pid 5723 (ndk_translation)
09-25 21:55:31.023  5726  5726 F DEBUG   : Cmdline: /system/bin/ndk_translation_program_runner_binfmt_misc_arm64 /data/local/tmp/hello_arm64 ./hello_arm64
09-25 21:55:31.023  5726  5726 F DEBUG   : pid: 5723, tid: 5723, name: ndk_translation  >>> /system/bin/ndk_translation_program_runner_binfmt_misc_arm64 <<<
```

## Program runner with the static arm64 binary

`ro.enable.native.bridge.exec` is empty on this image, so `init` never mounts
binfmt_misc (see `/system/etc/init/ndk_translation.rc`) and `hello_arm64` cannot
be executed directly from the shell ("not executable: 64-bit ELF file"). Invoking the
runner by hand:

```
/system/bin/ndk_translation_program_runner_binfmt_misc_arm64 /data/local/tmp/hello_arm64 /data/local/tmp/hello_arm64
Segmentation fault   (exit 139)
```

The fixture is a statically linked ET_EXEC binary (NDK 28, `-static`). The runner is
built for the binfmt_misc `arm64_dyn`/`arm64_exe` flow with bionic's dynamic loader
from `/system/lib64/arm64/`; a static binary crashing there says nothing about the
translator's app path, which the APK above exercises. Follow-up: build the fixture
dynamically linked as well and retry through the runner (`translator/smoke.ps1`).

`/proc/<pid>/maps` of the app is not readable from the adb shell on this
userdebug build (permission denied), so the library list is not included.
