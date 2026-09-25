# Bliss OS 16 guest installation under QEMU/UEFI

Research date: 2026-09-25. This is an operator checklist from official docs,
not a recording of an installation we ran. **UNVERIFIED:** Exact screens of the
selected Bliss 16.9.x ISO have not been observed. The docs mix installer generations.

## Before starting

Use the project's verified ISO and a new, disposable guest disk. Present only
that disk and the ISO to the VM; never pass a host physical disk to the installer.
Use UEFI firmware. The [official EFI guide][usb] warns that its partitioning
procedure erases the target drive. The [QEMU guide][qemu] shows OVMF and a virtio
disk, but its example is Bliss 14/Linux, not a tested Windows Bliss 16 installation.
Disk creation/launch remain in the launcher scripts. Do not change the Windows
host's firmware, ESP, Secure Boot or WHPX settings for these guest operations.

## Installer checklist

| Step | What to do | Evidence and remaining checks |
|---|---|---|
| 1. Boot ISO | Select the EFI entry `Android-x86 ... Installation`, not a live session. | [Bliss EFI installation][usb]. **UNVERIFIED:** Exact Bliss 16 menu wording/version suffix. |
| 2. Partitioning | Select `Create/Modify partitions`. Check the capacity matches the empty VM disk. | [Bliss EFI installation][usb]. **UNVERIFIED:** Fetched text does not name cgdisk, gdisk or cfdisk; record the tool banner. |
| 3. GPT | On the empty VM disk, choose GPT if asked. | The guide uses EFI type `ef00`; **UNVERIFIED:** GPT creation dialog/key sequence is not specified by [the guide][usb]. Stop and inspect if offered a different layout. |
| 4. EFI partition | Allocate `+512M`, type `ef00`, for EFI rather than Android root. | Size/type are explicit in [the guide][usb]. **UNVERIFIED:** FAT32-format dialog is not described. Intended ESP is FAT32; verify the installer formats it that way rather than guess a shell command. |
| 5. Root partition | Use remaining space for Android. Save with `[ WRITE ]`, then `[ QUIT ]`. Select that second partition for installation. | [Bliss EFI installation][usb] describes EFI plus a default Android partition. **UNVERIFIED:** Device names depend on attached disk; identify by size/type. |
| 6. Format root | Choose `ext4`; confirm `Yes` after checking the target. | Explicit in [the guide][usb]. Do not format EFI as ext4. |
| 7. Bootloader | Accept GRUB2 EFI installation and select the EFI partition when asked. | [The guide][usb] requires GRUB2 EFI and says newer installers ask for target ESP first. **UNVERIFIED:** Prompt ordering in this ISO; ESP selection may precede root formatting. |
| 8. OTA A/B, if offered | Choose `No` for this plain single-install test disk; record the answer. | [The guide][usb] says `Yes` requires twice the system space. `No` here is an OME test choice, not a universal upstream recommendation. **UNVERIFIED:** Whether this ISO shows the prompt. |
| 9. Writable system, if offered | Choose `No` for a plain install. | [Android-x86 installer docs][android-install] describe writable `/system` for debugging; otherwise read-only is sufficient. **UNVERIFIED:** Presence/text of this inherited prompt in Bliss 16. Translator replacement is a separate task. |
| 10. Installed boot | Finish, shut down/reboot as offered, detach ISO, then boot the virtual disk. | [Android-x86 installer docs][android-install] offer run-now or reboot. **UNVERIFIED:** ISO removal timing, first-boot setup screens and duration for this build. Detaching ISO is our procedure to prove installed-disk boot. |

Record actual screens/deviations in milestone evidence. Do not retry random
partitions or graphics flags after boot failure. Consult `guest/kernel-cmdline.md`
and retain the original entry.

## Native bridge

The [official hardware FAQ][hardware] identifies Google's `libndk_translation`
for Bliss 16/17 and requires x86_64-v2. It does not give a settings-toggle path.

**UNVERIFIED:** A `Native bridge` toggle and its location in Bliss 16 Settings
could not be confirmed. Do not claim a `Settings > Blissify > ...` route.
Inspect the installed ISO and record the actual label if one exists. If the
bridge is absent, report it rather than install another proprietary bridge or
silently switch guests.

M0 still requires recording the native-bridge property, ARM64 ABI list and guest
SSE4.2/POPCNT support. This research did not measure these inside a running guest.
The expected bridge implementation is supported by [the FAQ][hardware].

## Network ADB

1. Look under `Settings > System > Developer Options` for ADB over Wi-Fi/network.
   This path is in [Bliss Bass remote-management docs][adb-bass]. **UNVERIFIED:**
   The page does not state applicability to Bliss OS 16 or exact toggle label.
   Developer Options activation and exact screen sequence need ISO verification.
2. Enable network ADB only for the isolated test guest. Do not assume pairing-based
   `Wireless debugging` uses TCP 5555. The [QEMU guide][qemu] assumes a listener on
   5555 but does not explain how to enable it. **UNVERIFIED:** Which setting starts
   that listener on this ISO. If none exists, report it; no root/adbd property
   commands are prescribed without evidence.
3. The documented example maps `hostfwd=tcp::4444-:5555` and uses
   `adb connect localhost:4444`. [QEMU guide][qemu] Use the port actually printed
   by the OME launcher instead. The launcher must restrict exposure to localhost,
   not copy the example's all-interface binding. This is a security requirement,
   not a claim about the existing launcher's implementation.
4. **UNVERIFIED:** Authorization prompts and connectivity were not tested.
   Record connection state before installing apps. [Support issue #65][adb-issue]
   concerns Bliss 14.10 ethernet debugging, not a confirmed Bliss 16 solution;
   the fetched excerpt did not include working resolution steps.

## Account limitations

The guest is not Google-certified. Play sign-in and purchases are not guaranteed.
Follow the project's separate manual registration procedure. No automatic account
registration or certification bypass belongs here.

[usb]: https://docs.blissos.org/installation/install-from-bootable-usb/
[qemu]: https://docs.blissos.org/installation/install-in-a-virtual-machine/install-in-qemu/
[android-install]: https://www.android-x86.org/installhowto.html
[hardware]: https://docs.blissos.org/knowledgebase/frequently-asked-questions/hardware-compatibility/
[adb-bass]: https://docs.blisscolabs.dev/remote_management/using_scrcpy_for_remote_management/
[adb-issue]: https://github.com/BlissRoms-x86/support/issues/65
