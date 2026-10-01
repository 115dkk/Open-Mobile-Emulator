# P3 조사 B: 구글 에뮬레이터 이미지, 변환기, Digitalis (2026-10-01)

ASTRA 워커가 2026-10-01에 공식 색인과 저장소 원문을 읽어 확인한 사실표다. 바이너리는 내려받지 않았고, 접근일은 모두 2026-10-01이다.

## 1. 공식 시스템 이미지 색인 (sys-img2-5.xml, 접근 2026-10-01)

모든 행 `uses-license = android-sdk-license`, 체크섬 type `sha1`(색인 값, 내려받아 계산한 값 아님), 크기는 zip 바이트. 35개 항목 모두 `<translatedAbis>arm64-v8a</translatedAbis>`.

### 1-1. Google APIs (`https://dl.google.com/android/repository/sys-img/google_apis/`)

| 패키지 경로 | rev | zip | bytes | SHA-1 |
|---|---:|---|---:|---|
| `system-images;android-35;google_apis;x86_64` | 9 | `x86_64-35_r09.zip` | 1738815903 | `0103e6dab21290c4b9d16550a3ce99476f884eef` |
| `system-images;android-35;google_apis_ps16k;x86_64` | 5 | `x86_64-ps16k-35_r05.zip` | 1696342085 | `d709cf26b375abecfbbf5803afd20749ddfa2865` |
| `system-images;android-35-ext15;google_apis;x86_64` | 1 | `x86_64-35-ext15_r01.zip` | 1742908646 | `5e359834781e52fefcccd922a15f04bef5bade76` |
| `system-images;android-36;google_apis;x86_64` | 7 | `x86_64-36_r07.zip` | 1895447397 | `c6bf44bdcd885bb902b4ba752d111a073ad7a817` |
| `system-images;android-36;google_apis_ps16k;x86_64` | 7 | `x86_64-ps16k-36_r07.zip` | 1849428145 | `dd783282e84bf475a02eba6777c79fc5695e1583` |
| `system-images;android-36-ext18;google_apis;x86_64` | 1 | `x86_64-36-ext18_r01.zip` | 1888216599 | `a630fc642602137e6b00201b1ae343d54bc408dc` |
| `system-images;android-36-ext19;google_apis;x86_64` | 1 | `x86_64-36-ext19_r01.zip` | 1891037723 | `a641ca5992fef209c23e36a999cd7b3c12758ff6` |
| `system-images;android-36.1;google_apis;x86_64` | 4 | `x86_64-36.1_r04.zip` | 1960532087 | `15261872d5f0ae4b5728faefd0380d51b61a5b23` |
| `system-images;android-36.1;google_apis_ps16k;x86_64` | 4 | `x86_64-ps16k-36.1_r04.zip` | 1913923004 | `d812164d3704c2d5846d34e0a4d2c61cb1224a02` |
| `system-images;android-37.0;google_apis;x86_64` | 6 | `x86_64-37.0_r06.zip` | 2234615040 | `629e507fd5b737c2c836b12b52c81cd0e3b12399` |
| `system-images;android-37.0;google_apis_ps16k;x86_64` | 7 | `x86_64-ps16k-37.0_r07.zip` | 2081395296 | `9c50c3299708039310c98c94d698a75861b2f1bf` |
| `system-images;android-37.1;google_apis_ps16k;x86_64` | 9 | `x86_64-ps16k-37.1_r09.zip` | 2178012675 | `35c0da1cd298f3426a5b48233f59515097446a28` |
| `system-images;android-37.2-beta1;google_apis_ps16k;x86_64` | 1 | `x86_64-ps16k-37.2-beta1_r01.zip` | 2404586110 | `12ad6acabf3ab7a51f618776b94713665f5a3e18` |
| `system-images;android-37.2-beta2;google_apis_ps16k;x86_64` | 2 | `x86_64-ps16k-37.2-beta2_r02.zip` | 2411488936 | `c2ece4bc0dc42d33d1c725c661ff8a344d457150` |
| `system-images;android-37.2-beta3;google_apis_ps16k;x86_64` | 3 | `x86_64-ps16k-37.2-beta3_r03.zip` | 2421404738 | `2a4f408deb1dd8dca80e5f352019ebc79caf127c` |
| `system-images;android-37.2;google_apis_ps16k;x86_64` | 5 | `x86_64-ps16k-37.2_r05.zip` | 2437698985 | `01ea96227f7c20db3a9c2d9c2ed6a2e878f81c5d` |
| `system-images;android-canary-20260909;google_apis_ps16k;x86_64` | 16 | `x86_64-ps16k-canary-20260909_r16.zip` | 2312531033 | `57f53265916243fa5bafa9ecee3526b11b55721b` |

