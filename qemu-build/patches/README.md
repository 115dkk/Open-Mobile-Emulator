# QEMU patches

## Current patches

| File | Upstream file | Purpose | QEMU pin | Evidence |
|---|---|---|---|---|
| `0001-symlink-install-tree-skip-without-symlink-privilege.patch` | `scripts/symlink-install-tree.py` | On Windows an unprivileged user without Developer Mode cannot create symbolic links (WinError 1314), and the postconf script that builds the `qemu-bundle` tree made `meson setup` fail. The patch prints one warning and skips the bundle in that case; `ninja install` does not use the bundle. Written for OME (GPL-2.0-or-later, same as upstream file). | v11.1.1 | `docs/evidence/M1/custom-qemu-build.md` |
| `0002-virtio-gpu-refresh-rate-property.patch` | `include/hw/virtio/virtio-gpu.h`, `hw/display/virtio-gpu-base.c` | Adds the `refresh_rate` virtio-gpu property in mHz, validates 30000 through 240000 when non-zero, initializes EDID state from it and lets a non-zero UI backend rate override it. Zero keeps upstream behaviour. Written for OME (`virtio-gpu.h`: GPL-2.0; `virtio-gpu-base.c`: GPL-2.0-or-later, as stated by their headers). | v11.1.1 | `docs/evidence/M2/qemu-display-options.md` |
| `0003-ui-sdl2-swap-interval-option.patch` | `qapi/ui.json`, `ui/sdl2.c` | Adds `-display sdl,swap-interval=<-1\|0\|1>` for adaptive vsync, immediate updates or vsync. The default remains 0; refused values produce a warning, and refused adaptive vsync falls back to 0. Written for OME (`ui/sdl2.c`: MIT; headerless `qapi/ui.json`: GPL-2.0-or-later under QEMU `LICENSE`). | v11.1.1 | `docs/evidence/M2/qemu-display-options.md` |
| `0004-ui-sdl2-activate-on-click-option.patch` | `qapi/ui.json`, `ui/sdl2.c` | Adds `-display sdl,activate-on-click=off`. On Windows the opted-in SDL window answers `WM_MOUSEACTIVATE` with `MA_NOACTIVATE`, so an owned popup keeps host keyboard focus while still receiving the click; other platforms accept and ignore the option. Written for OME (`ui/sdl2.c`: MIT; headerless `qapi/ui.json`: GPL-2.0-or-later under QEMU `LICENSE`). | v11.1.1 | `docs/evidence/M2/embedded-display-freeze.md` |
| `0005-ui-sdl2-owner-window-and-qmp-geometry.patch` | `qapi/ui.json`, `ui/sdl2.c` | Adds the Windows-only `owner-window` SDL option and `x-ome-display-window` QMP command. The SDL frontend creates a hidden borderless owned popup before its GL context exists, suppresses guest-driven window resizing, and handles host geometry and visibility requests through SDL. Written for OME (`ui/sdl2.c`: MIT; headerless `qapi/ui.json`: GPL-2.0-or-later under QEMU `LICENSE`). | v11.1.1 | `docs/evidence/M2/embedded-display-freeze.md` |
| `0006-ui-sdl2-owner-window-keeps-guest-resolution.patch` | `ui/sdl2.c` | With `owner-window` set, the window is a plain viewport of the guest: a window resize is no longer reported to the guest as the preferred display mode (`qemu_console_set_ui_info`), and mouse coordinates are mapped through the renderer's placement of the surface (letterboxed and centred for GL as in `surface_gl_setup_viewport`, stretched for the 2D renderer) before they are scaled to the surface. The host sizes that window to its stage and the guest resolution is a product setting (`wm size`, kernel `video=`); the stock behaviour made the guest fall back to the stage size whenever the window was placed again and, once the sizes differed, put the guest pointer above the host cursor (user reports 2026-10-01). Without an owner window QEMU behaves as before. Written for OME (`ui/sdl2.c`: MIT). | v11.1.1 | `docs/adr/0009-guest-window-is-an-owned-popup.md` 결과 절 |

An empty patch directory is also supported.

Add reviewed patches as `0001-description.patch`, `0002-description.patch`, etc.
The clone step processes `*.patch` in lexical order, checks before applying, and
stages changes in the external QEMU checkout's index. It does not commit to OME.
The source offer archives that index tree, including modified and new files;
archiving HEAD alone would omit applied patches.

Each future patch must identify upstream source, author/license, purpose,
applicable QEMU pin and test evidence. No unofficial Windows/virgl patch is
presumed necessary. The fetched [MSYS2 PKGBUILD][pkgbuild] did not apply an active
Windows patch in its prepare function.

After replacing/removing/reordering patches, inspect local source work before
using `Build-Qemu.ps1 -Clean -IncludeSource`. Do not retain an old patched tree
and claim it matches a changed patch list.

[pkgbuild]: https://raw.githubusercontent.com/msys2/MINGW-packages/refs/heads/master/mingw-w64-qemu/PKGBUILD
