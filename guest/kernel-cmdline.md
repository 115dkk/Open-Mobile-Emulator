# Bliss OS boot arguments on QEMU (experiment log)

Research date: 2026-09-25. No VM was booted for this document. A documented
option is not evidence that it works with our ISO, Windows driver, or CPU model.

## What the official QEMU page actually recommends

The [Bliss QEMU guide][qemu] has a **Bliss 14 / Linux KVM** example, not a
Bliss 16 / Windows WHPX example. Its command is quoted below, with its original
paths, networking and audio preserved. Do not run it as a Windows command.

```bash
qemu-system-x86_64 \
-enable-kvm \
-M q35 \
-m 4096 -smp 4 -cpu host \
-bios /usr/share/ovmf/x64/OVMF.fd \
-drive file=disks/bliss14-k54-gapps.qcow2,if=virtio \
-cdrom images/<BLISS_IMAGE_NAME>.iso \
-usb \
-device virtio-tablet \
-device virtio-keyboard \
-device qemu-xhci,id=xhci \
-machine vmport=off \
-device virtio-vga-gl -display sdl,gl=on \
-audiodev pa,id=snd0 -device AC97,audiodev=snd0 \
-net nic,model=virtio-net-pci -net user,hostfwd=tcp::4444-:5555
```

Source: [official QEMU installation page][qemu]. It reports that the default
AOSP 12 entry fails with virgl and that GBM Mesa can crash the UI when drawing a
mouse; it suggests the `hwcomposer.drm` entry, removing `DEBUG=2` for normal boot.
It also contains an old warning that its Bliss 14 Windows configuration does not
work. Neither observation establishes compatibility with Bliss 16.

**UNVERIFIED:** A Bliss 16-specific official recommended QEMU command was not
found. Our M1 choice remains the project specification (WHPX plus
`-device virtio-vga-gl -display sdl,gl=on`); the source above verifies the GPU/display
syntax, not successful execution on this host. No KVM, Linux audio, unofficial
QEMU forks, or detection workarounds are adopted from the example.

The [advanced QEMU page][advanced] gives the guest kernel string
`root=/dev/ram0 console=ttyS0 HWC=drm_minigbm GRALLOC=minigbm_arcvm`.
It says most of the page assumes Linux, and does not state a Bliss 16 version.

## Ordered candidates

Preserve the installed entry's disk paths, `root=`, `SRC=`, `androidboot.*` and
`initrd` line. Replace conflicting graphics options rather than appending duplicate
keys. The ordering below is an OME test plan, not an upstream ranking.
**UNVERIFIED:** All expected effects in the last column still require this ISO's
boot/logcat evidence. Test one row at a time; do not concatenate the rows.

| Candidate | What it selects | Source URL | Expected effect (not a result) |
|---|---|---|---|
| M0-1: `nomodeset` with the project's standard VGA device | Legacy SwiftShader EGL software rendering; disables modesetting | [Command-line parameters][params] | Establish a software-rendered boot baseline; no virgl claim |
| M0-2: `HWACCEL=0` instead of `nomodeset` | Documented alternative for legacy software rendering | [Command-line parameters][params] | Test the alternate software-rendering selector if M0-1 does not boot |
| M0-3: `nomodeset ANGLE=1` | ANGLE with software SwiftShader Vulkan | [Command-line parameters][params] | Test only if the legacy software renderer fails; this is guest software Vulkan, not host Vulkan acceleration |
| M1-1: `HWC=drm_minigbm GRALLOC=minigbm_arcvm` | drm composer with minigbm and the virgl-oriented allocator | [Advanced QEMU][advanced], [parameter definitions][params] | First QEMU-specific composer/allocator candidate |
| M1-2: `HWC=drm_minigbm_celadon GRALLOC=minigbm` | Celadon composer plus minigbm | [Command-line parameters][params] | Compare the documented modern-hardware combination if the first candidate boots but misrenders |
| M1-3: `HWC=drm GRALLOC=gbm` | Legacy DRM composer with GBM | [Command-line parameters][params], [QEMU caveat][qemu] | Last documented alternate; inspect mouse drawing and UI crashes explicitly |
| Resolution test, after a successful M1 boot: `video=1920x1080` | Kernel display mode override | [QEMU resolution instructions][qemu] | Request 1080p; record actual guest mode and cursor alignment |

`GRALLOC4_MINIGBM` is a standalone flag (not a `GRALLOC=` value), documented as
compatible only with `drm_minigbm_celadon` in [the parameter reference][params].
It is not added to the initial candidates.

