# Known limitations

Final for release 0.1.1 (2026-10-02). Items are added as they are found and removed
only when a release fixes them.

- The release is not code-signed (CLAUDE.md section 8 item 3: no certificate until a
  sponsor pays for one). Windows SmartScreen warns once when the downloaded
  installer is opened; `docs/help/install.md` explains how to pass it and how to
  check the file instead, by SHA-256 and by the build provenance attestation that
  every release file carries (`gh attestation verify`, ADR-0007).
- The installer installs for the current user only, under
  `%LOCALAPPDATA%\Open Mobile Emulator`, without administrator rights (ADR-0011).
  There is no all-users option. Uninstalling removes the program folder and leaves
  the data folder `%LOCALAPPDATA%\OpenMobileEmulator` (guest disks, logs,
  settings) in place.
- 0.1.0 and 0.1.1 are pre-releases. The product's update check reads GitHub's `latest`
  release, which never points at a pre-release, so no update is offered between
  pre-releases; the first regular release will be.
- Security software that controls outbound connections per program (seen with
  AhnLab V3 365 Clinic on the development host, 2026-10-01,
  `docs/evidence/M3/guest-network-by-binary-path.txt`) can silently block the
  guest's internet: `qemu-system-x86_64.exe` in the install folder is a program
  that software has never seen. The guest still boots, adb works and ICMP goes
  through, but DNS and outbound TCP from the guest fail, so a game reports that
  it cannot reach its server. Allow `%LOCALAPPDATA%\Open Mobile Emulator\qemu\bin\qemu-system-x86_64.exe`
  in that software; the product cannot do it for you (R9). The same copy of
  QEMU at an already allowed path worked, so this is not a defect of the
  bundled runtime.
- Windows 10 is untested. Development and the completion runs used Windows 11
  22621 and the Windows Server 2025 hosted runner.
- A guest installed by the interactive Bliss installer (before ADR-0010, that is
  before the product's unattended install of 2026-10-01) used to boot through the
  firmware and GRUB at every start. Since 0.1.1 the product moves such a guest to
  the direct kernel boot once, on its next start, taking the kernel and initrd from
  the verified ISO; if that first direct boot fails, the product goes back to the
  GRUB boot and says so in a notice. Reinstalling the guest from Settings also
  switches it.
- The ARM translator is a proprietary Google binary (`libndk_translation`) that
  ships inside the Bliss OS image. This project did not write it and cannot fix
  it. Replacing it with an open-source translator is a post-release task (P2).
  Google distributes the emulator images this translator comes from under the
  Android SDK License Agreement, which grants use "solely to develop
  applications for compatible implementations of Android" (section 3.1).
  Running games is outside that scope (`docs/adr/0006-guest-image-distribution.md`).
  Self-built guest images are released without Google files (R3); the first
  one is the Android 15 profile below.
- The Android 15 profile (`ome-android-15`, image release
  `guest-android-15-20261002`) is a candidate, shown as 검증 전. It is this
  project's own blob-free build of the BlissOS 18 tree (ADR-0012,
  `docs/evidence/P3/build-20261002.md`): it carries no ARM translator and no
  Google apps, so an ARM-only game does not run on it at all until the
  translator flow of CLAUDE.md section 8 item 9 or the open translator (P2)
  exists. It is for booting, probing and development today.
- That image is built on the development PC's WSL2, not on a GitHub runner, so
  its release files carry no build provenance attestation; `gh attestation
  verify` fails on them by design. The release carries the pinned build inputs
  (`pins.env`, `ome.xml`), `build.prop`, the build logs, the forbidden-pattern
  scan, `NOTICE-guest.md` and the kernel source tarball instead (ADR-0012
  items 1 and 6). Product executables and QEMU keep their attestations. The
  image is signed with AOSP test keys.
- GitHub limits one release asset to 2 GiB, so the 2.95 GB image is published
  as three 1 GiB parts (`.iso.part0` to `.part2`). The product downloads and
  joins them and checks each part and the whole file by SHA-256. A manual
  download has to concatenate the parts in order (`copy /b`) before the
  `.iso.sha256` line matches.
