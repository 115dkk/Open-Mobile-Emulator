# P3 조사 A: BlissOS 소스 트리와 자체 빌드의 현재 사실 (2026-10-01)

ASTRA 워커가 2026-10-01에 공식 사이트와 GitHub·GitLab의 브랜치 목록과 원본 파일을 읽어 확인한 사실표다. 문서에
적힌 요구 사항이지 빌드나 실행으로 검증한 수치가 아니며, 아래 날짜는 모두 접근일이다. 작업서의 전제에서 틀린 것이
셋 있었다.

- **BlissOS 18은 안드로이드 15이고, `arcadia-x86`은 안드로이드 16이 아니라 BlissOS 15 / 안드로이드 12L이다.**
- **`BLISS_BUILD_VARIANT=foss`만으로는 네이티브 브리지가 빠지지 않는다.** 브리지와 구글 구성요소는 별도 플래그로 들어간다.
- **현재 트리는 `bootable/newinstaller` 대신 `bootable/aaropa`(Calamares 기반)를 쓴다.** `AUTO_INSTALL` 무인 설치의 근거는 없다.

## 1. 소스 트리와 안드로이드 버전

마지막 커밋은 manifest 브랜치 HEAD의 committer 날짜(UTC)다.

| 저장소 / 브랜치 | 버전과 `default.xml` | 마지막 커밋 | 출처 |
|---|---|---|---|
| `BlissRoms-x86/manifest` / `arcadia-x86` | BlissOS 15 / 안드로이드 12L(API 32). `android-12.1.0_r22`. vendor 15.9.3, codename Arcadia | 2026-04-12 `98a0a79cfffb` | https://github.com/BlissRoms-x86 , https://raw.githubusercontent.com/BlissRoms-x86/manifest/arcadia-x86/default.xml , https://raw.githubusercontent.com/BlissRoms-x86/platform_vendor_bliss/arcadia-x86/config/versions.mk |
| `BlissOS/platform_manifest` / `typhoon-x86` | BlissOS 16 / 안드로이드 13(API 33). `android-13.0.0_r77` | 2025-05-21 `8b3372680fdb` | https://raw.githubusercontent.com/BlissOS/platform_manifest/typhoon-x86/default.xml |
| 같은 저장소 / `universe-x86` | BlissOS 17 / 안드로이드 14(API 34). `android-14.0.0_r67` | 2026-05-11 `11e0522537b6` | https://raw.githubusercontent.com/BlissOS/platform_manifest/universe-x86/default.xml |
| 같은 저장소 / `universe-x86-qpr1` | `android-14.0.0_r25` | 2024-04-14 | https://raw.githubusercontent.com/BlissOS/platform_manifest/universe-x86-qpr1/default.xml |
| 같은 저장소 / `voyager-x86` | **BlissOS 18 / 안드로이드 15(API 35). `android-15.0.0_r14`.** vendor 18.4, codename Voyager, `BLISS_VERSION_STATIC=15.0` | 2026-05-14 `db973c9fec45` | https://raw.githubusercontent.com/BlissOS/platform_manifest/voyager-x86/default.xml , https://raw.githubusercontent.com/BlissOS/platform_vendor_bliss/voyager-x86/config/versions.mk |
| 같은 저장소 / `voyager-x86-qpr2` | 기본 revision `refs/heads/lineage-22.2`(AOSP 태그 아님). 최종 API 확인 못 함 | 2026-03-12 | https://raw.githubusercontent.com/BlissOS/platform_manifest/voyager-x86-qpr2/default.xml |
| 같은 저장소 / `voyager-qpr2-x86` | 위와 다른 브랜치. `refs/heads/lineage-22.2` | 2026-05-14 `0f029625d9e9` | https://raw.githubusercontent.com/BlissOS/platform_manifest/voyager-qpr2-x86/default.xml |
| `BlissRoms-x86/manifest` / `voyager-x86-qpr2` | 옛 조직에도 있음. `refs/heads/lineage-22.2` | 2026-03-10 | https://raw.githubusercontent.com/BlissRoms-x86/manifest/voyager-x86-qpr2/default.xml |
| 안드로이드 16·17용 BlissOS PC 트리 | **확인 못 함.** 두 manifest 저장소의 브랜치 목록과 조직 설명에 없음 | | https://api.github.com/repos/BlissOS/platform_manifest/branches?per_page=100 |

`arcadia-x86-130426`(2026-04-27), `arcadia-x86-new`, `arcadia-x86-surface`, `arcadia-bass`, `stable/arcadia-x86`(2025-02-04)의
`default.xml`도 모두 `android-12.1.0_r22`다. 이름이나 커밋 날짜만으로 새 안드로이드로 보면 안 된다.

