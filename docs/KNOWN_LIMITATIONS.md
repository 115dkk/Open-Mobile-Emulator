# Known limitations (draft, finalised at M3)

- The ARM translator is a proprietary Google binary (`libndk_translation`) that
  ships inside the Bliss OS image. This project did not write it and cannot fix
  it. Replacing it with an open-source translator is a post-release task (P2).
- GPU acceleration is OpenGL only, through virglrenderer. Games that require
  Vulkan are unsupported until the gfxstream path exists (P1).
- The guest is a Google-uncertified device. Play Store and in-app purchases are
  not guaranteed even after the uncertified-device registration described in
  `docs/GOOGLE_ACCOUNT.md`.
- Enabling the Windows Hypervisor Platform runs Windows on top of Hyper-V. Games
  with kernel anti-cheat on the same PC may refuse to start while it is on. The
  product never changes this setting by itself (R9).
- Games that detect and block emulators are out of scope. No evasion is
  implemented (R7).
- A guest-initiated reboot does not work under WHPX with QEMU 11.1.0: QEMU logs
  `failed to get xsave state` for every vCPU and then `WHPX: Unexpected VP exit
  code 4`, and the VM stays paused with QMP unreachable. Seen with `-cpu max` and
  `-cpu Skylake-Client-v4` on 2026-09-25 (`docs/evidence/M0/guest-install.md`).
  The launcher therefore starts QEMU with `-action reboot=shutdown`: a reboot
  inside Android ends the QEMU process, and it has to be started again
  (`launcher/Start-Guest.ps1`; the M2 supervisor will do this automatically).
  QMP `system_reset` has the same problem, so it is not used either.
- `-cpu max` must not be used with WHPX. QEMU answers the guest's CPUID from its
  own model there, and `max` resolves to the TCG definition (AMD vendor, TCG
  feature set); Bliss 16.9.7 hung after its init banner with it. The launcher
  defaults to `Skylake-Client-v4`.
- Verified GPU and driver combinations: NVIDIA GeForce RTX 2080 SUPER, driver
  32.0.16.1664, Windows 11 22621, distribution QEMU 11.1.0, virgl through
  `-device virtio-vga-gl -display sdl,gl=on` (2026-09-25, `docs/evidence/M1/whpx-boot.md`).
  Not verified: any Intel or AMD GPU (the development host has no integrated GPU),
  the GTK display, and the custom QEMU build.
- With a virgl (GL) scanout, QMP `screendump` answers `no surface` in QEMU 11.1.0;
  guest screenshots come from `adb screencap` until a host-side capture exists.
- Multi-instance, macros, and scripted automation are not part of v1 (D9).
