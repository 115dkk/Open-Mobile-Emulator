# OME custom QEMU build (M1 preparation)

This builds a separate QEMU process, not a library linked into OME. The target is
WHPX with SDL2/OpenGL/virgl and user-mode networking. No Windows virtualization
feature, boot configuration, or guest image is changed by these scripts.

**UNVERIFIED:** The build, DLL bundle, temporary-drive workaround and guest boot
have not been executed. This is M1 step 1 preparation, not M1 completion.
The package-query gate failed because the concurrently installed MSYS2 keyring
was not ready. Package names below were verified on packages.msys2.org instead.

## Prerequisites and build

- Windows with PowerShell 7; a completed, initialized MSYS2 installation in
  `C:\msys64`. Finish MSYS2's normal setup/update procedure separately. Do not
  build against a partially upgraded package database.
- UCRT64, as selected by the [MSYS2 launcher][terminals]. Exact dependencies and
  per-package source URLs are listed in `pins.env`; no global software is installed
  while preparing this repository. **Running the build yourself does install
  the listed packages** using `pacman -S --needed --noconfirm`.
- Space for QEMU, its recursive firmware sources, the compiler and source-offer
  staging. Staging is retained for inspection until explicit `-Clean`.
- Development network access for MSYS2 packages, upstream QEMU, recursive source
  submodules and three pinned Meson wrap projects. These are development fetches,
  not new installer endpoints. Do not ship these as product download code.

From the repository root, run the following yourself after setup is complete.

```powershell
pwsh -File .\qemu-build\Build-Qemu.ps1
```

The wrapper runs `msys2_shell.cmd -ucrt64 -defterm -no-start -here -c
"./build-qemu.sh <step>"`, streaming output and propagating the exit code.
Flags were checked in the installed launcher and [MSYS2 documentation][terminals].

[QEMU configure][configure] rejects spaces in source/build paths, and Meson
resolves a SUBST drive letter back to the real path (its `subprojects download`
then fails with "path is on mount 'C:', start on mount 'Z:'"). The wrapper
therefore keeps the work tree (`src`, `out`, `dist`) outside the repository
when the repository path contains whitespace: by default under
`%LOCALAPPDATA%\OpenMobileEmulator\qemu-build`, overridable with `-WorkRoot`
or the `OME_QEMU_BUILD_WORK` environment variable. After a run it creates a
junction `qemu-build\out` pointing at the work tree's `out\` so the launcher
finds `qemu-build\out\bin\qemu-system-x86_64.exe`. Do not change the work
tree path under an existing Ninja build; use `-Clean` first. For manual UCRT64
use, export `OME_QEMU_BUILD_WORK` yourself and set `OME_REPOSITORY_ROOT` to the
repository root when generating third-party notices.

Steps can be run separately with `-Step deps|clone|configure|build|dist`.
Each requires previous step outputs. `all` also generates the third-party
candidate and source offer. Already installed packages and existing clones are
reused; patch reverse checks avoid double application; matching configuration is
reused; Ninja is incremental. Packaging replaces generated runtime trees so stale
DLLs do not survive. Do not run multiple builders against the same tree.

`-Clean` explicitly removes only `out/` and `dist/`. Source is preserved unless
`-Clean -IncludeSource` is supplied. Inspect local source edits before using the
latter. `-WhatIf` does not launch a build. No OME Git add/commit/push is performed.

## Pins and configure policy

The [download page][download] and [upstream tags][tags] showed newest stable
`v11.1.1`, commit `c3d48b7d1e89604920e5b81b91140c2ad39a1943` at research time.
The build verifies the checked-out tag against that hash; it fails rather than
silently accepting a changed tag.

UCRT64 ships [virglrenderer][virgl] and [libepoxy][epoxy]. A source-built virgl
fallback is therefore not added. MSYS2 versions are **recorded, not frozen**:
`out/pacman-lock.txt` includes the entire installed package set, while
`out/pacman-requested.txt` lists explicit dependencies. Rebuilding later from
rolling repositories is not a promise of identical binaries.

The [MSYS2 PKGBUILD][pkgbuild] uses `--prefix`, `--bindir=bin`,
`--datadir=share/qemu`, and `--disable-download`; those choices are retained.
Its optional target-list example is `x86_64-softmmu`. It has no explicit
`--enable-gtk`, so this build disables GTK and requires SDL instead. The other
feature choices come from the [tagged Meson options][options] and
[build-system option mapping][build-system], not invented PKGBUILD flags.

Required features are `--enable-whpx --enable-virglrenderer --enable-opengl
--enable-sdl --enable-slirp --enable-dsound --enable-tools --enable-install-blobs`.
Documentation, warnings-as-errors, GTK, SDL-image, modules and Rust are disabled.
The [tagged Meson build][meson] selects `sdl2` and auto-detects DirectSound on
Windows; `--enable-dsound` makes missing support fatal. The old
`--audio-drv-list` does not occur in the [tagged configure script][configure].
These source checks do not prove successful audio/GL operation on this GPU.

