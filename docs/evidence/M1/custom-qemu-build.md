# Custom QEMU build (M1 step 1)

Date: 2026-09-25. Built with `qemu-build/Build-Qemu.ps1 -Step all` on the host in
`docs/evidence/M0/environment.md`; the third run succeeded after two fixes noted
below. Work tree: `%LOCALAPPDATA%\OpenMobileEmulator\qemu-build` (repository path
contains whitespace), exposed in the repository as the junction `qemu-build\out`.

## Pins and result

| Item | Value |
|---|---|
| QEMU tag | `v11.1.1` |
| Upstream commit | `c3d48b7d1e89604920e5b81b91140c2ad39a1943` |
| Patches | `0001-symlink-install-tree-skip-without-symlink-privilege.patch`, `0002-virtio-gpu-refresh-rate-property.patch`, `0003-ui-sdl2-swap-interval-option.patch` (see `qemu-build/patches/README.md`) |
| Build date (UTC) | 2026-09-26T10:30:31Z (rebuilt with the ANGLE bundling below; the first build was 2026-09-25T05:26:46Z) |
| `--version` | `QEMU emulator version 11.1.1 (v11.1.1-dirty)` (dirty = patched index tree) |
| Executable SHA-256 | `82f259806c13f907f9d7462e2f2a876d123c27b814ac0378ffd34f2059960b18` (ANGLE rebuild: `2b052552b9b8f4bb3dd1f636f0eb06189cf8944a29b7c048170325e3ccaed902`; first build: `ef3d62f55b78e49126e9d8b16a497a0087286e5401dceade4b6b1e56197d9caf`) |
| Bundled DLLs | 24, resolved with `ldd` from UCRT64 (`dist/qemu/dll-origins.tsv`): the first build's 19 plus `libEGL.dll`, `libGLESv2.dll`, `libjpeg-8.dll`, `libpng16-16.dll`, `libstdc++-6.dll` |

Rebuilt 2026-09-27 with 0002 and 0003 (see `docs/evidence/M2/qemu-display-options.md`).

Configure options (`out/configure-options.txt`):

```
--target-list=x86_64-softmmu
--prefix=/c/Users/USER/AppData/Local/OpenMobileEmulator/qemu-build/out/install
--bindir=bin
--datadir=share/qemu
--python=/ucrt64/bin/python
--enable-whpx
--enable-virglrenderer
--enable-opengl
--enable-sdl
--enable-slirp
--enable-dsound
--enable-tools
--enable-install-blobs
--disable-gtk
--disable-sdl-image
--disable-modules
--disable-rust
--disable-docs
--disable-werror
--disable-download
```

Configure summary lines observed: `whpx: enabled`, `virglrenderer: enabled`,
`opengl: enabled`, `sdl: enabled`, `dsound: enabled`, `slirp: enabled`,
`gtk: disabled`, `docs: disabled`, `modules: disabled`, `wrap_mode: nodownload`,
plus the patch's one-time warning
`skipping the qemu-bundle tree: creating symbolic links needs Developer Mode or administrator rights on Windows (WinError 1314)`.

Toolchain and library packages at build time (`out/pacman-lock.txt`):

```
mingw-w64-ucrt-x86_64-SDL2 2.32.10-1
mingw-w64-ucrt-x86_64-gcc 16.2.0-4
mingw-w64-ucrt-x86_64-glib2 2.90.0-1
mingw-w64-ucrt-x86_64-libepoxy 1.5.10-7
mingw-w64-ucrt-x86_64-libslirp 4.9.3-1
mingw-w64-ucrt-x86_64-meson 1.12.1-1
mingw-w64-ucrt-x86_64-ninja 1.13.2-1
mingw-w64-ucrt-x86_64-pixman 0.46.4-3
mingw-w64-ucrt-x86_64-python 3.14.7-1
mingw-w64-ucrt-x86_64-virglrenderer 1.3.0-1
```

