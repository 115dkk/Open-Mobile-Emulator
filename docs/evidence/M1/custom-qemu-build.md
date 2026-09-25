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
| Patches | `0001-symlink-install-tree-skip-without-symlink-privilege.patch` (see `qemu-build/patches/README.md`) |
| Build date (UTC) | 2026-09-25T05:26:46Z |
| `--version` | `QEMU emulator version 11.1.1 (v11.1.1-dirty)` (dirty = patched index tree) |
| Executable SHA-256 | `ef3d62f55b78e49126e9d8b16a497a0087286e5401dceade4b6b1e56197d9caf` |
| Bundled DLLs | 19, resolved with `ldd` from UCRT64 (`dist/qemu/dll-origins.tsv`) |

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

## Open issue: intermittent access violation at start-up

Three episodes of `0xC0000005` (or a start-up hang with no output) were observed
between 18:44 and 18:53 local time, each lasting minutes, then the same command
lines ran fine:

- 18:44: first launch after the build, `-device virtio-vga-gl -display sdl,gl=on` with pflash and QMP, exit `0xC0000005` after 6 s.
- 18:45: five launches including `--version` produced no output within 12 to 15 s and were killed.
- 18:53: 10 of 10 launches of `-m 256 -S -device virtio-vga-gl -display sdl,gl=on` with pflash exited `0xC0000005`, while 10 of 10 launches of the distribution QEMU 11.1.0 with the same arguments ran.
- 18:56 onward: the identical command line ran 3 times in a row, and every isolation combination ran.

No Windows Error Reporting event or report was recorded (WER appears disabled).
Suspects, none confirmed: Defender real-time scan of the freshly built binaries,
the MacType hook DLL that is injected into every windowed process, or a
timing-dependent fault in GL context or virgl initialisation. Next step: install
`mingw-w64-ucrt-x86_64-gdb` in UCRT64 and capture a backtrace of a crashing run,
or enable WER LocalDumps with the user's consent. Until then M0 and M1 experiments
use the distribution QEMU (`OME_QEMU_DIR`, runbook step 0) and the custom build is
the release-path artifact only.

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
| Size | 249,531,104 bytes |
| SHA-256 (`dist/SHA256SUMS`) | `dfe3bab9b8e4c9ecc9020035863a5767b2fba16f3053567a7b1d8d53dc350bac` |
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
