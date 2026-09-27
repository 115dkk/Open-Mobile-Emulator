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
  arm64 probe APK, (3) runs the short game scenario. Writes
  `docs/evidence/translator-<name>-<version>.md`.

## Where the bundled Google translator comes from

Bliss OS 16.9.x ships `libndk_translation` extracted from a Google emulator
image. Self-built images (P3) ship without it (`BLISS_BUILD_VARIANT=foss`): the
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
restriction.

The development host already holds such an image under the Android SDK
(`system-images/android-36/google_apis/x86_64`), which is useful for inspecting
the layout; nothing from it is committed.
