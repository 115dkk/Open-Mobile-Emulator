# P3 계획: 자체 게스트 이미지 (안드로이드 15, BlissOS 18 기반)

작성 2026-10-01. 사용자가 출시 후 트랙의 순서를 P3 → P1 → P2로 정한 날이다(CLAUDE.md 8절 6번).

**상태(2026-10-01 저녁): 진행 중, 빌드 호스트는 이 PC의 WSL2.** 5.1절의 선택지를 보고 사용자가 한때 하지 않기로
했으나(디스크 여유 C: 66 GB로는 250 GB를 채울 수 없고 유료 선택지는 쓰지 않는다), 같은 저녁 개발 PC에 남아 있던
페도라 44 설치(C: 바로 뒤의 ESP 2 GiB, /boot 2 GiB, btrfs 196.5 GiB. 2026-08에 깔다 만 것)를 지우고 C:를 늘린 뒤
작업 잔해(옛 세션 임시 홈, BlueStacks 5, 커널 덤프, LLM 실험, Codex 빌드 출력)를 치워 C: 여유를 400 GB로 만들었다.
빌드 호스트는 선택지 (가)다. Ubuntu 22.04 rootfs를 `C:\WSL\OME-Build`에 WSL2 배포판으로 들여와 쓰며, 만드는
절차와 빌드 스크립트는 `guest/build/`에 있다. 첫 빌드(2026-10-01 밤)는 48 %에서 Mesa의 설정 단계에 멈췄다. Ubuntu 22.04의
meson 0.61.2가 트리의 Mesa 24.3.3이 요구하는 1.1.0에 못 미쳐서이고, `setup-build-host.sh`가 PyPI의 meson(`pins.env`의
`MESON_PIP_VERSION`)을 대신 깔도록 고친 뒤 2026-10-02 새벽에 이어서 돌렸다. 같은 설정 단계에서 README가 빠뜨린 pkg-config, README에는
있으나 준비 스크립트가 빠뜨렸던 Rust 안드로이드 타깃과 cargo 프로그램(cargo-ndk, bindgen-cli 0.69.1, cbindgen), Intel GRL 커널의
python3-ply가 차례로 드러나 모두 준비 스크립트와 `pins.env`에 넣었고, 손으로 돌린 Mesa 설정 단계가 통과한 뒤 세 번째로 이어서 돌렸다. 세 번째는 23 %에서 WSL VM(31 GB, 스왑 8 GB)의 메모리가 바닥나
java(r8)가 OOM 킬러를 불러 ninja가 죽었고, `.wslconfig`로 VM을 48 GB(스왑 16 GB)로 올리고 `OME_JOBS=12`로 네 번째를 돌렸다. 네 번째는 60 %에서 `alsactl.c` 컴파일에 멈췄다. Bliss 매니페스트가 가리키는
android-generic의 alsa-utils(`pie-x86` HEAD)가 2026-06-27에 상류 1.2.16을 합쳐 `snd_lib_log_set_handler`를 부르는데 alsa-lib(`v-x86`)는
1.2.13 그대로라 그 함수가 없다. 상류 Bliss 트리 자체의 어긋남이고, `manifest/ome.xml`에서 alsa-utils를 그 합치기 직전 커밋
3278244c(1.2.13)로 고정해 다섯 번째를 돌렸다(이 스냅샷이 Bliss 매니페스트와 다른 유일한 자리이며 XML 주석으로 적어 두었다). 다섯 번째가 05:14에 끝나 첫 ISO
`Bliss-v18.4-x86_64-UNOFFICIAL-vanilla-20261002.iso`(2.95 GB)가 나왔고, 블롭 검사와 제품 QEMU(WHPX, virgl)로의 라이브 부팅까지
확인했다(`docs/evidence/P3/build-20261002.md`). 4절 3번의 나머지(도우미 설치, 직접 부팅, 데이터 보존)와 4~6번이 남았다. 개발 전용 변환기 추출 도구(3절 5번)는 보류 때 멈췄고 저장소에 남긴
파일은 없으며 4절 4번에서 다시 만든다. 빌드 호스트와 배포, 안드로이드 15 게스트의 adb 승인은 2026-10-02에 사용자가 정해 ADR-0012로 옮겼다(개발 PC 빌드를 사전 릴리스로, 1 GiB 조각, 프로필 `ome-android-15`는 `candidate`, 설치 때 호스트 adb 공개키를 `/adb_keys`에 주입. 5.1절은 (가)로 끝났다). 근거는 같은 날 ASTRA
워커 셋이 만든 조사 사실표 `docs/evidence/P3/research-{bliss-build,google-images,build-host}.md`이고, 이 문서는 그 사실에서
나온 계획과 사용자가 정할 것을 적는다. 결정은 ADR-0012에 있다.