### 1-2. Google Play (`https://dl.google.com/android/repository/sys-img/google_apis_playstore/`)

| 패키지 경로 | rev | zip | bytes | SHA-1 |
|---|---:|---|---:|---|
| `system-images;android-35;google_apis_playstore;x86_64` | 9 | `x86_64-35_r09.zip` | 1762061559 | `2f0054868e6aab3c098acd3decba17a82aed4176` |
| `system-images;android-35;google_apis_playstore_ps16k;x86_64` | 5 | `x86_64-playstore-ps16k-35_r05.zip` | 1510397655 | `921af95f9566e79cd2e2c9d08fd253c58d18595e` |
| `system-images;android-36;google_apis_playstore;x86_64` | 7 | `x86_64-36_r07.zip` | 1924689206 | `16fa3c441d29fde6c9eea2f766eecf77032d68b4` |
| `system-images;android-36.1;google_apis_playstore;x86_64` | 4 | `x86_64-36.1_r04.zip` | 2001299692 | `ab58abb8dbbc8a00c9437c30c210e8b6a5286795` |
| `system-images;android-37.0;google_apis_playstore;x86_64` | 6 | `x86_64-37.0_r06.zip` | 2326241523 | `6950c614f07592b18d21a6e3e65d40a9aa1c3635` |
| `system-images;android-37.1;google_apis_playstore_ps16k;x86_64` | 9 | `x86_64-playstore-ps16k-37.1_r09.zip` | 2199005985 | `d1e31a6c6bf800479f4a552c941eb0169c3eced3` |
| `system-images;android-37.2;google_apis_playstore_ps16k;x86_64` | 5 | `x86_64-playstore-ps16k-37.2_r05.zip` | 2457416861 | `7735e262360c5240789ada41ab40bbe4de6eb4c3` |

(ext14/ext15/ext18/ext19, ps16k 36, 37.0 ps16k, 37.2 beta1~3, canary 행은 생략. 색인에 있음.)

- 37.1, 37.2, canary의 x86_64는 `ps16k` 변형뿐. 베타의 `api-level`은 37.1, `beta-api-level` 37.2. canary `api-level` 37.2.

## 2. zip 내부 구조와 추출 대상

- 최근 이미지의 바깥 `system.img`는 GPT 디스크 → `super` → 동적 파티션(`system`, `product` 등). 단일 ext4가 아님. 분석 코드: https://github.com/JoshuaKGoldberg/emoji-platform-data/blob/main/packages/generator/scripts/refreshAndroid.ts
- 37.0 일반 Play 이미지의 `product`는 EROFS(`dump.erofs`, erofs-utils 1.8.5+). 37.1/37.2 ps16k는 ext4(7-Zip으로 읽은 보고). https://github.com/JoshuaKGoldberg/emoji-platform-data/issues/1085
- API 34~36의 `system` 파티션 파일 시스템은 확인 못 함. sparse(simg2img) 여부도 확인 못 함.
- Android-Generic `update.sh`(unified): zip 해제 → `7z e system.img` → `binwalk -e super.img` → `7z x _super.img.extracted/100000.ext*`. API 35~37 성공은 확인 못 함.
- 변환기 파일 목록(Android 11 스크립트 `ext_emu_x86_64.sh`, r11-x86_64): `/system/lib64/libndk_translation.so`, `/system/lib64/libndk_translation_proxy_*.so`, `/system/lib64/arm64/`, `/system/bin/arm64/`, `/system/etc/binfmt_misc/`, `/system/etc/ld.config.arm64.txt`, `/system/etc/init/ndk_translation_arm64.rc`, `/system/bin/ndk_translation_program_runner_binfmt_misc_arm64`.
- 최근 이미지 추가 사항(Digitalis README): API 34~37 Google APIs에 `ro.dalvik.vm.native.bridge=libndk_translation.so`, `/system/etc/berberis/cpuinfo.arm64.txt`, binfmt `arm64_dyn`, `arm64_exe`.
- **API 35 묶음에는 `libberberis_exec_region.so`도 필요**(없으면 `berberis::ExecRegionElfBackedFactory::Create`에서 중단). https://github.com/remote-android/redroid-doc/issues/933#issuecomment-5698603124
- API 35~37 전체 추출 명세(init rc, 프록시 목록, APEX 의존, SELinux)는 확인 못 함.

## 3. API 35~37 변환기 보고

