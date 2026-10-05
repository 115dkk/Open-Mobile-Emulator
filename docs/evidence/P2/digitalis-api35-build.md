# P2: Digitalis를 안드로이드 15(API 35) 자체 이미지 트리에서 빌드하는 첫 시도 (2026-10-05)

목적: CLAUDE.md 8절 9번의 (나) 선택지(Digitalis가 6절 스모크와 트릭컬 A/B를 통과하면 기본 변환기로)를 판단할 근거를 만든다.
Digitalis는 Apache-2.0이라 소스에서 빌드하면 블롭이 아니고 이미지에 넣어 배포할 수 있다.

## 결론

지금의 Digitalis 포크는 안드로이드 15 트리에 그대로 넣으면 빌드되지 않는다. Soong 분석 단계에서 두 번 멈췄고 C++ 컴파일에는
이르지 못했다. 빌드 선언의 세대 차이(첫 오류)는 패치로 고쳤다. 두 번째 오류는 A15의 `native_bridge_support`에 libm 프록시
기본 모듈이 없기 때문이며, A15 bionic에 맞춘 libm 프록시와 게스트 지원을 만들어야 넘어간다. 스모크와 트릭컬 A/B는 아직 하지 않았다.

## 조사 (2026-10-05, 고정 커밋)

| 항목 | 값 |
|---|---|
| Digitalis 패키징 저장소 | `DigitalisX64/digitalis` `11062cb09097c47a7d2b9ee292aa05626a821f10`, 기본 브랜치 `android-latest-release` |
| 번역기 포크 | `DigitalisX64/platform_frameworks_libs_binary_translation` `f5f1c90b17d0c1df9e9be41534c653a3e74bfc64` |
| 매니페스트 | `DigitalisX64/manifest` `3969040d9ea562e8c874a57f545beeaf88975bc9`, 기반 `android16-qpr2-release` |
| 상류와의 분기 | merge-base `e5ab6a1be847f9490db91789f4a5af9d0dffa58d`, 그 뒤 694 커밋, 204 파일 |
| API 35 원본과의 차이 | `android-15.0.0_r14`의 번역기 `cf1446707cc8e12aa42c024026d42c95228dea38` 대비 566 파일. A15 → A16 상류 변경만 416 파일 |
| 브리지 | `libberberis_arm64.so`, 제품 목록 `berberis_config.mk`, 활성화 `enable_arm64_to_x86_64.mk`, 배치 경로 74개(`digitalis-artifact-paths.txt`, 소스에 적힌 예정 목록) |
| A15 `native_bridge_support` | `aa99591cb20cf7cd2eadab8d176abf7442c7b674`. A16(`ffd3f17e…`)에 있는 `native_bridge_proxy_libm_defaults`, `libnative_bridge_guest_libm`가 없다 |
| 툴체인 | A15 Soong 기본 `gnu++20`, Clang `clang-r530567`. C++ 표준은 실패 원인이 아니다 |
| 호스트 CPU | CPUID로 SSE4.x, AVX/AVX2, FMA, F16C, AES, CLMUL을 읽고 JIT 경로를 고른다. AVX2 일괄 필수라는 근거는 없다(실측 전) |
| 바이너리 섞기 | 상류 패키징 안내가 소비 제품의 플랫폼이 묶음과 같아야 한다고 적는다. A16 빌드를 복사해 쓸 수 없다 |

## 빌드 시도

| 시도 | 내용 | 결과 |
|---|---|---|
| 1 | local manifest로 번역기 프로젝트만 포크로 바꾸고 `repo sync` | 프로젝트 이름이 바뀌어 `--force-sync` 필요 |
| 2 | `--force-sync`, `lunch bliss_x86_64-ap4a-userdebug`, `m libberberis_arm64` | `unrecognized module type "ndk_translation_package"`(A16 전용 모듈 형식) |
| 3 | `phony_rule`과 `.native_bridge` 의존 방식으로 바꾸는 호환 패치, `build-image.sh modules` | `"libberberis_proxy_libm" depends on undefined module "native_bridge_proxy_libm_defaults"` |

원문은 `module-attempt-1.txt`~`3.txt`, 복구 기록은 `restore-and-check.txt`.

