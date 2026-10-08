# Self-built guest image (P3)

Builds the blob-free Android 15 x86_64 guest from the BlissOS 18 tree (`voyager-x86`). The plan,
the facts behind it and what is still open are in `docs/P3-PLAN.md` and
`docs/evidence/P3/research-*.md`. The rules are CLAUDE.md R1 and R3: no proprietary blob is ever
built in, committed or shipped; the native bridge and Google apps stay out (`OME_BLOB_FLAGS` in
`pins.env` are unset and refused).

| File | Role |
|---|---|
| `pins.env` | Every input the build consumes: manifest URL, branch and commit, lunch target, variant, the installer-environment release, the build-host rootfs and its SHA-256 |
| `manifest/ome.xml` | `repo manifest -r` snapshot written by `build-image.sh snapshot`: every project at the commit that was built. Later builds start from it |
| `aaropa.sha256` | Checksums of the installer-environment files (`install.sfs` and friends) recorded on the first `build-image.sh aaropa` |
| `setup-build-host.sh` | Prepares an Ubuntu 22.04 host as root: packages from the Bliss README (meson from PyPI instead, pinned in `pins.env`, because the distro's 0.61.2 is below Mesa's 1.1.0), repo launcher, build user `ome`, rustup |
| `build-image.sh` | `init`, `sync`, `snapshot`, `aaropa`, `build`, `collect` or `all` |
| `Dockerfile` | The same host as a container (`ubuntu:22.04` + `setup-build-host.sh`) |
| `New-BuildDistro.ps1` | Windows PC: downloads and verifies the rootfs, imports the WSL 2 distro `OME-Build`, runs the setup |

## Requirements

250 GB free on the drive that holds the build (source about 100 GB with the shallow history,
`out/` about 150 GB), 24 GB RAM, four to six hours on 16 threads. The sync downloads on the order
of 100 GB.

The README's 24 GB is not enough for the parallel build on 16 threads: in a 31 GB WSL VM with
8 GB swap, `java` (r8) invoked the OOM killer at 23 % and ninja was killed (2026-10-02). On the
development PC the VM gets 48 GB and 16 GB swap through `%USERPROFILE%\.wslconfig`
(`[wsl2] memory=48GB`, `swap=16GB`, applied by `wsl --shutdown`) and the build runs with
`OME_JOBS=12`.

## On the development PC (WSL 2)

```powershell
pwsh -File guest/build/New-BuildDistro.ps1 -InstallPath C:\WSL\OME-Build
wsl -d OME-Build -- bash -lc 'OME_REPO_ROOT="/mnt/c/Open Mobile Emulator" "/mnt/c/Open Mobile Emulator/guest/build/build-image.sh" all'
```

The tree lives inside the distro at `~/bliss` (ext4, fast); the repository stays on the Windows
drive and is only read. Long stages (sync, build) must be started from a `wsl.exe` process that
stays alive on the Windows side: WSL stops a distro a few seconds after its last `wsl.exe` session
ends, even with systemd on and a transient service still running (seen 2026-10-01: a `systemd-run`
sync died 15 s after the launching session closed). A hidden `Start-Process wsl.exe -d OME-Build -u
ome -- bash /home/ome/run-sync.sh` keeps the session, and the stage script appends to
`~/bliss/ome-logs/` with an exit marker line to poll for. The ISO, its `.sha256`, `build.prop`, the manifest snapshot and the build
log land in `~/bliss/dist/<date>-<manifest commit>/`.

Before `lunch`, the build stage writes `vendor/extra/product.mk` (BlissOS's property override hook;
it turns adb on with `persist.sys.usb.config=adb`) and applies `patches/<project path>/*.patch` to
the synced tree. A patch that already applies in reverse is skipped, and one that applies neither
way stops the build. Since the 2026-10-08 user decision, the published S26 Ultra identity
in `manifests/device-profile.prop` is validated and copied into `/system/etc/ome-device-profile.prop`.
`system/core/0002-load-ome-device-profile.patch` applies its model and fingerprint after vendor overrides.
The donor brand/product/device remain provenance metadata; native partition identities are kept. It leaves SDK, ABI, security patch level and boot settings intact.
The earlier patches keep competing Bliss hooks from substituting other device identities or
changing unrelated behavior. They are implementation choices, not a ban on compatibility profiles:

| Patch | What it restores |
|---|---|
| `system/core/0001-init-keep-real-build-properties.patch` | init no longer runs `workaround_snet_properties`, which rewrote the build type, `ro.debuggable` and the verified-boot state to a locked retail device's values at every boot |
| `build/make/0001-sysprop-keep-real-build-keys.patch` | upstream's `BUILD_KEYS` choice, so the fingerprint of a test-key build says `test-keys` instead of `release-keys` |
| `frameworks/base/0001-keep-real-device-identity.patch` | upstream AOSP at every call into `PixelPropsUtils`, `AttestationHooks`, `GamesPropsUtils` and `HideDeveloperStatusUtils`, and upstream `hasSystemFeature`, `getInstallerPackageName` and `RuntimeInit`: apps see the real build and device values, Pixel features are not claimed, key attestation is not blocked |
| `packages/apps/Settings/0001-drop-hide-developer-status-entry.patch` | drops the Settings entry for hiding developer status, which does nothing once the framework hook is gone |

The BlissOS utility classes stay in the tree; nothing calls them. See
`docs/DEVICE_PROFILE.md` for profile provenance, existing-guest migration and live test status.

An incremental rebuild in the same tree removes `system/vendor/firmware` and the kernel image
first: the kernel rule's `copy-firmware.sh` stops at links an earlier build left there.

## Experimental Digitalis module probe (API 35)

The default remains `OME_TRANSLATOR=none`. `modules` builds only the translator modules;
`digitalis-iso` explicitly builds and collects an experimental image as described below.
`manifest/ome-digitalis.xml` pins only the translator fork. API 35 `native_bridge_support`
and bionic keep their base commits with the optional compatibility patches applied during
the probe; the goldfish product and GPU configuration stay unchanged. Do not import the upstream
manifest's agent configuration/linkfiles or sample applications.

On a clean translator checkout, save its revision and git metadata, copy the optional manifest
to `<tree>/.repo/local_manifests/ome-digitalis.xml`, then sync only
`frameworks/libs/binary_translation`. Changing the project's remote identity requires repo's
`--force-sync`; do not run it on an uncommitted checkout. The original object database must remain.

```sh
OME_TRANSLATOR=digitalis OME_BUILD_ROOT=/home/ome/bliss OME_JOBS=6 \
    bash '/mnt/c/Open Mobile Emulator/guest/build/build-image.sh' modules
```

The probe applies only `patches/digitalis/<project path>/*.patch`, suppresses the Bliss
vendorsetup installer's `releases/latest` download, and runs `m libberberis_arm64`. Logs go to
`<tree>/ome-logs/digitalis-modules-*.log`. The regular patch pass excludes this directory.
The compatibility patch replaces the A16-only `ndk_translation_package` dependency aggregators
with A15's `phony_rule` and `.native_bridge` dependency names; it does not change runtime code.

The API 35 bridge and the complete `BERBERIS_PRODUCT_PACKAGES_ARM64_TO_X86_64` module list
**built successfully on 2026-10-05**. Set `OME_DIGITALIS_MODULES=all` alongside
`OME_TRANSLATOR=digitalis` to build that list. The probe uses the upstream enable makefile,
adds the ARM64 guest board target, and leaves the GPU configuration unchanged. The libm patch
uses AOSP's `expf`/`powf` proxy stubs, API 35 bionic sources and version maps, and a separate
API 35 optimized-math archive whose two proxied entry points come from the stubs instead.
The normal host libm keeps its original implementation. A Python generator patch accommodates
A15's embedded Python 3.11. No API 36 bionic binaries are used.

All 74 upstream distribution paths exist. ELF machine types, `NativeBridgeItf`, libm exports,
and DT_NEEDED were checked (`docs/evidence/P2/api35-elf-validation.txt`). This is not a boot or
runtime test. Module builds leave the old image `build.prop` unchanged. The latest full-module
build is `docs/evidence/P2/module-attempt-12.txt`. A separate experimental ISO build and live
ARM64 smoke test subsequently passed on 2026-10-05 (see below).

In the existing incremental tree, the generated Soong graph once stayed older than the changed
product variables, causing false missing-module errors. Moving only the generated
`out/soong/build.bliss_x86_64.ninja` aside forced its regeneration and resolved that failure.
Do not remove product modules to hide it.

`init`, `sync`, `snapshot`, `all`, `build`, and `collect` refuse the experimental translator
setting, its installed local manifest, or an installed Digitalis bridge in the shared product
output. To return to baseline sources, reverse only this probe's patches in reverse order,
restore the saved `vendor/extra/product.mk`, move its local manifest out of
`.repo/local_manifests`, and sync only the translator back to `ome.xml`'s revision. Keep the
previous `dist/` intact. The 2026-10-05 probe was restored this way.