Packages that provided the bundled DLLs (`dist/qemu/dll-packages.txt`):

```
mingw-w64-ucrt-x86_64-SDL2
mingw-w64-ucrt-x86_64-bzip2
mingw-w64-ucrt-x86_64-gettext-runtime
mingw-w64-ucrt-x86_64-glib2
mingw-w64-ucrt-x86_64-libepoxy
mingw-w64-ucrt-x86_64-libffi
mingw-w64-ucrt-x86_64-libgcc
mingw-w64-ucrt-x86_64-libiconv
mingw-w64-ucrt-x86_64-libslirp
mingw-w64-ucrt-x86_64-libwinpthread
mingw-w64-ucrt-x86_64-ncurses
mingw-w64-ucrt-x86_64-pcre2
mingw-w64-ucrt-x86_64-pixman
mingw-w64-ucrt-x86_64-virglrenderer
mingw-w64-ucrt-x86_64-zlib
mingw-w64-ucrt-x86_64-zstd
```

## Start-up checks from plain PowerShell (no MSYS2 on PATH)

`-accel help` lists `tcg whpx`; `-display help` lists `sdl`; `-device help` lists
`virtio-vga-gl` (checked inside the UCRT64 shell by `copy_runtime`). From plain
PowerShell with a PATH that excludes `C:\msys64` and Git:

| Test | Result |
|---|---|
| `--version` (4 PATH/cwd variants) | exit 0 in 25 to 37 ms |
| `-S -display none` with the bundled `edk2-x86_64-code.fd` (pflash) | runs |
| `-S -device VGA -display sdl` | runs |
| `-S -device virtio-vga -display sdl` | runs |
| `-S -device VGA -display sdl,gl=on` | runs |
| `-S -device virtio-vga-gl -display sdl,gl=on` (with and without pflash, `-m 128` and `-m 256`) | runs in the isolation matrices |
| Loaded modules of a running instance | only `out\bin\*.dll`, Windows system DLLs, and the host's `MacType64.dll` (MacType hook) |

The bundled `edk2-x86_64-code.fd` is byte-identical to the distribution build's
(3,653,632 bytes, same SHA-256).

## Access violation at start-up: root cause found on 2026-09-26

On 2026-09-25 three episodes of `0xC0000005` (or a start-up hang with no output)
were seen between 18:44 and 18:53, each lasting minutes, and then the same command
lines ran; the suspects noted that day (Defender, the MacType hook, GL timing) were
wrong. On 2026-09-26 the crash reproduced 10 of 10 times under `cdb` and 9 of 9
times without a debugger, within 1 to 3 s, with every `-display sdl,gl=on`
configuration (`VGA` or `virtio-vga-gl`, with or without pflash); `-display sdl`
without GL does not crash (`custom-qemu-cdb/summary.txt`).

Stack from `cdb` with the build's DWARF line table resolved by `addr2line`
(`custom-qemu-cdb/run-1.txt`, `run-breakpoint.txt`):

| Frame | Location |
|---|---|
| 0 | `0x0` (call through a null pointer, `rip=0`) |
| 1 | `sdl2_window_create`, `ui/sdl2.c:127`: `qemu_egl_display = eglGetCurrentDisplay();` |
| 2 | `sdl2_gl_console_init`, `ui/sdl2-gl.c:313` |
| 3 | `sdl2_display_init`, `ui/sdl2.c:975` |
| 4 | `qemu_init_displays`, `system/vl.c:2710` |