- adb on the Android 15 guest requires key authorization (`ro.adb.secure=1`).
  At install the product writes the host adb client's public key
  (`%USERPROFILE%\.android\adbkey.pub`, created by `adb start-server`) into
  the guest's system image as `/adb_keys`, so the first boot is authorized
  without a dialog. If that key file is deleted or the data folder is moved to
  another user, the guest shows `unauthorized` and has to be reinstalled from
  Settings. `adb root` is refused on this image by Bliss's root setting
  ("ADB Root access is disabled by system setting"), so the product's root
  toggle is absent for it until that setting is wired (docs/P3-PLAN.md).
- The Android 15 image contains GPL components (the kernel, busybox,
  e2fsprogs, alsa-utils and others). The release attaches the kernel source
  tarball; for everything else the source offer is the `ome.xml` snapshot,
  which pins every project to a commit in a public repository, listed with its
  license marker in `NOTICE-guest.md`. A full source archive of the tree (about
  100 GB) is not published for the pre-release.
- 32-bit ARM apps (`armeabi-v7a`, `armeabi`) run only through a proprietary
  translator. The open translator the project aims to make the default for
  64-bit ARM apps (Digitalis, P2) has an ARM64 backend only, and no open
  ARM32-to-x86 translator was found (2026-09-28). The Android 13 guest lists
  both 32-bit ABIs (`docs/evidence/M0/bridge-props.txt`), but no 32-bit-only
  app has been run on it yet (`docs/adr/0008-drop-pie-abi-based-translator-policy.md`).
- Android 9 (Pie) is not a planned guest profile. Commercial emulators keep it
  mainly for low-memory PCs, many instances, macros, and 32-bit apps; the first
  three are outside this product, and 32-bit apps are covered by the Android 13
  profile (ADR-0008).
- GPU acceleration is OpenGL only, through virglrenderer. Games that require
  Vulkan are unsupported until the gfxstream path exists (P1).
- The guest is a Google-uncertified device. Play Store and in-app purchases are
  not guaranteed even after the uncertified-device registration described in
  `docs/GOOGLE_ACCOUNT.md`. On 2026-09-26 the registration worked on the
  development host: the account was added, Google Play Games signed in, and the
  Play Store opened; purchases remain untested.
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
  The custom QEMU build (v11.1.1 with ANGLE bundled) booted the same guest on
  the same GPU on 2026-09-26 (`docs/evidence/M1/run-custom-qemu-boot/`).
  Not verified: any Intel or AMD GPU (the development host has no integrated GPU)
  and the GTK display.
- With a virgl (GL) scanout, QMP `screendump` answers `no surface` in QEMU 11.1.0;
  guest screenshots come from `adb screencap` until a host-side capture exists.