**Source restoration does not clean the product output.** After saving the experimental image
and restoring the baseline sources, run `build-image.sh installclean`, then
`build-image.sh baseline-check`. The clean stage calls Soong's existing `installclean` target
without envsetup's download hook: installed system/root/images and packaging are removed, but
object caches and `dist/` remain. This procedure passed after the 2026-10-05 live smoke test;
a full baseline ISO rebuild was not run.

### Experimental ISO and live smoke

With the optional manifest synced and `OME_TRANSLATOR=digitalis`, run:

```sh
OME_TRANSLATOR=digitalis OME_BUILD_ROOT=/home/ome/bliss OME_JOBS=8 \
    bash '/mnt/c/Open Mobile Emulator/guest/build/build-image.sh' digitalis-iso
```

Use an optional `OME_DIGITALIS_RUN=jni1` (letters, digits and hyphens only) to distinguish
multiple experiments on one day. It becomes part of both the ISO name and the dist directory;
existing experiment directories are still never overwritten.

The command applies normal OME identity patches and Digitalis compatibility patches, verifies
the pinned installer cache, builds `iso_img`, renames the successful output to
`OME-api35-<date>-digitalis.iso`, and collects it in
`dist/<date>-<manifest commit>-digitalis/`. It refuses an existing collection directory and
copies the exact repo manifest, compatibility patches, pins, build properties, and build log.
Neither the baseline manifest snapshot nor existing release assets are overwritten.

The 2026-10-05 ISO contains the enabled bridge, ARM64 ABI properties, binfmt handlers and the
stub-bearing API 35 libm. Product QEMU live boot on separate adb/QMP ports passed static ARM64
hello and the ARM64 probe APK. Evidence is `docs/evidence/P2/api35-iso-inspection.txt`,
`api35-live-smoke.txt`, `api35-probe-ui-unlocked.txt`, and `api35-probe-unlocked.png`.
The test VM was shut down normally. No game test was performed.

R1 name scanning finds 24 `libberberis*` files in the ISO (the 23 canonical x86-64 files plus
32-bit `libberberis_exec_region.so`). These were source-built; no filename exceptions or
release approval were added. The experimental image is not a published release.

## Release assets (ADR-0012)

`make-notice-guest.sh <tree> <dist> <out>` mounts the ISO, its `system.efs` and the `system.img`
inside it read-only, writes the file list and the R1 forbidden-name scan (`forbidden-scan.txt`,
exit code 3 when something matches), copies the image's own `NOTICE.xml.gz`, and generates
`NOTICE-guest.md` from the manifest snapshot and the license marker at each project root
(`notice_guest.py`).

`make-kernel-source.sh <tree> <out>` checks that `kernel/x86/common` sits at the commit the
manifest pins, archives that commit (`git archive`, gzip) and copies the `.config` the build used.

`make-release-assets.sh <tree> <dist> <out>` runs both, splits the ISO into 1 GiB parts
(`<name>.iso.part0`, `.part1`, ...; GitHub caps one release asset at 2 GiB), collects the build
inputs and logs, and writes `SHA256SUMS` and `release-assets.json` (the values to copy into
`manifests/artifacts.json`). Example:

```sh
guest/build/make-release-assets.sh ~/bliss ~/bliss/dist/20261002-db973c9 ~/bliss/dist/20261002-db973c9/release
```

The upload itself is `gh release create --prerelease` from the development PC with every file in
`<out>`; ADR-0012 fixes the release name (`guest-android-15-<date>`) and what the release has to
carry in place of a build provenance attestation.

## In a container or on a Linux VM

```sh
docker build -t ome-guest-build guest/build
docker run --rm -it -v ome-bliss:/build -v "$PWD:/repo:ro" ome-guest-build \
    env OME_REPO_ROOT=/repo OME_BUILD_ROOT=/build/bliss /repo/guest/build/build-image.sh all
```

## What is not pinned yet

- The kernel project (`kernel/x86/common`, branch `hm/crimson`) and every other project are
  pinned only once `manifest/ome.xml` exists. Until the first snapshot, `init` uses the manifest
  repository at `BLISS_MANIFEST_COMMIT` and the project revisions that manifest names (branches).
- Where release builds run, and how their provenance is attested, is a P3 decision still to be
  recorded (docs/P3-PLAN.md section 5). Builds on the development PC are for development and
  evidence.