## 덧붙여 발견한 것

- `source build/envsetup.sh`가 Bliss의 `device/generic/x86_64/vendorsetup.sh`를 통해 `bootable/aaropa/download.sh`를 부르고
  `releases/latest`에서 설치기 자산을 다시 받는다. 고정 입력 원칙과 어긋난다. 다시 받은 `install.sfs`, `boot_hybrid.img`는
  `aaropa.sha256`과 맞았다. 모듈 시험에서는 패치(`OME_SKIP_INSTALLER_DOWNLOAD=1`)로 막았고, 이미지 빌드 쪽은 따로 고쳐야 한다.
- R1의 `libberberis*` 패턴은 소스에서 빌드한 Digitalis 산출물도 이름으로 잡는다. 이미지 검사에서 출처가 확인된 Digitalis 산출물을
  독점 파일과 구별하는 규칙이 필요하다(정책 결정).

## 트리 상태

실험 뒤 WSL 트리는 원래 API 35 커밋으로 되돌렸고 local manifest를 치웠다. `dist/`는 손대지 않았다. 바뀐 저장소 파일은
`guest/build/build-image.sh`(`OME_TRANSLATOR=none|digitalis`, `modules` 명령, 실험 상태에서 이미지 작업 거부), `guest/build/README.md`,
`guest/build/manifest/ome-digitalis.xml`, `guest/build/patches/digitalis/**`다.

## 다음

1. A15 bionic에 맞춘 libm 프록시와 게스트 libm(A16 구현을 참고하되 A15 소스와 심볼, 버전 스크립트에 맞춘다).
2. `libberberis_arm64`, 프록시 21개, 실행기, 게스트 라이브러리 빌드와 ELF 검사(`NativeBridgeItf`, DT_NEEDED, 설치 위치).
3. OME 보드에 ARM64 네이티브 브리지 타깃과 ABI 목록을 넣고 전체 이미지 빌드.
4. 6절 스모크, 트릭컬 A/B.

## 두 번째 시도: 모듈 빌드 통과 (2026-10-05 밤)

A15의 libm에 맞춘 프록시와 게스트 지원을 만들었다. A16 구현이 가로채는 함수는 `expf`, `powf` 둘이고 A15 `libm.map.txt`에도 있다.
A15 bionic의 libm 소스와 옵션을 공유 기본 모듈로 옮기고, 게스트용 수학 묶음에서는 두 함수 소스만 빼 스텁이 맡게 했다(A16 바이너리나 구현은
가져오지 않았다). 포크의 생성 스크립트가 A15 내장 Python 3.11에서 문법 오류를 내 따옴표를 고쳤다. 상류 `enable_arm64_to_x86_64.mk`는
`OME_TRANSLATOR=digitalis`일 때만 상속하고 보드에 ARM64 네이티브 브리지 타깃을 더했다(ABI는 `arm64-v8a`만).

| 회차 | 결과 |
|---|---|
| 4 | 생성 스크립트 문법 오류 |
| 5 | `libberberis_arm64` 성공 |
| 6, 7 | `libRS.native_bridge` 모르는 타깃(오래된 Soong 그래프, 다시 만들어 해결) |
| 8 | 게스트 libm의 `expf`, `powf` 중복 심볼 |
| 9, 10, 12 | `BERBERIS_PRODUCT_PACKAGES_ARM64_TO_X86_64` 전체 성공(12회차에서 libm 중복 설치 경고도 해결) |

ELF 검사(`api35-elf-validation.txt`): 상류 배치 경로 74개 모두 있음, 호스트 쪽 x86-64와 `/arm64/` 아래 AArch64, `NativeBridgeItf` 내보내기,
게스트 libm의 `expf@@LIBC`, `powf@@LIBC` 등, 실패 0. 브리지 SHA-256 `e9e8052a0d711bc152565c2d824c5bf6cdcb01636f1db833416b21371c506f25`.
패치는 `guest/build/patches/digitalis/` 아래 일곱 개. 트리 소스는 원래 커밋으로 되돌렸지만 `out/` 안의 실험 산출물은 남아 있어,
`build-image.sh`가 그 상태에서 기본 이미지 작업을 거부한다. 부팅, 스모크, 게임은 아직이다.