## 1. 조사로 바뀐 전제

CLAUDE.md와 ADR-0006이 2026-09-25~28에 적은 전제 가운데 넷이 사실과 어긋났다. 이 문서와 함께 CLAUDE.md 1.3절, D2, P3,
`docs/ARCHITECTURE.md` 3.15절의 문장을 고쳤다.

1. **BlissOS에는 안드로이드 16·17 PC 트리가 없다.** 현재 트리는 `typhoon-x86`(16, 안드로이드 13), `universe-x86`(17,
   안드로이드 14), `voyager-x86`(18, 안드로이드 15, `android-15.0.0_r14`)이고, `voyager-qpr2-x86`은 `lineage-22.2`
   브랜치(안드로이드 15 QPR2 계열)를 기반으로 한다. **`arcadia-x86`은 BlissOS 15, 곧 안드로이드 12L이다.** 따라서 1차
   자체 이미지는 **안드로이드 15(API 35)**이고, 16과 17 프로필은 상류 트리가 생기면 같은 절차로 더한다.
2. **Bliss 공식 이미지 중단은 2026년 8월 상태 페이지에서도 유효하다.** BlissOS 18 공식 ISO는 없다. 안드로이드 15
   게스트로 가는 길은 자체 빌드뿐이다.
3. **`BLISS_BUILD_VARIANT=foss`는 블롭 없는 변형이 아니다.** 그 값은 microG와 F-Droid 묶음(`vendor/foss`, Voyager
   manifest에는 들어 있지도 않다)을 고르는 것이고, 네이티브 브리지는 `USE_LIBNDK_TRANSLATION_NB`,
   `USE_CROS_HOUDINI_NB`로, 구글 앱은 `USE_EMU_GAPPS`, `USE_OPENGAPPS`로 따로 들어간다. OME의 "블롭 없는 변형"은
   **`BLISS_BUILD_VARIANT=vanilla`에 그 플래그를 모두 끈 빌드**다. ADR-0006과 R3의 문구는 이 뜻으로 읽고, 최종 산출물에
   독점 파일이 없는지는 빌드한 뒤 파일 목록으로 확인한다(`NOTICE-guest.md` 생성과 같은 단계).
4. **설치기는 `bootable/aaropa`(Calamares)로 바뀌었고 무인 모드는 없다.** 그래도 initrd의 `/init`은 226~229행에서
   `/scripts/*`와 `/mnt/$SRC/scripts/*`를 source하므로(2026-10-01 직접 확인) ADR-0010의 도우미 설치 훅은 그대로 쓴다.
   OME가 이미지를 직접 만들므로 도우미 스크립트와 `sgdisk`는 아예 initrd에 넣어 두어 ISO의 `install.sfs`에 기대지
   않는다. 시스템 이미지의 마운트 경로(`/sfs/system.img`)와 A/B 처리는 빌드한 ISO로 다시 확인한다.

조사에서 더 알게 된 것.

- Voyager는 커널 `android-generic/kernel_common` `hm/crimson`(6.6.129, 움직이는 브랜치), Mesa 24.3.3, `USE_EROFS=1`
  (`system.efs`), virtio GPU·네트워크·입력은 모듈, 블록과 PCI는 내장, loop·ext4·erofs·squashfs 내장이다. GPU는 virgl과
  Venus(`vulkan.virtio`)가 있고 gfxstream 게스트 드라이버는 주석뿐이다. P1은 게스트 쪽 드라이버 작업을 포함한다.
