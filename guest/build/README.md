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
| `setup-build-host.sh` | Prepares an Ubuntu 22.04 host as root: packages from the Bliss README, repo launcher, build user `ome`, rustup |
| `build-image.sh` | `init`, `sync`, `snapshot`, `aaropa`, `build`, `collect` or `all` |
| `Dockerfile` | The same host as a container (`ubuntu:22.04` + `setup-build-host.sh`) |
| `New-BuildDistro.ps1` | Windows PC: downloads and verifies the rootfs, imports the WSL 2 distro `OME-Build`, runs the setup |

## Requirements

250 GB free on the drive that holds the build (source about 100 GB with the shallow history,
`out/` about 150 GB), 24 GB RAM, four to six hours on 16 threads. The sync downloads on the order
of 100 GB.

## On the development PC (WSL 2)

```powershell
pwsh -File guest/build/New-BuildDistro.ps1 -InstallPath C:\WSL\OME-Build
wsl -d OME-Build -- bash -lc 'OME_REPO_ROOT="/mnt/c/Open Mobile Emulator" "/mnt/c/Open Mobile Emulator/guest/build/build-image.sh" all'
```

The tree lives inside the distro at `~/bliss` (ext4, fast); the repository stays on the Windows
drive and is only read. The ISO, its `.sha256`, `build.prop`, the manifest snapshot and the build
log land in `~/bliss/dist/<date>-<manifest commit>/`.

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