**UNVERIFIED:** The fetched current parameter reference does not document
`VULKAN=` or valid values for it. No `VULKAN=0/1` guess is prescribed. Likewise,
`video=1280x720` would be an adaptation of the documented 1080p example, not a
separately verified Bliss 16 preset.

The [graphics troubleshooting page][graphics] also has older `HWC=drmfb`,
`HWC=none` and `GRALLOC=none` examples and identifies `HWC=drm` as standard for
Android 9 and earlier. They are not promoted to Bliss 16 defaults here.

## `savedefault` already persists the last entry (observed 2026-09-25)

Every entry in the installed `grub.cfg` runs `savedefault`, and the ESP's
`grubenv` is writable, so the entry booted last becomes the default at the next
boot. After booting `VM Options -> ... No HW Acceleration` once, the following boot
highlighted `VM Options ->` by default (`docs/evidence/M0/guest-install.md`). For
the M0 harness no `grub.cfg` edit is needed; the editing procedure below stays
for settings that are not menu entries (for example `video=`).

## Edit once, then persist only a proven setting

1. At the installed GRUB menu, highlight the entry and press `e`. Edit only the
   graphics additions on the `linux` kernel line. Press Ctrl+X to boot this edit
   once. [Command-line configuration][params]
2. After a successful test, back up and edit the **guest's** `/boot/grub/grub.cfg`.
   The general parameter reference also mentions `/etc/grub.d/40_custom` on
   systems where another distribution manages GRUB. [Command-line configuration][params]
3. The older advanced-install guide locates `boot/grub/grub.cfg` on the installed
   EFI partition and uses a `linux ... androidboot.*` entry. Keep those existing
   parameters and file locations intact. Do not mount or edit the Windows host's
   ESP to change a virtual guest. [Advanced installation][install]
4. **UNVERIFIED:** The exact partition/mountpoint holding this file in the chosen
   Bliss 16.9.x installation must be inspected after installation; the docs cover
   several installer generations. Record it before giving a permanent edit command.

## Support evidence and stop conditions