- 빌드 입력 가운데 고정되지 않은 것이 셋이다. manifest 브랜치, 커널 브랜치, aaropa의 `download.sh`가 받는
  `aaropa_rootfs/releases/latest`(SHA-256 대조 없음). OME는 셋 다 커밋 또는 릴리스 번호와 SHA-256으로 고정한다.
- Voyager README의 요구 사항은 최신 Ubuntu LTS, RAM 24 GB(VM 30 GB), 디스크 250 GB(소스 약 170 GB), `libncurses5`다.
  AOSP 공식 문서는 RAM 64 GB, 디스크 400 GB를 적는다.
- Digitalis는 AOSP 16 QPR2(`android16-qpr2-release`) 전용이고 다른 플랫폼 릴리스의 묶음 혼용을 지원하지 않는다. 공개
  릴리스나 태그는 없고 자기 트리를 전체 빌드해 `vendor/digitalis/prebuilts/`로 넣는 방식만 있다. 안드로이드 15 이미지에
  넣으려면 ADR-0008 5번대로 플랫폼 층을 API 35에 맞춰 이식해야 하며, 그것은 P2의 일이다.
- 구글 색인에는 API 35 `google_apis` x86_64 r09(`x86_64-35_r09.zip`, SHA-1 `0103e6da…`)가 있고 35개 x86_64 항목 모두
  `translatedAbis=arm64-v8a`다. 최근 이미지의 `system.img`는 GPT 디스크 안의 `super` 동적 파티션이고(ext4와 EROFS가
  섞임), Android-Generic의 추출 도구는 2022년에 멈춰 SDK 32까지만 안다. API 35 묶음에는 기존 이름 패턴이 놓치는
  `libberberis_exec_region.so`가 더 필요하다는 보고가 있다. 안드로이드 15(redroid 15)에 API 35 변환기를 넣어 arm64
  앱을 돌린 보고가 있다.

## 2. 목표와 범위

목표는 셋이다. (1) 블롭 없는 안드로이드 15 x86_64 게스트 ISO를 고정된 입력에서 재현 가능하게 빌드하고 출처 증명과
함께 OME 릴리스로 낸다(R3, ADR-0006, ADR-0007). (2) 그 이미지가 제품의 이미지 프로필로 들어와 마법사의 도우미 설치와
직접 커널 부팅으로 부팅하고 능력 조사를 통과한다(ADR-0004, ADR-0010). (3) 개발 전용 변환기 묶음으로 트릭컬이 안드로이드
15 게스트에서 도는지 확인한다(ADR-0006 4번. 배포하지 않는다).

범위 밖은 둘이다. 제품이 사용자 PC에서 구글 이미지를 받아 변환기를 조립하는 흐름은 8절 9번 결정 뒤에 한다(ADR-0006
3번). gfxstream 게스트 드라이버는 P1이다.

사용자가 던진 질문("순정 안드로이드에도 볼따구는 말랑한가")에 대한 답은 이렇다. 블롭 없는 이미지에는 ARM 변환기가
없으므로 arm64 전용 게임은 아예 실행되지 않는다. 변환기를 넣어야 돌고, 지금 넣을 수 있는 것은 구글 API 35 변환기(개발
전용)뿐이다. 그래서 (3)이 그 답을 내는 단계다.

## 3. 산출물

1. `guest/build/`: `Dockerfile`(Ubuntu 22.04 LTS 기반 빌드 환경. 24.04는 `libncurses5` 문제가 있다), `build-image.sh`
   (repo init → OME 고정 manifest → sync → 오버레이 적용 → `lunch bliss_x86_64-ap4a-userdebug` → `make iso_img`),
   `pins.env`(manifest 커밋, 커널 커밋, aaropa rootfs 릴리스와 SHA-256, Mesa 커밋), `manifest/ome.xml`(`repo manifest -r`
   스냅샷으로 모든 프로젝트를 커밋에 고정), `make-notice-guest.sh`(`NOTICE-guest.md` 기계 생성과 금지 패턴 검사),
   `make-kernel-source.sh`(커널은 GPL-2.0이므로 R4와 같은 소스 묶음을 릴리스에 올린다).
2. `guest/overlay/`: 도우미 설치 스크립트(`99-ome-install`과 `sgdisk`)를 initrd에 넣는 오버레이, 기기 프로필
   (`manifests/device-profile.prop`, D8), 필요하면 `init.rc` 조각. AOSP 파생 파일은 Apache-2.0 변경 표시를 지킨다(R5).
