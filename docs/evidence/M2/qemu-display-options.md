# Custom QEMU display options

Date: 2026-09-27. Built and verified on the host recorded in
`docs/evidence/M0/environment.md`.

## Pins and patches

| Item | Value |
|---|---|
| QEMU tag | `v11.1.1` |
| Upstream commit | `c3d48b7d1e89604920e5b81b91140c2ad39a1943` |
| Build command | `pwsh -NoProfile -File .\qemu-build\Build-Qemu.ps1 -Step all -Clean` |
| Build result | Exit 0 |
| Build date (UTC) | `2026-09-27T03:38:59Z` |
| Executable | `qemu-build/out/bin/qemu-system-x86_64.exe` |
| Executable SHA-256 | `82f259806c13f907f9d7462e2f2a876d123c27b814ac0378ffd34f2059960b18` |
| `--version` | `QEMU emulator version 11.1.1 (v11.1.1-dirty)` |

| Patch | SHA-256 |
|---|---|
| `0001-symlink-install-tree-skip-without-symlink-privilege.patch` | `7c719fae9f059951a4a05b17db93af0d9a8a0a7cac1f591fa7901244e9cd2a4c` |
| `0002-virtio-gpu-refresh-rate-property.patch` | `7df0852cb30b19f1c1dd2e052fc2b99c6439f4a5d97fbabb57456a20268987f4` |
| `0003-ui-sdl2-swap-interval-option.patch` | `6a0b0eb9767ec0c31cba2d68d3ccb812dd1d5bee8d019b64a2327cb0ec58ef71` |

Both new patches are `git format-patch` mail files authored and signed off by
`Open Mobile Emulator contributors <noreply@example.invalid>`. QEMU's
`checkpatch.pl --no-tree` reported zero errors and zero warnings for each file.
The external QEMU checkout remains at the pinned upstream HEAD; all three OME
patches are applied only in its Git index. The clone step's reverse checks
reported each of 0001, 0002 and 0003 as already applied.

The first 0002 implementation added `-display sdl,refresh-rate=<hz>`. A guest
boot proved that 120 Hz reached EDID, but SDL's initial 640x480 window size also
replaced virtio-gpu's configured 1280x800 geometry. Evidence is in
`docs/evidence/M2/sizing-20260927/bootC-guest.txt`. That patch was deleted and
replaced by the device-side `refresh_rate` property documented here. The final
patch does not add an SDL refresh-rate option and does not send UI geometry.

## Source changes

Patch 0002 has subject `virtio-gpu: add configurable EDID refresh rate` and
changes:

- upstream `include/hw/virtio/virtio-gpu.h` hunks at lines 131 and 176: adds
  `virtio_gpu_base_conf.refresh_rate` and the unsigned `refresh_rate` property,
  with mHz as the unit and 0 as the default;
- upstream `hw/display/virtio-gpu-base.c` hunk at line 108: uses a non-zero UI
  backend rate when present and otherwise retains the configured device rate;
- upstream `hw/display/virtio-gpu-base.c` hunk at line 194: rejects non-zero
  values outside 30000 through 240000 mHz and initializes scanout 0's requested
  EDID rate from the device property next to its configured `xres` and `yres`.

The source headers state GPL version 2 for `virtio-gpu.h` and GPL version 2 or
later for `virtio-gpu-base.c`; the patch body records both statements exactly.

Patch 0003 has subject `ui/sdl2: add configurable GL swap interval` and changes:

- upstream `qapi/ui.json` hunk at line 1477: adds optional integer
  `swap-interval` and documents `-1`, `0` and `1`;
- upstream `ui/sdl2.c` hunk at line 24: includes QAPI and error-reporting APIs;
- upstream `ui/sdl2.c` hunks at lines 110 and 121: selects the requested
  interval or upstream default 0, warns when SDL refuses it, and retries
  interval 0 only when adaptive interval `-1` is refused;
- upstream `ui/sdl2.c` hunk at line 858: accepts only `-1`, `0` or `1` before
  SDL initialization.

## Build and distribution

The distribution step reported:

