# Distribution QEMU capability probe

Date: 2026-09-25. Host: see `docs/evidence/M0/environment.md`. WHPX not yet
active (feature enabled, reboot pending), so the probe ran under TCG.

## Binary

- Installer: winget `SoftwareFreedomConservancy.QEMU` 11.1.0, which downloaded
  `https://qemu.weilnetz.de/w64/2026/qemu-w64-setup-20260811.exe` (197 MB).
- `qemu-system-x86_64.exe --version`: `QEMU emulator version 11.1.0 (v11.1.0-12130-ge470268ff4)`
- `-accel help`: tcg, whpx
- `-display help`: none, gtk, sdl, egl-headless, curses, spice-app, dbus
- `-audiodev help`: none, dbus, dsound, jack, sdl, spice, wav
- `-device help` includes: VGA, virtio-vga, virtio-vga-gl, virtio-gpu-gl-pci
- `share\` firmware: `edk2-x86_64-code.fd` (3.48 MB), `edk2-i386-vars.fd` (0.52 MB), plus secure-boot and other-arch variants

## virgl start-up probe

Command (each element one argv entry; paths with spaces must not be split):

```
qemu-system-x86_64.exe -accel tcg -machine q35 -m 256 -S
  -device virtio-vga-gl -display sdl,gl=on
  -qmp tcp:127.0.0.1:4466,server=on,wait=off
  -drive if=pflash,format=raw,readonly=on,file=C:\Program Files\qemu\share\edk2-x86_64-code.fd
```

Result: process alive after 6 s, no stderr output. QMP exchange:

```
{"execute":"qmp_capabilities"}      -> {"return": {}}
{"execute":"query-display-options"} -> {"return": {"gl": "on", "type": "sdl"}}
{"execute":"query-status"}          -> round trip 2.46 ms (idle, TCG, paused guest)
{"execute":"quit"}                  -> SHUTDOWN event, reason host-qmp-quit, exit code 0
```

Conclusion: the 2026-08-11 distribution build is compiled with virglrenderer
and OpenGL, contrary to the older note in CLAUDE.md 1.3 (now amended). Whether
a Bliss guest actually gets accelerated GL through it is an M1 question and is
not shown by this probe.

## Pitfall recorded

`Start-Process -ArgumentList` does not quote elements containing spaces, so the
`-drive ...file=C:\Program Files\...` argument was split and QEMU failed with
`Could not open 'C:\Program'`. Launch QEMU via `ProcessStartInfo.ArgumentList`
(one Add per argument) instead.