## 2. 빌드 절차와 요구 사항 (Voyager README)

| 항목 | 사실 | 출처 |
|---|---|---|
| 초기화 | `repo init -u https://github.com/BlissOS/platform_manifest.git -b voyager-x86 --git-lfs` | https://raw.githubusercontent.com/BlissOS/platform_manifest/voyager-x86/README.md |
| 동기화 | `repo sync -c --force-sync --no-tags --no-clone-bundle -j$(nproc --all) --optimized-fetch --prune`. `--depth` 권고 없음(일부 프로젝트에 `clone-depth="1"`) | 같은 README |
| 요구 사항 | 최신 Ubuntu LTS, CPU 2코어 이상, **RAM 24 GB(VM 30 GB)**, **디스크 250 GB(소스 약 170 GB)**. 패키지 목록에 `libncurses5`(24.04에는 없음. Arcadia README는 `libncurses6`에 링크를 거는 절차를 적음). 어느 우분투에서 Voyager 빌드가 되는지는 확인 못 함 | 같은 README, https://raw.githubusercontent.com/BlissRoms-x86/manifest/arcadia-x86/README.md |
| 빌드 | `. build/envsetup.sh` → `lunch bliss_x86_64-ap4a-userdebug` → `make iso_img` | 같은 README |
| ISO 이름 | `$(PRODUCT_OUT)/$(BLISS_BUILD_ZIP).iso`. `Bliss$(BLISS_SPECIAL_VARIANT)-v18.4-$(BLISS_BUILD)-$(BLISS_BUILDTYPE)-$(BLISS_BUILD_VARIANT)-YYYYMMDD.iso`, build type 기본 `UNOFFICIAL` | https://raw.githubusercontent.com/BlissOS/platform_vendor_bliss/voyager-x86/config/versions.mk , https://raw.githubusercontent.com/BlissOS/bootable_aaropa/voyager-x86/Android.mk |
| 공식 빌드용 Docker 이미지 | 확인 못 함(`sickcodes/dock-droid`는 실행용) | https://docs.blissos.org/installation/run-from-docker/ |

## 3. `BLISS_BUILD_VARIANT`와 독점 구성요소

| 항목 | 사실 | 출처 |
|---|---|---|
| 정의 위치 | x86 vendor 구현은 `BlissOS/platform_vendor_bliss`(Typhoon·Voyager), `BlissRoms-x86/platform_vendor_bliss`(Arcadia). `config/versions.mk`가 기본값 `vanilla`, `config/common.mk`가 변형별 파일 포함 | https://raw.githubusercontent.com/BlissOS/platform_vendor_bliss/voyager-x86/config/common.mk |
| `vanilla` | 기본값. `gapps`, `foss`, `microg` 묶음을 고르지 않음. 모든 독점 파일을 빼는 전역 옵션이라는 근거는 없음 | 같은 파일 |
| `foss` | `vendor/foss/foss.mk`(android-generic `vendor_foss`: FDroidPrivilegedExtension, Provision, 생성되는 `apps.mk`, microG 선택). **Voyager `03-extras.xml`에는 `vendor/foss` 항목이 없어** 기본 sync만으로 준비되는지 확인 못 함 | https://raw.githubusercontent.com/android-generic/vendor_foss/hmchoice/foss.mk , https://raw.githubusercontent.com/BlissOS/platform_manifest/voyager-x86/android-x86/03-extras.xml |
| `gapps` | Voyager는 `vendor/gms/products/gms.mk`가 있으면 그것, 없으면 `vendor/gapps/products/gapps.mk`. Voyager에 `goapps`, `microg` 분기도 있음 | common.mk |
| 네이티브 브리지 | `device/generic/common/device.mk`와 x86_64 `BoardConfig.mk`가 `USE_LIBNDK_TRANSLATION_NB`, `USE_CROS_HOUDINI_NB`, `ANDROID_USE_NDK_TRANSLATION`, `ANDROID_USE_INTEL_HOUDINI`를 검사. **`BLISS_BUILD_VARIANT != foss` 같은 제외 조건 없음.** `USE_LIBNDK_TRANSLATION_NB=true`는 `widevine.mk`도 포함 | https://raw.githubusercontent.com/BlissOS/device_generic_common/voyager-x86/device.mk , https://raw.githubusercontent.com/BlissOS/device_generic_x86_64/voyager-x86/BoardConfig.mk |
| GApps 별도 포함 | `USE_EMU_GAPPS=true` → `vendor/google/emu-x86/target/gapps.mk`, `USE_OPENGAPPS=true` → OpenGApps | 같은 device.mk |