- In-app video (H.264 through Android's software codec) plays black on Bliss
  16.9.7: the guest's `media.swcodec` process aborts with `gralloc-mapper is
  missing` on the first decoded frame, under both the virgl and the
  software-rendering boot entries (2026-09-26, `docs/evidence/M0/swcodec-gralloc-abort-virgl.txt`).
  The tested game skips its intro on a tap and otherwise runs. Bliss's ffmpeg
  codec2 service (`FFMPEG_CODEC2_PREFER=1` on the kernel line; `init.sh` reads
  that name, not the shipped menu's `FFMPEG_PREFER_C2`) decodes without a
  crash but the frames still do not reach the game's texture, so the video
  stays black either way (`docs/evidence/M0/findings-20260926.md`).
- QEMU's generated EDID offers the guest a single 75 Hz mode, which pins
  SurfaceFlinger's vsync to 13.3 ms and held the tested game at 38 fps. The
  launcher passes `edid=off` on the virgl device so the guest falls back to a
  60 Hz mode (58 fps measured). With `edid=off` the guest still lists 35 DRM
  modes from 1280x800 to 4096x2160, all at 56 to 60 Hz (2026-09-27,
  `docs/evidence/M2/sizing-20260927/`); only the EDID's preferred mode and
  refresh rate are lost. The SDL display backend never reports a refresh rate
  to virtio-gpu (only GTK does), so a chosen refresh rate needs the OME patch
  `qemu-build/patches/0002-virtio-gpu-refresh-rate-property.patch` (device
  property `refresh_rate` in mHz), and host vsync needs
  `0003-ui-sdl2-swap-interval-option.patch`. With the patched build and
  `refresh_rate=120000` the guest boots at 1280x800 with a 119.997 Hz mode and
  an 8.33 ms VSYNC period (2026-09-27, `docs/evidence/M2/sizing-20260927/`);
  whether a game renders at that rate is untested.
- The translator aborted once with `ndk_translation: Cannot process signal 11`
  while a native thread attached to the JVM (`docs/evidence/M0/crash-ndk-translation-1/`),
  after 170 s of a launch that was otherwise idle on a download prompt. It did
  not recur across the following launches and a 37-minute session. This is
  inside the proprietary translator and cannot be fixed here; it is tracked as a
  stability risk until it is seen again or ruled out.
- On this uncertified guest without a Google account, a game that calls Google
  Play Games sign-in at start-up opens the system "Add account" page every
  launch; Back returns to the game, which then offers its own login methods.
- The tested game's attribution SDK (`airbridge.io`) could not complete a TLS
  handshake from the development host's network at all, guest or host. The game
  retried in the background and was not blocked by it. Product networking is
  unaffected (R10), but a host network that filters such domains will show the
  same retries in `logcat`.
- Bliss 16.9.7 boots with the media stream at index 5 of 15, which AudioFlinger
  applies as -33 dB. Game audio does reach the host through QEMU's `dsound`
  backend (measured on the host's audio session, 2026-09-26,
  `docs/evidence/M1/whpx-boot.md`), but at that index it arrives as near
  silence. Until the launcher or the M2 wizard sets the volume after the first
  boot, run `adb shell cmd media_session volume --stream 3 --set 15` once; the
  guest keeps the value across reboots.
- The translator presents ARM code with a two-processor `/proc/cpuinfo`
  (`/system/etc/cpuinfo.arm64.txt`), so Unity logs `Cores = 2` on a 4-vCPU
  guest. A four-processor view changed neither the game's thread count nor its
  frame rate (`docs/evidence/M1/cpuinfo-experiment.txt`), so the stock file stays
  and the log line is cosmetic.
- QEMU 11.1.1's SDL GL display calls `eglGetCurrentDisplay` through libepoxy,
  which needs `libEGL.dll` next to the executable or on `PATH`; without it QEMU
  crashes at start-up with `0xC0000005`. The custom build therefore bundles
  ANGLE (`libEGL.dll`, `libGLESv2.dll`, BSD-3-Clause, about 12 MB), as the
  distribution build does (`docs/evidence/M1/custom-qemu-build.md`). Do not
  strip those files from a release.
- App root does not work on Bliss 16.9.7 GApps 2024-10-11: the KernelSU manager
  app is installed but the kernel side is absent (`/data/adb/ksu`, `ksud` and
  `su` do not exist; Bliss support issue #99 reproduces). `adb root` works. The
  product's root toggle is therefore absent for this image profile
  (`docs/DECISION-root-adb.md`, `docs/evidence/M2/sizing-20260927/`).
- adb on this guest has no key authentication (`ro.adb.secure=0`): any program
  on the same PC that connects to `127.0.0.1:5555` gets a root shell. The
  product keeps the port on loopback and shows a warning in Settings; opening
  it to the network is an explicit opt-in.
- A 16-vCPU, 32 GiB guest boots in 25 s on the development host (2026-09-27,
  `docs/evidence/M2/sizing-20260927/`). The memory is committed as private
  memory of the QEMU process, so the product's limits come from the host
  (physical memory minus 4 GiB, logical processor count). More vCPUs do not
  raise a game's thread count (the translator reports two cores to ARM code).
- Keyboard input reaches the guest only while the product window has keyboard
  focus and the stage is on screen: the main webview captures its own key
  events and forwards them (`input_host_key`). There is no global keyboard
  hook. The first implementation used one, and on the development host it
  received nothing at all while the product window was the foreground window,
  although the same keys arrived in the webview (2026-09-29,
  `docs/evidence/M2/keyboard-capture.md`). While the stage is active, Enter and
  Space go to the guest as well, so the stage screen's buttons are for the
  mouse. Clicking the embedded guest does not move keyboard focus to it.
- The guest window is not a child of the main window. It is a borderless,
  non-activating top-level popup owned by the main window and placed over the
  stage (ADR-0009). Re-parenting failed on a 200 % monitor: a DPI-unaware
  child froze its presentation at random, and a per-monitor-v2 child was
  covered by WebView2's composition and lost SDL's size bookkeeping
  (2026-09-29, `docs/evidence/M2/embedded-display-freeze.md`). Consequences:
  QEMU resizes its window to the guest resolution at each mode switch and the
  product puts it back within one second, so the guest can flash at its own
  size briefly; other applications' windows can appear between the popup and
  the main window; the product starts QEMU with
  `SDL_WINDOWS_DPI_AWARENESS=permonitorv2` so the popup renders in physical
  pixels. Only the SDL frontend is covered; the GTK frontend is not used.
  An owned popup is activated by a mouse click even with `WS_EX_NOACTIVATE`,
  which would make SDL drop the click and take the keyboard away from the
  webview, so the product starts QEMU with `-display sdl,activate-on-click=off`
  (OME QEMU patch 0004). Changing the SDL window's style, owner or size from
  outside QEMU freezes its OpenGL presentation until the guest's next mode
  switch (2026-09-30, same evidence document), so the product lets QEMU create
  the window already owned and borderless (`-display sdl,owner-window=`, patch
  0005) and sets its position, size and visibility through the QMP command
  `x-ome-display-window`; the window never follows the guest resolution on its
  own. Since 0.1.1 (patch 0006) the window's size is also never reported to the
  guest as its display size, so the resolution comes from the display settings
  alone, and mouse coordinates are mapped through the letterboxed placement of
  the guest image inside the window. A QEMU without these patches, such as a
  distribution build, is not usable for hosting: the guest window would activate
  on click and freeze.
- The product draws its own title bar (0.1.1). Windows 11's snap layout menu,
  which opens when the pointer rests on a standard maximize button, does not
  open on the product's maximize button.
- A host whose default OpenGL context is Microsoft's software fallback (`GDI Generic`,
  OpenGL 1.1: no display driver, a basic VM display adapter, some remote sessions)
  cannot run virgl; QEMU exits with `No provider of glCreateShader found`
  (2026-09-30, `docs/evidence/M2/dod-ci.md`, run 22 on a GitHub windows-2025
  runner). The product probes a plain WGL context once at start-up and, when it
  reports no OpenGL 2.0 renderer, switches the graphics setting to software
  rendering and says so in a notice; the setting stays editable. A host without
  any audio output device cannot open DirectSound either, and QEMU treats that
  as fatal too (run 21); the product then starts the guest without an audio
  backend.
- The shared folder is a one-way copy: Settings > 저장 위치 > 공유 폴더 > 가상 머신으로 보내기 pushes the
  folder's files with adb into `/sdcard/OME/` on the guest. There is no live
  mount (the Windows QEMU build has neither 9p nor virtiofs, and QEMU's built-in
  SMB needs Samba on the host) and nothing comes back from the guest. Whether
  gallery apps see pushed media right away depends on the media scan request
  succeeding (unverified on Bliss 16.9.7); the Files app shows the folder
  regardless.
- Multi-instance, macros, and scripted automation are not part of v1 (D9).