`eglGetCurrentDisplay` goes through libepoxy's dispatch pointer. Breaking at the
call showed the pointer chain intact (`rax` = libepoxy's `epoxy_eglGetCurrentDisplay`
variable, holding its resolver in `.text`); the resolver then looks for
`libEGL.dll`, finds none in the bundle or on `PATH`, returns NULL, and the
rewritten dispatch calls address 0. The distribution QEMU 11.1.0 in
`C:\Program Files\qemu` runs the same line (its exe imports the same 16 epoxy EGL
symbols and upstream `master` and `stable-11.1` carry the line unguarded) but
ships ANGLE next to the exe: `libEGL.dll`, `libGLESv2.dll`, `libGLESv1_CM.dll`
and the `*_vulkan_secondaries` / `*_with_capture` variants. With
`C:\Program Files\qemu` prepended to `PATH`, the unmodified custom binary ran 3 of
3 times. The 2026-09-25 "then it worked" episodes are the same mechanism: whether a
`libEGL.dll` was reachable through the launching shell's `PATH` or working
directory (browsers, Electron and Qt applications ship one).

Fix applied the same evening: `pins.env` pins
`mingw-w64-ucrt-x86_64-angleproject=2.1.r25748.890b5d8f-6` (BSD-3-Clause),
`copy_runtime` in `build-qemu.sh` copies `libEGL.dll` and `libGLESv2.dll` from
UCRT64 and lets the existing `ldd` queue pull their dependencies
(`libstdc++-6.dll`, `libjpeg-8.dll`, `libpng16-16.dll`), and the generated block of
`THIRD_PARTY.md` lists angleproject, libjpeg-turbo, libpng and libstdc++. This
matches the distribution build and keeps the QEMU source unpatched; the bundle
grows by about 12 MB. `Build-Qemu.ps1 -Clean -Step all` rebuilt everything in
764 s, then `dist`, `thirdparty` and `sourceoffer` were run again. Verification
of the rebuilt binary: three starts with `-m 256 -S -device virtio-vga-gl
-display sdl,gl=on` and the bundled pflash from a `PATH` of
`C:\Windows\System32;C:\Windows` only, all alive after 8 s
(`custom-qemu-cdb/summary.txt`); `ci/Invoke-AllChecks.ps1` passes. Booting the
Bliss guest on the custom build is the next check; M0 and M1 evidence stays on
the distribution QEMU (`OME_QEMU_DIR`, runbook step 0).

Guest boot on the custom build (19:57, `run-custom-qemu-boot/boot.txt`): with
`OME_QEMU_DIR` unset the launcher picked `qemu-build\out\bin` (source
`custom-build`, version 11.1.1) and `Start-Guest.ps1 -Name default -Gpu virgl`
brought Bliss to `sys.boot_completed` in 34 s, the same as the distribution
build: Mesa virgl on the RTX 2080 SUPER, VSYNC period 16.68 ms (60 Hz mode from
`edid=off`), 1280x800, native bridge on, adb up. The custom build's stderr holds
only the two CPUID warnings; the `failed to get xsave state` lines that the
distribution snapshot prints on every boot do not appear with v11.1.1. The
tested game then started on this build (after the guest was woken from its
lock screen with `input keyevent 224; wm dismiss-keyguard`): its asset-loading
screens presented at 57.8 and 58.2 fps (p50 16.6 to 17.1 ms), the game's audio
at 0.15 peak on the host session of the custom `qemu-system-x86_64.exe`, the
usual `media.swcodec` abort on the intro video and nothing else in the crash
buffer (`run-custom-qemu-boot/boot.txt`, `sf-latency-*.txt`, `host-audio.txt`).

Two build-script observations from the rebuild, not fixed today: MSYS2 bash
failed to fork right after the `all` step finished its install
(`dofork: child -1 - CreateProcessW failed ... errno 13`, exit 254), so the
`dist` step had to be run separately; and a second `-Step build` on the same
tree stops with "Inputs changed; use -Clean" because the configure fingerprint
is computed before `configure-options.txt` exists on a clean run.

## Failures met and fixed on the way

