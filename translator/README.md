# Translator contract

The ARM translator is a replaceable part (D4). Anything that satisfies this
contract can be installed with `install.ps1` and checked with `smoke.ps1`.

## Bundle layout

Follows the AOSP native-bridge layout:

```
<bundle>/
  translator.json
  system/bin/<arch>/
  system/lib/<arch>/
  system/lib64/<arch>/
  system/lib64/<bridge library>          e.g. libndk_translation.so or libberberis.so
  system/etc/binfmt_misc/
  system/etc/init/*.rc
```

## translator.json

```json
{
  "name": "string",
  "version": "string",
  "provider": "google | intel | digitalis",
  "android_api": 33,
  "host_arch": "x86_64",
  "guest_abis": ["arm64-v8a", "armeabi-v7a"],
  "bridge_lib": "libndk_translation.so",
  "props": {
    "ro.dalvik.vm.native.bridge": "libndk_translation.so",
    "ro.dalvik.vm.isa.arm": "x86",
    "ro.dalvik.vm.isa.arm64": "x86_64",
    "ro.enable.native.bridge.exec": "1",
    "ro.product.cpu.abilist": "x86_64,x86,arm64-v8a,armeabi-v7a,armeabi",
    "ro.product.cpu.abilist64": "x86_64,arm64-v8a",
    "ro.product.cpu.abilist32": "x86,armeabi-v7a,armeabi"
  },
  "source_url": "string",
  "source_sha256": "64 hex chars",
  "license": "SPDX id or free text"
}
```

`bundle-template/translator.json` is the empty template.

## Scripts (to be written when the first replacement bundle exists, P2)

- `install.ps1 -Bundle <dir> -Target <guest> [-Uninstall]`: pushes the bundle
  over `adb root; adb remount`, applies the props, reboots. Refuses when
  `android_api` differs from the guest API level. `-Uninstall` restores the
  previous state from a backup it made on install.
- `smoke.ps1`: (1) runs `tests/fixtures/hello_arm64`, (2) installs and runs the
  arm64 probe APK, (2-1) installs and runs the armeabi-v7a probe APK
  (`tests/fixtures/arm32-probe`; recorded as "not applicable" when the bundle's
  `guest_abis` has no 32-bit ABI, ADR-0008), (3) runs the short game scenario.
  Writes `docs/evidence/translator-<name>-<version>.md`.

## Where the bundled Google translator comes from

Bliss OS 16.9.x ships `libndk_translation` extracted from a Google emulator
image. Self-built images (P3) ship without it (`BLISS_BUILD_VARIANT=vanilla` with every
native-bridge and GApps flag off; Bliss's `foss` value only picks the microG bundle,
see `docs/P3-PLAN.md`): the
installer downloads a Google APIs x86_64 system image from `dl.google.com` on the
user's PC, extracts a bundle in this contract's layout there, and installs it
into the guest (R3, ADR-0006). No release, server, or mirror of this project
carries a guest image that contains Google files. The `vendor_google_emu-x86`
flow on a build machine stays a development and evidence tool.

The Google images are distributed under the Android SDK License Agreement
(`android-sdk-license` in the repository index), which grants use "solely to
develop applications for compatible implementations of Android" (3.1) and
forbids copying, modifying, and redistributing any part of the SDK (3.4). Running
games through the Google translator is outside 3.1; the choice this leaves is
CLAUDE.md section 8 item 9. Digitalis (P2) is the translator without that
restriction, for 64-bit ARM only: it has no ARM32 backend, so 32-bit ARM apps
stay on a proprietary translator whichever way item 9 is decided (ADR-0008).

The index entries for API 35 to 37 x86_64 images, with their SHA-1 and layout
notes, are in `docs/evidence/P3/research-google-images.md`; nothing from such an
image is committed.

## Digitalis bundles per API level (ADR-0008)

Digitalis tracks AOSP 16 (API 36), and its documentation does not support
mixing a bundle into an image of another platform release: the ARM64 guest
bionic and the proxy trampolines come from `frameworks/libs/native_bridge_support`
built from the same sources as the host image. The translator core (JITs,
interpreter, guest loader) is not tied to the Android version. OME therefore
does not hard-fork Digitalis; it builds the version-bound platform layer per API
level from source, in the order API 33 (the current guest), API 34 to 37, then
API 30 to 32. Where to build is to be decided (an AOSP tree may exceed a
GitHub-hosted runner's disk and time); a bundle that ships in a release still
needs ADR-0007 provenance. `native_bridge_support` exists in
AOSP release branches from Android 11, `binary_translation` from Android 14.

One guest has a single `ro.dalvik.vm.native.bridge` file name, but the 32-bit
zygote loads it from `/system/lib` and the 64-bit zygote from `/system/lib64`.
Running Digitalis for 64-bit and a proprietary translator for 32-bit in one guest
is therefore possible in principle (to be verified in P2).