따라서 OME의 "블롭 없는 변형"은 `BLISS_BUILD_VARIANT=vanilla`에 브리지·GApps 플래그를 모두 끈 빌드다. 최종 산출물에
독점 파일이 전혀 없는지는 빌드한 뒤 목록으로 확인한다.

## 4. GPU 드라이버와 Mesa (빌드 설정의 포함 여부. 실행 성공이 아님)

| 트리 | virgl / Venus / gfxstream | Mesa | 출처 |
|---|---|---|---|
| Typhoon(16) | Gallium `virgl`, Vulkan `virtio`, 패키지 `vulkan.virtio`. ranchu·GLES emulation 없음 | `25.0_prebuilt-intel-shaders` = 25.0.3 | https://raw.githubusercontent.com/BlissOS/device_generic_common/typhoon-x86/gpu/gpu_mesa.mk |
| Voyager(18) | Gallium `virgl`, Vulkan `virtio`, 패키지 `vulkan.virtio`. `# GL/Vk implementation for gfxstream` 아래는 주석 처리된 빈 줄 | `24.3_v-x86` = 24.3.3 | https://raw.githubusercontent.com/BlissOS/device_generic_common/voyager-x86/BoardConfig.mk , https://raw.githubusercontent.com/BlissOS/device_generic_common/voyager-x86/gpu/gpu_mesa.mk , https://raw.githubusercontent.com/android-generic/external_mesa/24.3_v-x86/VERSION |
| Arcadia(15/12L) | 같음. gfxstream 주석 | `26.0/main` = 26.0.8 | https://raw.githubusercontent.com/BlissRoms-x86/device_generic_common/arcadia-x86/gpu/gpu_mesa.mk |

gfxstream 게스트 드라이버의 패키징은 확인하지 못했다. P1은 게스트 쪽 작업을 포함한다.

## 5. 커널과 장치·파일 시스템

| 항목 | 사실 | 출처 |
|---|---|---|
| 기본 커널 | 세 manifest 모두 `kernel/x86/common` = `android-generic/kernel_common` **`hm/crimson`**(움직이는 브랜치). 현재 Makefile **6.6.129** | https://raw.githubusercontent.com/BlissOS/platform_manifest/voyager-x86/android-x86/02-android-x86.xml , https://raw.githubusercontent.com/android-generic/kernel_common/hm/crimson/Makefile |
| 선택지 | Surface `hm/crimson-surface` 6.6.89, Zenith `android-generic/kernel-zenith` `6.12` = 6.12.77(`BOARD_IS_ZENITH_BUILD`). `kernel-ng`는 확인 못 함 | https://raw.githubusercontent.com/android-generic/kernel-zenith/6.12/Makefile |
| defconfig | `android-x86_64_defconfig` | https://raw.githubusercontent.com/BlissOS/device_generic_common/voyager-x86/build/tasks/kernel.mk |
| virtio | `CONFIG_DRM_VIRTIO_GPU=m`, `CONFIG_VIRTIO_NET=m`, `CONFIG_VIRTIO_BLK=y`, `CONFIG_VIRTIO_INPUT=m`, `CONFIG_VIRTIO_PCI=y` | https://raw.githubusercontent.com/android-generic/kernel_common/hm/crimson/arch/x86/configs/android-x86_64_defconfig |
| 파일 시스템 | `CONFIG_BLK_DEV_LOOP=y`, `CONFIG_EXT4_FS=y`, `CONFIG_EROFS_FS=y`, `CONFIG_SQUASHFS=y` | 같은 파일 |

## 6. 설치기와 부팅 구조