| Failure | Cause | Fix |
|---|---|---|
| Meson `subprojects download`: "path is on mount 'C:', start on mount 'Z:'" | The wrapper mapped the repository (path with spaces) to a SUBST drive; Meson resolves the real path | Work tree moved outside the repository, junction for `qemu-build\out` |
| configure: "Could not find a version that satisfies the requirement wheel>=0.34.2" | QEMU 11 installs the `[tooling]` group offline; MSYS2 `python-pip` and `python-wheel` were missing | Added both packages to `pins.env` (as in the MSYS2 QEMU PKGBUILD makedepends) |
| `meson setup`: postconf `symlink-install-tree.py` failed | Unprivileged Windows user cannot create symbolic links (WinError 1314) | Patch 0001 skips the `qemu-bundle` tree with one warning |
| Third-party step hung four hours | `git diff --quiet` recursed into submodules and `git status` processes deadlocked on `index.lock` in roms/edk2 and nested openssl | `--ignore-submodules`; submodule check reduced to pin and inventory match |
| `verify_build`: "Build scripts/pins/patches changed" after editing the wrapper | Input hash covered the build scripts themselves | Hash narrowed to pins, patches and recorded configure options |

## Source offer (R4)

Produced by `Build-Qemu.ps1 -Step sourceoffer` on 2026-09-25 after the two
verification fixes above, streaming `git archive` output for the patched QEMU
index tree, every firmware submodule at its recorded commit, and the three pinned
Meson wraps into one tarball without extracting anything on the host.

| Item | Value |
|---|---|
| File | `dist/qemu-source-offer-v11.1.1-c3d48b7d1e89.tar.gz` |
| Size | 249,527,191 bytes (regenerated 2026-09-26 with the ANGLE rebuild; the 2026-09-25 tarball was 249,531,104 bytes) |
| SHA-256 (`dist/SHA256SUMS`) | `637ea3acdc9cdf6e9845959ed6cfe93658fbb9befd87ec389a9c8a354b9ed6d8` (2026-09-25: `dfe3bab9b8e4c9ecc9020035863a5767b2fba16f3053567a7b1d8d53dc350bac`) |
| Members | 84,031 |

Representative members (type and size from `tar -tvzf`):

```
-rw-r--r-- 0/0           14798 2026-09-25 14:26 qemu-source-offer-v11.1.1-c3d48b7d1e89/BINARY-SHA256SUMS
-rw-r--r-- 0/0            6251 2026-09-25 14:26 qemu-source-offer-v11.1.1-c3d48b7d1e89/SOURCE-OFFER.txt
-rw-r--r-- 0/0            3643 2026-09-25 14:26 qemu-source-offer-v11.1.1-c3d48b7d1e89/THIRD_PARTY.generated.md
-rw-r--r-- 0/0            2097 2026-09-25 14:26 qemu-source-offer-v11.1.1-c3d48b7d1e89/qemu-build/patches/0001-symlink-install-tree-skip-without-symlink-privilege.patch
-rwxrwxr-x root/root     61685 2026-09-25 19:00 qemu-source-offer-v11.1.1-c3d48b7d1e89/qemu-build/src/qemu/configure
lrwxrwxrwx root/root         0 2024-11-07 08:08 qemu-source-offer-v11.1.1-c3d48b7d1e89/qemu-build/src/qemu/roms/edk2/EmulatorPkg/Unix/Host/X11IncludeHack -> /opt/X11/incl
-rwxrwxr-x root/root      3487 2024-11-07 08:08 qemu-source-offer-v11.1.1-c3d48b7d1e89/qemu-build/src/qemu/roms/edk2/edksetup.sh
-rw-r--r-- 0/0            2661 2026-09-25 14:26 qemu-source-offer-v11.1.1-c3d48b7d1e89/qemu-build/src/qemu/subprojects/keycodemapdb/README
```

The `X11IncludeHack` entry is the edk2 symbolic link that broke the extract-based
first version of the script; it is now carried as a link entry exactly as
upstream recorded it. `THIRD_PARTY.generated.md` from the same run was copied
over the repository's `THIRD_PARTY.md` (generated block between the markers).

R4 also names a CI job `qemu-source-offer` that must gate releases. That job does
not exist yet; it belongs to the release workflow (M3) and is listed as open work.