3. 이미지 릴리스: `ome-android-15-x86_64-<날짜>-<커밋>.iso`, `.sha256`, `SHA256SUMS`, `NOTICE-guest.md`, `ome.xml`
   스냅샷, 커널 소스 묶음. 전부 출처 증명. 워크플로 이름은 `guest-image-release.yml`(QEMU 릴리스와 같은 모양,
   사전 릴리스 `guest-android-15-<접미사>`). `manifests/artifacts.json`에 `fetched_by: installer` 항목(URL은 OME 릴리스,
   `allowed_hosts`에 이미 `github.com`이 있다)과 `manifests/images/ome-android-15.json`(`distribution: self_built`,
   `translator: none`, `install: helper_boot`, `status: candidate`).
4. 제품 쪽: `Current` 세대 어댑터(API 35 이상)의 능력 조사 결과 `docs/evidence/P3/images/ome-android-15.md`. 어댑터
   수정이 필요하면 ADR-0004 4번대로 어댑터만 고친다. 마법사 S1.4의 이미지 카드는 이미 여러 프로필을 받는다.
5. 개발 전용: `guest/build/dev-translator/extract-google-translator.sh`(API 35 구글 이미지 zip → GPT/super/EROFS 또는
   ext4 → 6절 배치의 묶음과 `translator.json`), `translator/install.ps1`과 `smoke.ps1`(6절 계약. P2가 Digitalis에 쓸
   것과 같은 스크립트이므로 여기서 먼저 만든다). 결과는 `docs/evidence/P3/game-on-android-15.md`.
6. 문서: ADR-0012(자체 이미지의 빌드 입력 고정, 빌드 호스트, 배포), CLAUDE.md의 정정, `docs/KNOWN_LIMITATIONS.md`
   (안드로이드 15 프로필의 상태), `docs/NETWORK.md`(이미지 내려받기는 기존 GitHub Releases 행에 포함).

## 4. 순서

0. **빌드 호스트 결정(사용자).** 5절.
1. 빌드 환경과 스크립트. 호스트가 어디든 같은 Docker 이미지와 스크립트로 돌게 만든다. 첫 sync 뒤 `repo manifest -r`로
   고정 manifest를 만든다.
2. 첫 빌드(vanilla, 브리지 없음, userdebug). ISO의 내용물 목록으로 독점 파일이 없는지 확인하고 `NOTICE-guest.md`를
   만든다.
3. 개발 PC에서 제품 번들 QEMU(WHPX, virgl)로 부팅. 확인할 것은 virtio-gpu 모듈 적재와 GL, 입력, 오디오, adb, 네트워크,
   `adb root`(userdebug), 도우미 설치(`system.efs` 안의 `system.img` 유무, A/B), 직접 커널 부팅, 재부팅 뒤 데이터 보존이다.
   프로필을 만들고 능력 조사를 돌려 증거를 남긴다.
4. 개발 전용 변환기 묶음을 만들어 6절 계약으로 넣고 트릭컬 시나리오(M0 6번 축약판)를 돌린다. 안드로이드 15에서의
   네이티브 브리지 속성과 측정표를 남긴다.
5. 이미지 릴리스 워크플로와 출처 증명. 릴리스가 생기면 `artifacts.json`과 프로필을 그 자산으로 맞추고, 마법사에서 새
   프로필을 골라 설치하는 완료 실행을 돌린다(게임은 변환기가 없으니 부팅과 능력 조사까지).
6. 8절 9번 결정 뒤: 제품의 조립 흐름(S1.4 뒤 라이선스 동의 화면, 내려받기, 추출, 6절 설치). 이것은 M2 계약 개정이다.

## 5. 사용자가 정할 것

### 5.1 빌드 호스트

조건은 디스크 250 GB(AOSP 공식은 400 GB), RAM 24 GB 이상, 리눅스, 4~6시간. 선택지는 조사 사실표 C에서 나온 셋이다.