```text
Launcher executable: /c/Users/USER/AppData/Local/OpenMobileEmulator/qemu-build/out/bin/qemu-system-x86_64.exe
Distribution: /c/Users/USER/AppData/Local/OpenMobileEmulator/qemu-build/dist/qemu/bin/qemu-system-x86_64.exe
QEMU emulator version 11.1.1 (v11.1.1-dirty)
Copyright (c) 2003-2026 Fabrice Bellard and the QEMU Project developers

[2026-09-27T03:40:03Z] third-party
```

The last 20 lines of the successful build log were:

```text
./share/qemu/s390-ccw.img: OK
./share/qemu/skiboot.lid: OK
./share/qemu/slof.bin: OK
./share/qemu/trace-events-all: OK
./share/qemu/u-boot-sam460.bin: OK
./share/qemu/u-boot.e500: OK
./share/qemu/vgabios-ati.bin: OK
./share/qemu/vgabios-bochs-display.bin: OK
./share/qemu/vgabios-cirrus.bin: OK
./share/qemu/vgabios-qxl.bin: OK
./share/qemu/vgabios-ramfb.bin: OK
./share/qemu/vgabios-stdvga.bin: OK
./share/qemu/vgabios-virtio.bin: OK
./share/qemu/vgabios-vmware.bin: OK
./share/qemu/vgabios.bin: OK
./share/qemu/vof-nvram.bin: OK
./share/qemu/vof.bin: OK
Source offer: /c/Users/USER/AppData/Local/OpenMobileEmulator/qemu-build/dist/qemu-source-offer-v11.1.1-c3d48b7d1e89.tar.gz
Members: 84033
Checksums: /c/Users/USER/AppData/Local/OpenMobileEmulator/qemu-build/dist/SHA256SUMS
```

`dist/SHA256SUMS` passed `sha256sum -c`. The source offer is
`qemu-source-offer-v11.1.1-c3d48b7d1e89.tar.gz`, 249,531,995 bytes, SHA-256
`ee6e271e9c0edf058416c9a73d814de5644b7618e9d183c5b7ca69f4055c8bfd`.
MSYS2 `tar -tzf` found both final patch members:

```text
qemu-source-offer-v11.1.1-c3d48b7d1e89/qemu-build/patches/0002-virtio-gpu-refresh-rate-property.patch
qemu-source-offer-v11.1.1-c3d48b7d1e89/qemu-build/patches/0003-ui-sdl2-swap-interval-option.patch
```

`qemu-build/out/bin/ome-patches.txt` and the copy in `dist/qemu/bin` are
byte-identical and contain:

```text
0001-symlink-install-tree-skip-without-symlink-privilege.patch
0002-virtio-gpu-refresh-rate-property.patch
0003-ui-sdl2-swap-interval-option.patch
```

## QMP option verification

The freshly built executable was started with:

```text
-machine q35 -nodefaults -S -device virtio-vga-gl,id=gpu0,edid=on,refresh_rate=120000 -display sdl,gl=on,swap-interval=1 -qmp tcp:127.0.0.1:4445,server=on,wait=off
```

A PowerShell TCP client sent `qmp_capabilities`, listed
`/machine/peripheral`, read `/machine/peripheral/gpu0` property `refresh_rate`,
queried display options, then sent `quit`. The JSON lines received were:

```json
{"QMP": {"version": {"qemu": {"micro": 1, "minor": 1, "major": 11}, "package": "v11.1.1-dirty"}, "capabilities": ["oob"]}}
{"return": {}}
{"return": [{"name": "type", "type": "string"}, {"name": "gpu0", "type": "child<virtio-vga-gl>"}]}
{"return": 120000}
{"return": {"gl": "on", "swap-interval": 1, "type": "sdl"}}
{"timestamp": {"seconds": 1790480612, "microseconds": 367224}, "event": "SHUTDOWN", "data": {"guest": false, "reason": "host-qmp-quit"}}
```

The QEMU process exited 0 and wrote nothing to standard error. No guest VM was
started for this verification.

## Invalid option verification

Both invalid values were rejected at startup and exited 1. Standard error was:

```text
C:\Open Mobile Emulator\qemu-build\out\bin\qemu-system-x86_64.exe: -device virtio-vga-gl,edid=on,refresh_rate=20000: refresh_rate must be between 30000 and 240000 mHz
C:\Open Mobile Emulator\qemu-build\out\bin\qemu-system-x86_64.exe: SDL swap interval must be -1, 0, or 1
```
