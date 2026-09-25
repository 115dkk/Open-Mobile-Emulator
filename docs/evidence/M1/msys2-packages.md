# MSYS2 build environment facts

Date: 2026-09-25. MSYS2 installed with winget `MSYS2.MSYS2` 20260611 to `C:\msys64`
(elevated install), then updated as the normal user with `pacman -Syu` twice.

## Keyring note

The silent elevated install left the pacman keyring uninitialised. `pacman -Sy`
failed with "key 5F944B02... is unknown" and "keyring is not writable" until
`pacman-key --init` and `pacman-key --populate msys2` were run once as the normal
user (no elevation needed; the gnupg directory was owned by that user). Recorded
so `qemu-build/README.md` can mention it.

## Runtime after update

| Package | Version |
|---|---|
| msys2-runtime | 3.6.10-4 |
| pacman | 6.1.0-25 |

## QEMU dependency packages available in UCRT64 (`pacman -Si`, `LC_ALL=C`)

| Package | Version | License (pacman field) |
|---|---|---|
| mingw-w64-ucrt-x86_64-virglrenderer | 1.3.0-1 | MIT |
| mingw-w64-ucrt-x86_64-libepoxy | 1.5.10-7 | MIT |
| mingw-w64-ucrt-x86_64-SDL2 | 2.32.10-1 | Zlib |
| mingw-w64-ucrt-x86_64-libslirp | 4.9.3-1 | BSD-3-Clause |
| mingw-w64-ucrt-x86_64-glib2 | 2.90.0-1 | LGPL-2.1-or-later |
| mingw-w64-ucrt-x86_64-pixman | 0.46.4-3 | MIT |

This settles the question in CLAUDE.md M1 step 1: MSYS2 ships virglrenderer for
UCRT64, so no source build of virglrenderer is needed. Exact versions used by a
given build are frozen in `qemu-build/out/pacman-lock.txt` at build time.