| 항목 | 사실 | 출처 |
|---|---|---|
| 현재 설치기 | 세 manifest 모두 **`bootable/aaropa`**. Devuan ceres 기반 작은 데스크톱과 **Calamares** 설치기 | https://raw.githubusercontent.com/BlissOS/platform_manifest/voyager-x86/android-x86/04-custom.xml , https://raw.githubusercontent.com/BlissOS/bootable_aaropa/voyager-x86/README.md |
| ISO 구성 | `Android.mk`가 `kernel`, `initrd.img`, `ramdisk-recovery.img`, system 이미지, 준비된 `iso/`를 넣음. 설치 환경은 `download.sh`가 `iso/install.sfs`로 넣음(구 `install.img`와 다름) | https://raw.githubusercontent.com/BlissOS/bootable_aaropa/voyager-x86/Android.mk , https://raw.githubusercontent.com/BlissOS/bootable_aaropa/voyager-x86/download.sh |
| system 형식 | Typhoon·Voyager `USE_SQUASHFS := 0`, `USE_EROFS := 1`(`system.efs`). Arcadia는 반대 | https://raw.githubusercontent.com/BlissOS/device_generic_common/voyager-x86/BoardConfig.mk |
| `SRC=` | initrd가 `/mnt/$SRC` 아래에서 `fstab.android`, `system$SLOT.?fs`, `system$SLOT.img` 등을 찾음 | https://raw.githubusercontent.com/BlissOS/bootable_aaropa/voyager-x86/initrd/init |
| `/scripts/*` | **`init` 226~229행: `for s in /scripts/* /mnt/"$SRC"/scripts/*; do test -e "$s" && source "$s"; done`**(2026-10-01 Claude가 직접 확인). ADR-0010의 도우미 훅이 그대로 통한다. 시스템 이미지는 `setup_loop`, `setup_ab_loops`(`/dev/block/by-name/`, `system_a.img`)로 다루므로 도우미의 `/sfs/system.img` 전제는 빌드한 ISO로 다시 확인한다 | 같은 파일 |
| `INSTALL` | `[ "$INSTALL" ] && setup_install` 뒤 `exec switch_root /install /sbin/init`. 무인 설치 코드가 아님. `AUTO_INSTALL` 처리 없음 | 같은 파일, https://raw.githubusercontent.com/BlissOS/bootable_aaropa/voyager-x86/initrd/scripts/1-install |
| GRUB | ISO 구성의 `boot/grub/grub.cfg`를 `Android.mk`가 치환 | Android.mk |
| 설치기 사전 빌드 파일 | `download.sh`가 `BlissOS/aaropa_rootfs/releases/latest`에서 `install.sfs`, `initrd_lib.tar.gz`, `grub-rescue.iso`, `boot_hybrid.img`를 받음. **고정 릴리스 번호도 SHA-256 대조도 없음** | download.sh |

## 7. 공식 ISO와 2026년 공지

| 항목 | 사실 | 출처 |
|---|---|---|
| 공식 상태 | 홈페이지: 공개 이미지는 중단, 소스 개발은 계속. 상태 페이지 "As of August 2026": 새 공식 이미지를 내지 않으며 일정 없음 | https://blissos.org/ , https://blissos.org/status.html |
| SourceForge `Official/` | `BlissOS16`, `BlissOS17`, `BlissOSZenith`, `BlissOS14`, `BlissOS15`, `Archive`. **BlissOS18 폴더 없음** | https://sourceforge.net/projects/blissos-x86/files/Official/ |
| 블로그 | 최신 개발 소식은 2025-08. 2026년 공지 확인 못 함. Telegram 본문 접근 못 함 | https://blog.blissos.org/ |

## 8. Android-Generic과 구글 에뮬레이터 추출 도구

| 항목 | 사실 | 출처 |
|---|---|---|
| 관계 | BlissOS README는 Android-Generic Project로 PC 빌드를 수정했다고 명시. AG 문서의 Bliss 소스 절차 예시는 `r11-r36`. Voyager를 ag-tool로 빌드하는 검증된 절차는 확인 못 함 | https://android-generic-project.gitbook.io/documentation/android-generic-project-using-with-bliss-os-source |
| 실제 저장소 | `gitlab.com/android-generic/android_vendor_google_emu-x86`(`android_` 접두사). 브랜치 `r11-x86_64`(2021-10-27), `unified`(2022-03-23) | https://gitlab.com/android-generic/android_vendor_google_emu-x86/-/branches |
| 지원 버전 | 스크립트는 SDK 29·30·31·32만. **API 33~37 분기 없음** | https://gitlab.com/android-generic/android_vendor_google_emu-x86/-/raw/unified/update.sh |
| 실행법 | `vendor/google/emu-x86`에 두고 `. vendor/google/emu-x86/update.sh x86_64`. `PLATFORM_SDK_VERSION`을 읽어 분기 | 같은 저장소 README |
| 포함 플래그 | `USE_LIBNDK_TRANSLATION_NB=true`가 이 저장소의 native bridge mk와 Widevine을 포함, `USE_EMU_GAPPS=true`가 `target/gapps.mk`를 포함 | https://raw.githubusercontent.com/BlissOS/device_generic_common/voyager-x86/device.mk |

## 확인 범위

워커는 파일 생성, 저장소 수정, 패키지 설치, 소스 sync, ISO 내려받기, 빌드, VM 부팅을 하지 않았다. 빌드 성공, 독점
파일 부재, 무인 설치 성공, GPU 실행 성공은 검증하지 않았다. GitHub API는 조사 후반에 rate limit 응답이 있었다.
