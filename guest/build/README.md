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
