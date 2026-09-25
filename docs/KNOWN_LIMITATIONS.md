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
- Verified GPU and driver combinations: none yet. See `docs/evidence/M1/`.
- Multi-instance, macros, and scripted automation are not part of v1 (D9).
