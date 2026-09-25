# Open Mobile Emulator

A Windows emulator for arm64-only mobile games, assembled only from parts whose
provenance is clear: a custom QEMU accelerated by the Windows Hypervisor
Platform, an x86_64 Bliss OS guest, the ARM native bridge that ships inside that
guest, and a thin Windows front end.

**Status:** pre-alpha. M0 (translation check) and M1 (Windows execution path) are
in progress. Nothing here is usable by end users yet.

## Layout

| Path | What it holds |
|---|---|
| `CLAUDE.md` | The engineering specification: decisions, legal rules turned into CI, milestones, evidence requirements |
| `manifests/` | External artifact URLs with SHA-256, guest device profile |
| `launcher/` | PowerShell 7 scripts that check the host, fetch artifacts, create the guest disk, start QEMU, run guest tests |
| `qemu-build/` | MSYS2 build scripts, pins, and patches for the custom QEMU, plus the GPL source-offer bundler |
| `guest/` | Boot-argument notes and the guest overlay (Apache-2.0, see NOTICE) |
| `translator/` | Contract that makes the ARM translator a replaceable part |
| `compat/` | Per-game compatibility entries |
| `ci/` | Forbidden-pattern list, allowlist, and the checks that enforce them |
| `docs/` | Network endpoints, known limitations, evidence per milestone |
| `tests/` | Self-built fixtures and script tests |

## Quick start (developers, Windows 11)

```powershell
pwsh -File ci/Install-Hooks.ps1             # once per clone: pre-commit runs the R1 and R5 checks
pwsh -File launcher/Check-Host.ps1          # reports host readiness, changes nothing
pwsh -File launcher/Get-Artifacts.ps1       # downloads and verifies the guest ISO
pwsh -File launcher/New-GuestDisk.ps1       # creates the qcow2 and boots the installer
pwsh -File launcher/Start-Guest.ps1         # starts the guest
pwsh -File launcher/Invoke-GuestTest.ps1    # collects boot, bridge, and app evidence
```

Data lives under `%LOCALAPPDATA%\OpenMobileEmulator` (override with `OME_HOME`).

## License

GPL-2.0-or-later for the host code. Guest overlay files derived from the Android
Open Source Project stay Apache-2.0. See `LICENSE`, `NOTICE`, and `THIRD_PARTY.md`.
The guest image, Google services, and the ARM translator are third-party
binaries that the installer downloads from their publishers; this repository
never contains or redistributes them.

"Android" is a trademark of Google LLC. This project is not affiliated with
Google, Bliss Labs, or any game publisher.