[Support issue #8][issue8] reports a Bliss 15.8.6 boot/debug loop with Proxmox,
q35 and OVMF. The fetched issue provides no confirmed fix and no Bliss 16 virgl
recipe. It is failure evidence, not a recommended workaround.

The requested Windows/virgl [community article][community] returned HTTP 403;
no command or build instruction from it is treated as verified here.

If virgl initialization fails or the host driver produces a black screen, save
logs and driver details and report it (M1 stop condition). Do not cycle through
undocumented patches to conceal the failure. Stop on native-bridge library
crashes or an application's emulator rejection as required by the project.

## What the 16.9.7 ISO's own GRUB menu uses (read on 2026-09-25)

The ISO's `VM Options ->` submenu carries two virtual-machine entries. Their
parameters were read from the GRUB edit screen
(`docs/evidence/M0/install-03-grub-entry-qemu-virgl.png`, `install-04-grub-entry-no-hw-accel.png`):

| Entry | Parameters after `quiet` | Matches candidate |
|---|---|---|
| `Live - QEMU/KVM - Virgl - SW-FFMPEG` | `HWC=drm_minigbm GRALLOC=minigbm_arcvm` | M1-1 |
| `Live - Vbox/VMWare - No HW Acceleration` | `nomodeset HWACCEL=0` | M0-1 and M0-2 together |

The linux line is `linux $kd/kernel root=/dev/ram0 $src $@`, so these are the only
graphics-related additions the ISO authors ship for VMs. M0 uses the second set
with standard VGA; M1 starts from the first set with `virtio-vga-gl`.

## Experiments

| Date | Harness | GPU device | Kernel cmdline additions | Result |
|---|---|---|---|---|
| 2026-09-25 | Distribution QEMU 11.1.0, `-accel whpx,kernel-irqchip=off`, `-cpu max`, q35, 6 GiB, 4 vCPU | `VGA` | ISO default entry (`Live PC-Mode (Default)`), then `Installation`; none added | Hang after the init banner in both entries; no adb for 10 min. Cause is the CPU model, not the command line: under WHPX `max` is the TCG model (AMD vendor, TCG feature set). Details in `docs/evidence/M0/guest-install.md`. |
| 2026-09-25 | Same, `-cpu Skylake-Client-v4` | `VGA` | `Installation` entry, none added | Installer text UI after 16 s. Installation completed from this boot. |
| 2026-09-25 | Same, installed disk | `VGA` | installed default entry (`quiet` only; kernel modesets `bochs-drm`) | Hang in early userspace: idle CPU, no DHCP after 2 min, adb SYNs unanswered. |
| 2026-09-25 | Same, installed disk | `VGA` | `quiet nomodeset HWACCEL=0` (installed `VM Options` entry) | Boots. adb `device` after 42 s, setup wizard on screen, arm64 probe APK runs. Host window shows a garbled 1024x768 frame (pitch mismatch with the guest's 1280x800), `adb screencap` is correct. See `docs/evidence/M0/guest-install.md`. |
| 2026-09-25 | Same, installed disk | `virtio-vga-gl`, `sdl,gl=on` | `quiet nomodeset HWACCEL=0` (saved default) | Boots; host window and QMP `screendump` correct at 1024x768; renderer still SwiftShader. Preferred M0 harness display. |
| 2026-09-25 | Same, installed disk | `virtio-vga-gl`, `sdl,gl=on` | `quiet HWC=drm_minigbm GRALLOC=minigbm_arcvm` (installed `VM Options` virgl entry, candidate M1-1) | Boots in 16 s to adb. SurfaceFlinger `GLES: Mesa, virgl (NVIDIA GeForce RTX 2080 SUPER/PCIe/SSE2), OpenGL ES 3.2 Mesa 24.0.8`, hwcomposer `drm_minigbm`, gralloc `minigbm_arcvm`, 1280x800. QMP `screendump` returns `no surface` (GL scanout); `adb screencap` works. M1-2 and M1-3 not needed. `docs/evidence/M1/whpx-boot.md`. |
| 2026-09-26 | Same, `-m 8192` instead of 6144 | `virtio-vga-gl`, `sdl,gl=on` | virgl entry (saved default) | Boots. Guest MemFree 1.46 GB during the game instead of 80 to 282 MB at 6 GiB; frame rate unchanged (38 fps, see the EDID row). 8192 is now the launcher default. `docs/evidence/M0/metrics.md`. |
| 2026-09-26 | Same | `virtio-vga-gl`, `sdl,gl=on` | Custom entry `OME virgl FFMPEG` in `/boot/grub/custom.cfg` on vda2 (sourced by Bliss's `android.cfg`): `quiet HWC=drm_minigbm GRALLOC=minigbm_arcvm FFMPEG_CODEC=1 FFMPEG_CODEC2_PREFER=1` | Boots. The ffmpeg codec2 decoders are chosen from the first launch and `media.swcodec` no longer aborts, but in-app video stays black (the break is between decoder output and Unity's texture). The shipped menu's `FFMPEG_PREFER_C2=1` spelling does nothing: `/system/etc/init.sh` reads `FFMPEG_CODEC2_PREFER`. Not the default. `docs/evidence/M0/findings-20260926.md`. |
| 2026-09-26 | Same | `virtio-vga-gl`, `sdl,gl=on` | Custom entry `OME virgl FFMPEG 60Hz`: the row above plus `video=Virtual-1:1280x800@60` (the resolution-test candidate) | Boots; no effect on the mode. The guest's only DRM mode stays the EDID's 1280x800 at 75 Hz, so `video=` cannot change the refresh rate here. |
| 2026-09-26 | Same, QEMU `-global virtio-vga-gl.edid=off` (now `virtio-vga-gl,edid=off` in the launcher) | `virtio-vga-gl,edid=off`, `sdl,gl=on` | virgl entry (saved default) | Boots. Without the generated EDID the guest falls back to a CVT 1280x800 at 60 Hz (SurfaceFlinger VSYNC 16.68 ms); the tested game presents 58.5 fps in battle instead of 38. Launcher default for `-Gpu virgl`. `docs/evidence/M0/findings-20260926.md`, `docs/evidence/M0/metrics.md`. |

[qemu]: https://docs.blissos.org/installation/install-in-a-virtual-machine/install-in-qemu/
[advanced]: https://docs.blissos.org/installation/install-in-a-virtual-machine/advanced-qemu-config/
[params]: https://docs.blissos.org/configuration/configuration-through-command-line-parameters/
[graphics]: https://docs.blissos.org/knowledgebase/troubleshooting/graphics-troubleshooting/
[install]: https://docs.blissos.org/installation/advanced-installation/
[issue8]: https://github.com/BlissRoms-x86/support/issues/8
[community]: https://guanzhang.medium.com/running-android-games-on-windows-10-11-using-qemu-hyper-v-virglrenderer-bliss-os-4eea9be9a06b