| 선택지 | 비용 | 조건 | 출처 증명 |
|---|---|---|---|
| (가) 이 PC의 WSL2 Debian 13 | 돈은 들지 않음 | 지금 여유는 C: 66 GB, D: 94 GB, E: 25 GB라 **디스크를 비우거나 더해야 한다.** `.wslconfig`로 메모리를 48 GB까지 올릴 수 있다 | 개발 PC 빌드라 ADR-0007이 릴리스에 올리지 않는다고 적은 쪽이다. 자체 호스팅 러너로 증명을 붙일 수는 있지만 GitHub가 공개 저장소에서 쓰지 말라고 경고하고 SLSA L2가 아니다 |
| (나) 주문형 클라우드 VM(Hetzner CCX43: 16 vCPU, 64 GB, NVMe 360 GB, €0.44/시간) | 빌드 한 번에 약 €3~5. 계정과 결제 수단은 사용자가 만든다 | VM을 그 빌드 동안만 만든다. 짧은 수명의 자체 호스팅 러너로 등록하면 `release.yml`과 같은 모양으로 `actions/attest`를 붙일 수 있다 | 전용 인프라의 짧은 수명 러너라 개인 PC보다 낫다. ADR-0007을 "이미지는 OME가 띄운 짧은 수명 리눅스 러너에서 빌드한다"로 고쳐야 한다 |
| (다) 둘 다 | (나)의 비용 | 개발 반복(2~4번)은 WSL2에서, 릴리스(5번)만 VM에서 | (나)와 같다 |

**정함 2026-10-02: (가).** 개발 PC 빌드를 증명 없이 사전 릴리스로 올린다(ADR-0012 결정 1). 아래 표는 결정 당시의 비교다.

GitHub 호스트 러너는 디스크(보장 14 GB)와 RAM(16 GB), 잡 6시간 때문에 안 되고, 대형 러너는 Team·Enterprise 조직
전용이라 개인 계정 저장소에서는 쓸 수 없다. R3가 말한 "CI에서 foss 변형의 빌드 성공을 게이트로 둔다"는 (나)의
러너로만 지킬 수 있고, (가)만 고르면 그 게이트는 개발 PC의 수동 빌드 기록으로 대신한다.

### 5.2 안드로이드 15 트리

`voyager-x86`(`android-15.0.0_r14`, AOSP 태그 기준)을 1차로 고른다. `voyager-qpr2-x86`은 LineageOS 22.2 브랜치
기반이라 AOSP 태그에 묶이지 않으므로, 처음 빌드가 끝난 뒤 두 번째 후보로 본다. 사용자가 달리 정하지 않으면 이대로
간다.

### 5.3 8절 9번

이 계획의 1~5번은 8절 9번과 무관하게 진행할 수 있다. 6번만 결정이 필요하다. 조사로 더해진 사실은 Digitalis가 안드로이드
16 전용이라 안드로이드 15 이미지에서는 (나)의 조건(Digitalis가 P2를 통과)이 바로 채워지지 않는다는 것이다.

## 6. 완료 기준(제안)

1. 고정된 입력(`pins.env`, `ome.xml`)에서 `guest/build/`의 스크립트로 블롭 없는 안드로이드 15 ISO가 재현 가능하게
   나오고, 들어간 구성요소의 이름, 출처, 버전, 라이선스가 `NOTICE-guest.md`로 기계 생성되며, R1 패턴에 걸리는 파일이
   ISO에 없다는 검사 기록이 있다.
2. 그 ISO가 제품의 프로필로 마법사에 보이고, 도우미 설치와 직접 커널 부팅으로 부팅해 능력 조사를 통과한다.
   증거는 `docs/evidence/P3/images/ome-android-15.md`.
3. 개발 전용 변환기 묶음으로 트릭컬이 전투까지 간다. 증거는 `docs/evidence/P3/game-on-android-15.md`. 묶음과 이미지는
   배포하지 않는다.
4. 이미지 릴리스에 ISO, SHA256SUMS, `NOTICE-guest.md`, manifest 스냅샷, 커널 소스 묶음이 출처 증명과 함께 올라가고,
   `manifests/artifacts.json`의 항목이 그 자산의 SHA-256을 가리킨다.
5. 빌드 호스트 결정과 ADR-0007의 정정이 ADR-0012에 있다.