The clone step gets pinned wrap sources for keycodemapdb, SoftFloat and TestFloat
before configuring with downloads disabled. Revisions come from the fetched
upstream [keycodemapdb][keycode], [SoftFloat][softfloat] and [TestFloat][testfloat]
wraps. All recursive [Git submodules][submodules] are retained for firmware source.
No unreviewed Windows patch or secondary binary distributor is used.

## Outputs

| Location under qemu-build | Contents |
|---|---|
| `src/qemu/` | External upstream checkout, staged local patches, pinned subprojects/submodules |
| `out/build/`, `out/install/` | Ninja build and installation prefix |
| `out/bin/` | Symlink-free launcher copies of both EXEs and resolved UCRT64 DLLs |
| `out/share/qemu/` | Firmware, keymaps and installed QEMU data beside the launcher bin directory |
| `dist/qemu/bin/`, `dist/qemu/share/qemu/` | Same distributable runtime layout |
| `dist/qemu/dll-origins.tsv`, `dll-packages.txt` | DLL paths and pacman ownership inventory |
| `out/logs/` | Appended logs for each build/packaging step |
| `out/THIRD_PARTY.generated.md` | Full candidate document with only the generated block replaced |
| `dist/qemu-source-offer-<tag>-<shortcommit>.tar.gz` | Exact patched QEMU source, subprojects, firmware sources, scripts and build/package records |
| `dist/SHA256SUMS` | Source archive and runtime file checksums |

`edk2-x86_64-code.fd`, `edk2-i386-vars.fd` and `keymaps` are required outputs,
verified against [upstream data installation][pcbios]. Additional installed data
is retained so VGA/NIC ROMs are not accidentally omitted. Only dependencies
resolved under `/ucrt64/bin` are copied from MSYS2; Windows system DLLs stay on the
host. Unexpected dependency roots or unresolved DLLs stop packaging.

**UNVERIFIED:** `ldd` covers linked dependencies, not every optional runtime
`LoadLibrary` plugin. A clean-machine SDL/OpenGL/virgl/audio/firmware test is still
required. Capability-list checks are not a VM boot test. License notices and
corresponding source for shipped MSYS2 DLLs must be reviewed before release.

## Source offer and third-party generation

In the same UCRT64 path/environment, after a successful build and dist step:

```bash
bash ./make-third-party.sh
bash ./make-source-offer.sh
```

The offer archives the exact Git **index tree**, not bare HEAD: patches were
applied with `git apply --index`. It includes new/modified files, all patch files
(including an empty directory), `common.sh`, both wrappers, pins, lock files and
source metadata. It checks build inputs, installed packages, wrap contents,
submodules and runtime checksums before claiming correspondence. Unlike the
[upstream archive helper][archive], this packaging step never fetches sources.

The offer contains firmware sources plus the upstream prebuilt firmware shipped
with QEMU. **UNVERIFIED:** Reproduction of those prebuilt firmware bytes from the
recorded source pins has not been demonstrated. The source statement covers the
QEMU executables, not a blanket license-compliance assertion for the DLL bundle.
The release owner must also supply notices/source/relinking material required
by those independent licenses; the scripts do not design or modify CI.

`make-third-party.sh` asks `pacman -Qi` for each copied DLL's owning package,
extracting name, version, licenses and upstream URL. It preserves text outside
the marker pair, writes only `out/THIRD_PARTY.generated.md`, and prints a diff.
It never modifies the root `THIRD_PARTY.md`. The caller reviews/merges that diff.

## Sources consulted (2026-09-25)

Build sources are linked below. Every package-page URL is listed next to the
package declaration in `pins.env`; those comments are part of this source list.
Bliss research and its complete page list are in `guest/kernel-cmdline.md` and
`docs/GUEST_INSTALL.md`. The docs index used to find them was
https://docs.blissos.org/ . The package discovery search was
https://packages.msys2.org/search?q=virglrenderer&t=pkg and the QEMU dependency
cross-check was https://packages.msys2.org/package/mingw-w64-ucrt-x86_64-qemu .

[download]: https://www.qemu.org/download/#source
[tags]: https://gitlab.com/qemu-project/qemu/-/tags
[pkgbuild]: https://raw.githubusercontent.com/msys2/MINGW-packages/refs/heads/master/mingw-w64-qemu/PKGBUILD
[build-system]: https://www.qemu.org/docs/master/devel/build-system.html
[configure]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/configure
[options]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/meson_options.txt
[meson]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/meson.build
[pcbios]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/pc-bios/meson.build
[archive]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/scripts/archive-source.sh
[submodules]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/.gitmodules
[keycode]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/subprojects/keycodemapdb.wrap
[softfloat]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/subprojects/berkeley-softfloat-3.wrap
[testfloat]: https://gitlab.com/qemu-project/qemu/-/raw/v11.1.1/subprojects/berkeley-testfloat-3.wrap
[virgl]: https://packages.msys2.org/package/mingw-w64-ucrt-x86_64-virglrenderer
[epoxy]: https://packages.msys2.org/package/mingw-w64-ucrt-x86_64-libepoxy
[terminals]: https://www.msys2.org/docs/terminals/
