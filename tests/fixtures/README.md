# Self-built test fixtures

Only self-made, clearly licensed test material lives here (R1 exception rule).
Compiled outputs are build products and are not committed; `build.ps1`
regenerates them with the Android NDK found under the Android SDK.

- `hello_arm64/`: a static arm64 Linux binary printing a fixed string. Used by
  `translator/smoke.ps1` step 1 to prove the native bridge executes arm64 code.
- `arm64-probe/`: a minimal arm64-only APK whose native library reports the ABI
  it runs on. Used by `translator/smoke.ps1` step 2.
