# M0 environment

Recorded 2026-09-25. Update when any row changes.

## Host

| Item | Value |
|---|---|
| CPU | Intel Core i5-12600KF (10C/16T) |
| RAM | 64 GB |
| GPU | NVIDIA GeForce RTX 2080 SUPER, driver 32.0.16.1664 |
| OS | Windows 11 Education 10.0.22621 |
| Hypervisor | Hyper-V enabled and running. `HypervisorPlatform` (WHPX) was disabled at the start; enablement with user consent is recorded in `whpx-enable.log` |
| Tools | PowerShell 7.6.6, adb 37.0.1, NDK 27.2 / 28.2, build-tools 35 / 36, JDK 17 |

## Harness

Distribution QEMU for Windows (winget `SoftwareFreedomConservancy.QEMU` 11.1.0),
`-accel whpx`, standard VGA with software rendering. See `launcher/Start-Guest.ps1 -Gpu std`.
Chosen over VirtualBox because Hyper-V is active on this host, which forces
VirtualBox onto the same WHPX path, and because the QEMU path carries over to M1.

## Guest image

| Item | Value |
|---|---|
| File | `Bliss-v16.9.7-x86_64-OFFICIAL-gapps-20241011.iso` |
| Source | SourceForge `blissos-x86`, `Official/BlissOS16/Gapps/Generic/` |
| Size | 2,429,550,592 bytes |
| SHA-256 (published `.iso.sha256`) | `17137711fb42236640ac6fe4421fcb3bd15710065fcf75c20807c018f42e3751` |
| SHA-256 (computed locally, 2026-09-25, `sha256sum`) | `17137711fb42236640ac6fe4421fcb3bd15710065fcf75c20807c018f42e3751` (match) |

## Evidence files in this folder

- `environment.md` (this file)
- `whpx-enable.log` (R9 consent record and command output)
- `metrics.md` (M0 step 7, pending)
- `bridge-props.txt`, `cpuinfo.txt` (M0 step 4, pending)
- Screenshots per scenario step (M0 step 6, pending)