- 색인 `translatedAbis=arm64-v8a` 전부. Digitalis README: API 34~37 Google APIs x86_64에 Berberis 기반 비공개 ARM64 구현(`berberis::intrinsics::Arm64ReadFpcr` 심볼).
- redroid #933(2026-08~09): API 35 변환기를 redroid 15에 넣어 arm64 전용 APK 실행 성공. API 36 변환기는 Android 17에서 로드 안 됨, API 37.0은 로드됨(VLC, Termux). binfmt `P` 플래그 문제 보고.
- 에뮬레이터 35.6.11(2025-06-24) 릴리스 노트: `libndk_translation.so aborts ... Arm64AesEncode` 수정(이슈 388718417). https://developer.android.com/studio/releases/emulator

## 4. Android-Generic 추출 저장소

- 실제 URL `https://gitlab.com/android-generic/android_vendor_google_emu-x86`(프로젝트 23029797). 브랜치 `r11-x86_64`(기본, 마지막 커밋 2021-10-27), `unified`(2022-03-23). README는 Android 10~12, 스크립트는 SDK 29~32만. API 33~37 지원 확인 못 함.
- 입력은 Play 이미지 `x86_64-30_r09-linux.zip`(SDK 29·30), `x86_64-31_r08-linux.zip`(SDK 31·32) 고정.
- 빌드 통합: `BoardConfig.mk`에 `board/native_bridge_arm_on_x86.mk`, 제품에 `target/native_bridge_arm_on_x86.mk`, 변환기는 `target/libndk_translation.mk`.
- 라이선스: 루트 LICENSE는 스크립트 MIT, 추출 파일은 재배포 불가. `unified/update.sh` 머리는 `GPLv3+`.

## 5. Android SDK 라이선스

- https://developer.android.com/studio/terms 는 2026-04-28 판 그대로. 3.1 "solely to develop applications for compatible implementations of Android", 3.4 복제·수정·재배포 금지, 3.5 오픈소스 구성요소는 그 라이선스만 적용. 사용자 PC 추출에 대한 예외 문구 없음.

## 6. Digitalis

- 조직 저장소 6개(기본 브랜치 `android-latest-release`): `digitalis`(push 2026-09-13), `platform_frameworks_libs_binary_translation`(2026-09-13), `manifest`(2026-08-05), `device_generic_goldfish`(2026-06-09), `sample_hello_digitalis`, `digitalisx64.github.io`.
- 기반: `manifest/default.xml` revision **`android16-qpr2-release`**, AOSP 16 / API 36. 공개 GitHub Release·태그 없음. 릴리스 노트 최신 2026-09-13.
- 번들: `docker/build-digitalis.sh`, `scripts/build-and-package-prebuilts.sh`가 `dist/digitalis-prebuilts/`와 `digitalis-prebuilts-<date>-<sha>.tar.gz`를 만든다. 전체 빌드(`out/target/product/emu64xa`, 타깃 `sdk_phone64_x86_64_digitalis-trunk_staging-userdebug`) 전제. 완성 바이너리 자산은 확인 못 함.
- 번들 내용: `system/lib64/libberberis_arm64.so`, `libberberis_exec_region.so`, `libberberis_proxy_*.so`(21개), `system/bin/berberis_program_runner_arm64`, `berberis_program_runner_binfmt_misc_arm64`, `system/lib64/arm64/*.so`(74개 산출물), `system/bin/arm64/app_process64`, `linker64`, `system/etc/ld.config.arm64.txt`, `system/etc/init/berberis.rc`, `system/etc/binfmt_misc/arm64_dyn`, `arm64_exe`, `digitalis-prebuilts.mk`, `MANIFEST.txt`, `SHA256SUMS`.
- 설치: 소비자 AOSP 트리 `vendor/digitalis/prebuilts/`에 두고 `inherit-product digitalis-prebuilts.mk` 뒤 이미지 빌드. 다른 플랫폼 릴리스의 번들 혼용 미지원. adb 주입 절차 문서 없음.
- 보고: LineageOS 23.2/Android 16 QPR2 WayDroid 시험(이슈 #1, 초기 실패 → 2026-06-05 수정 뒤 닫힘). 2026-09-13 노트: host tests 3,731/0, 샘플 158/158, 실제 APK 135 PASS / 2 FAIL(Clash of Clans 에뮬레이터 검사, AliExpress 데몬). Geekbench 6 529/1,592(네이티브 2,511/7,136), 3DMark WLE 4,903(네이티브 5,458). Google 변환기와의 직접 비교표 없음. Apache-2.0.
- 출처: https://github.com/DigitalisX64/digitalis , https://github.com/DigitalisX64/manifest/blob/android-latest-release/default.xml , https://github.com/DigitalisX64/digitalis/blob/android-latest-release/docker/README.md , https://github.com/DigitalisX64/digitalis/blob/android-latest-release/docs/integrating-digitalis.md , https://github.com/DigitalisX64/digitalis/blob/android-latest-release/docs/RELEASE_NOTES.md , https://github.com/DigitalisX64/platform_frameworks_libs_binary_translation/issues/1
