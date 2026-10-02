# 작업 지시서: Open Mobile Emulator (윈도우용 자체 안드로이드 에뮬레이터)

이 파일은 저장소 루트의 `CLAUDE.md`로 두고 Claude Code가 항상 읽게 한다. 마일스톤 순서, 완료 기준, 금지 사항, 법적 제약을 공학 명세로 옮긴 규칙이 전부 여기에 있다. 이 문서와 어긋나는 작업을 하기 전에는 반드시 사용자에게 묻는다.

문서 작성일: 2026-09-25. 아래 "확인된 사실" 절의 내용은 이 날짜 기준으로 웹에서 검증한 것이며, 그 밖의 외부 저장소나 도구에 관한 주장은 작업 시점에 다시 확인한 뒤 쓴다.

개정 2026-09-25 (같은 날 오후): 사용자 결정으로 제품 이름, 라이선스, M0 하네스, APK 출처를 확정했다. 변경된 항목은 D6, R5, R6, 8절과 새로 넣은 9절이다.

개정 2026-09-28 (PR #1로 병합): 사용자가 밝힌 최우선 목표를 0절에 적고, 자체 이미지 배포 방식(`docs/adr/0006-guest-image-distribution.md`)과 릴리스 빌드 출처 증명(`docs/adr/0007-release-build-provenance.md`)을 반영했다. 변경된 항목은 0절, D4, R2, R3, M3, P2, P3, 6절, 8절 9번이다.

개정 2026-09-28 (같은 날): 안드로이드 9(Pie)를 1차 목표에서 빼고, 번역기의 공개 원칙을 ABI 기준으로 세웠다(`docs/adr/0008-drop-pie-abi-based-translator-policy.md`). 변경된 항목은 D2, D4, M3 4번, P2, 6절, 8절 9번이다.

개정 2026-09-30: 코드 서명을 하지 않기로 했다(사용자 결정, 인증서 비용을 대는 후원이 생기면 그때 한다). 변경된 항목은 M3 2번, 8절 3번이다.

개정 2026-10-01: 운영체제 설치는 사용자의 승인을 받아 제품이 무인으로 수행한다(사용자 결정). 원래 요구의 "설치기 조작은 사용자가 한다"는 사용자가 설치를 **승인**한다는 뜻이었는데, M2 명세가 사용자가 설치기 화면을 직접 조작한다는 뜻으로 오독해 지금의 S1.5는 설치기 화면과 안내를 그대로 보여 사용자에게 GRUB와 파티션 작업을 시킨다. 설치기 화면을 그대로 보여 주면 불안한 사용자가 화면을 조작해 실패를 만든다. 변경된 항목은 M1 3번의 주석, M2 기능 명세 1번과 9번, M2 완료 기준이고, `docs/M2-SCREENS.md` S1.5의 규칙을 고쳤다. 완료 실행 드라이버(`ci/dod/run-dod.mjs`)가 OCR과 키로 설치기를 넘기는 것은 시험 장치이지 제품의 길이 아니다.

개정 2026-10-01 (P3 시작): 사용자가 출시 후 트랙 순서를 P3 → P1 → P2로 정했다(8절 6번). P3 조사(`docs/evidence/P3/research-*.md`, `docs/P3-PLAN.md`)로 사실 셋을 정정했다. BlissOS의 `arcadia-x86`은 안드로이드 16이 아니라 12L이고 안드로이드 16·17 트리는 없으므로 1차 자체 이미지는 Bliss 18(`voyager-x86`, 안드로이드 15)이다. `BLISS_BUILD_VARIANT=foss`는 블롭을 빼는 값이 아니라 microG 묶음을 고르는 값이고 네이티브 브리지와 구글 앱은 별도 플래그다. 설치기는 `bootable/aaropa`로 바뀌었지만 initrd의 `/scripts/*` 훅은 남아 ADR-0010이 그대로 통한다. 변경된 항목은 1.3절, D2, R3, P3, 8절 6번이다.

개정 2026-10-02 (P3 결정): 자체 게스트 이미지는 개발 PC의 WSL2에서 빌드해 사전 릴리스로 올리고(ADR-0007의 예외), 안드로이드 15 게스트의 adb는 설치 때 도우미가 넣은 호스트 adb 공개키로 승인한다(모두 사용자 결정, `docs/adr/0012-self-built-image-release.md`). 변경된 항목은 R3, 4절의 `guest/build/` 줄, P3다.

개정 2026-10-01 (M3 시작): 설치기는 현재 사용자 범위의 NSIS이고 설치 폴더를 여는 단추를 두며, 첫 릴리스 v0.1.0은 사전 릴리스로 올리고, M3 완료 기준을 신설했다(모두 사용자 결정, `docs/adr/0011-installer-and-release-pipeline.md`). 변경된 항목은 M3 1번, M3 완료 기준, 4절의 `installer/`와 워크플로 줄, 8절 10번과 11번이다.

---

## 0. 한 문단 요약

목표는 트릭컬 리바이브(국내판, `com.epidgames.trickcalrevive`)를 비롯한 arm64 전용 서브컬처 모바일 게임을 윈도우 PC에서 돌리는, 출처를 신뢰할 수 있는 부품만으로 조립한 제품화된 에뮬레이터다. 구조는 `WHPX 가속 커스텀 QEMU + Bliss OS x86_64 게스트 + 게스트에 포함된 구글 libndk_translation + 얇은 윈도우 프런트엔드`다. 변환기가 비공개 바이너리라는 점은 알려진 한계로 문서화하고 그대로 출시하며, 오픈소스 변환기(Digitalis)로의 교체는 출시 후 후속 작업이다. 중국 업체가 만든 바이너리는 어느 계층에도 넣지 않는다.

가장 중요한 목표는 소스가 공개되어 있고 새 안드로이드 버전을 가장 빨리 따라가는, 누구나 쓸 수 있는 에뮬레이터이며, 트릭컬은 그 목표 가운데 하나다(사용자 결정 2026-09-28). 소스를 공개하는 이유는 비공개 소스, 특히 무엇을 하는지 아무도 확인할 수 없는 커널 드라이버가 늘 의혹을 낳기 때문이다. 그래서 공개는 소스에서 멈추지 않고 릴리스 바이너리의 출처 증명까지 간다(ADR-0007).

---

## 1. 확인된 사실 (2026-09-25 기준)

Claude Code는 이 절의 사실을 다시 조사하지 않아도 된다. 다만 날짜가 오래되면 갱신을 제안한다.

### 1.1 대상 게임
- 트릭컬 리바이브 APK의 ABI는 `arm64-v8a`(구버전에는 `armeabi-v7a` 병행)이며 x86_64 빌드가 없다. 따라서 ARM 변환 없이는 실행되지 않는다.
- 최소 안드로이드 버전은 8.0 언저리(스토어 표기 기준), Unity 엔진, Google Play 결제 사용.
- LDPlayer, MuMu, BlueStacks가 공식 안내 페이지를 운영하므로 에뮬레이터를 적극 차단하거나 Play Integrity를 엄격히 요구하지 않는다고 판단한다. 구글 플레이 게임즈 PC판에는 없다.
- 에피드게임즈는 2026-09-18 3주년 방송에서 PC 클라이언트 개발을 공식화했고 전담 인력을 꾸렸다. 일정은 미정이다.
- 글로벌판은 Bilibili Game이 퍼블리싱한다. 이 프로젝트의 대상은 국내판뿐이다.

### 1.2 림버스 컴퍼니에서 배운 것
- 스팀 클라이언트에는 2023년 4월부터 BattlEye(커널 안티치트)가 있고, 모바일 빌드를 에뮬레이터에서 돌리는 경로에는 없다. PC 클라이언트가 나와도 에뮬레이터의 존재 이유가 남을 수 있다.
- 2026년 4월 Unity 엔진 업그레이드 뒤 MuMu에서 검은 화면 버그가 났다. GPU 경로는 게임 엔진 갱신을 뒤따라가는 지속 작업이다.

### 1.3 부품
- Bliss OS 공식 빌드는 네이티브 브리지를 포함한다. Bliss 14/15는 인텔 libhoudini, Bliss 16/17은 구글 libndk_translation이며, 둘 다 다른 기기 펌웨어에서 추출한 독점 라이브러리다. 네이티브 브리지는 x86_64-v2 CPU를 요구한다.
- Bliss 16.9.x(안드로이드 13) GApps x86_64 ISO는 SourceForge `blissos-x86` 프로젝트에 보관본으로 남아 있다. 예: `Bliss-v16.9.7-x86_64-OFFICIAL-gapps-20241011.iso`. Bliss 공개 이미지는 현재 중단 상태이고 소스 트리만 갱신 중이다. 정정 2026-10-01: 트리는 `typhoon-x86`(BlissOS 16, 안드로이드 13), `universe-x86`(17, 안드로이드 14), `voyager-x86`(18, 안드로이드 15, `android-15.0.0_r14`)이고 `arcadia-x86`은 BlissOS 15, 곧 안드로이드 12L이다. 안드로이드 16·17 트리는 없다. 공식 이미지 중단은 2026년 8월의 상태 페이지에서도 유효하고 BlissOS 18 공식 ISO는 없다(`docs/evidence/P3/research-bliss-build.md`).
- Android-Generic의 `vendor_google_emu-x86`(GitLab)은 구글 에뮬레이터 이미지에서 libndk_translation과 GApps를 추출해 자체 빌드에 넣는 도구다. 자체 이미지 빌드 단계에서 쓴다.
- 구글 Android Emulator의 Google APIs x86_64 시스템 이미지(API 34~37)에는 Berberis 기반의 비공개 ARM64 변환기가 `libndk_translation.so`로 들어 있다. 순수 AOSP 이미지에는 네이티브 브리지가 없다.
- 상류 AOSP Berberis(Apache 2.0)는 RISC-V 백엔드만 공개했다. 오픈소스 ARM64 백엔드는 Digitalis(`github.com/DigitalisX64/digitalis`, AOSP 16 기준, 바이너리 번들 빌드 제공)가 유일하며 초기 단계다.
- AEHD(Android Emulator Hypervisor Driver)는 2026-12-31 종료 예정이다. 하이퍼바이저는 WHPX로 확정한다.
- 윈도우용 QEMU 배포본(qemu.weilnetz.de)은 GPU 가속이 꺼져 있다고 알려져 있었으나, 2026-09-25에 설치한 배포본 11.1.0(설치기 `qemu-w64-setup-20260811.exe`, 빌드 `v11.1.0-12130-ge470268ff4`)은 `virtio-vga-gl` 장치와 `sdl`, `gtk` 디스플레이, `whpx` 가속, `dsound` 오디오를 모두 포함하고, `-device virtio-vga-gl -display sdl,gl=on`으로 기동해 QMP `query-display-options`가 `gl: on`을 돌려주는 것을 확인했다(`docs/evidence/M1/dist-qemu-probe.md`). 따라서 M0과 M1 실험은 배포본으로 진행할 수 있다. 자체 빌드(`qemu-build/`)는 R4의 소스 묶음과 패치 적용을 위한 릴리스 경로로 유지한다. WHPX + virtio-vga-gl + virglrenderer로 Bliss를 돌린 선례는 2023년부터 있다.
- QEMU 상류에는 rutabaga_gfx를 통한 gfxstream 장치(`virtio-gpu-rutabaga`)가 있으나 문서가 리눅스 호스트 전제다. 윈도우 호스트 gfxstream은 출시 후 과제다.
- EmberbirdOS(`github.com/IamAzmathullaShaikh/EmberbirdOS`)는 같은 설계를 문서화했지만 2026-09-24 기준 어떤 게스트도 빌드하지 못했고 VM도 부팅하지 못했다. 설계 결정과 라이선스 경계 분석만 참고하고 코드는 기대하지 않는다.

---

## 2. 확정된 설계 결정

| ID | 결정 | 바꾸려면 |
|---|---|---|
| D1 | 하이퍼바이저는 WHPX. 커널 드라이버를 만들거나 배포하지 않는다. | 사용자 승인 |
| D2 | 게스트는 x86_64 안드로이드 이미지이고 **이미지 프로필로 교체되는 부품**이다(`docs/adr/0004-replaceable-guest-image.md`, 사용자 요구 2026-09-26 밤). 첫 프로필은 Bliss OS 16.9.x GApps(안드로이드 13) 공식 ISO(M0, M1로 검증). 1차 목표는 안드로이드 13, 15, 16, 17 프로필이며(안드로이드 9는 ADR-0008로 뺐다), 그 뒤로 새 버전이 나오는 대로 프로필을 더해 따라간다. 15는 Bliss 18(`voyager-x86`) 자체 빌드(R3)이고, 16과 17은 상류 트리가 생기면 더한다(정정 2026-10-01: `arcadia`는 안드로이드 12L이라 후보가 아니다. `docs/P3-PLAN.md`). 세대별 차이는 세대 어댑터 하나에만 있고 다른 코드는 API 레벨로 분기하지 않는다. | 사용자 승인 |
| D3 | GPU는 1단계 virglrenderer(OpenGL), 2단계 gfxstream. | 2단계 진입은 사용자 승인 |
| D4 | ARM 변환기는 게스트에 포함된 구글 libndk_translation. 자체 이미지에서는 게스트에 미리 들어 있지 않고 설치 시점에 사용자 PC가 6절 계약으로 넣는다(ADR-0006). 교체 가능한 부품으로 취급하며 `translator/` 계약(6절)을 지킨다. 공개 원칙은 ABI 기준이다(ADR-0008). arm64-v8a는 공개 번역기(Digitalis, P2)를 기본으로 삼는 것이 목표이고, armeabi-v7a와 armeabi는 공개 번역기가 없으므로 비공개 번역기를 쓰며 그 이유를 밝힌다. | 계약 변경은 사용자 승인 |
| D5 | QEMU는 별도 프로세스로 실행하고 QMP 소켓으로 제어한다. QEMU 코드를 링크하거나 복사하지 않는다. | 변경 불가 |
| D6 | 자체 코드 라이선스는 GPL-2.0-or-later. 이유는 상용 에뮬레이터 업체가 이 코드를 가져가 비공개로 최적화해 되파는 일을 막기 위해서다(사용자 결정 2026-09-25). 게스트 오버레이(`guest/overlay/`)와 변환기 번들 템플릿처럼 AOSP 파생 파일은 원 라이선스 Apache-2.0을 유지하며, `or-later` 덕분에 두 부분이 만나도 GPL-3.0 아래에서 결합할 수 있다. | 사용자 승인 |
| D7 | 프런트엔드는 **Tauri 2 + Rust**(사용자 결정 2026-09-26, 근거는 `docs/DECISION-frontend.md` 8절). 지능은 Rust 크레이트가 갖고 Tauri 껍데기와 웹뷰는 얇게 둔다. `unsafe`는 윈도우 전용 크레이트의 ffi 모듈에만 허용하고 나머지 크레이트는 전부 `forbid`한다(`docs/adr/0002-unsafe-policy.md`). M0, M1은 PowerShell 7 스크립트로만 만들었다. | 사용자 승인 |
| D8 | 기기 프로필은 평범한 x86_64 태블릿으로 두고, 그 이상의 위장이나 탐지 회피 기능은 만들지 않는다. | 변경 불가 |
| D9 | v1에는 멀티 인스턴스, 매크로, 스크립트 자동화 기능을 넣지 않는다. 다만 입력 파이프라인은 매크로 같은 편의 기능이 새 이음새 없이 들어올 자리를 남긴다(`docs/adr/0005-input-pipeline-and-overlay.md`, 사용자 요구 2026-09-26 밤). 여러 세대의 게스트를 만들어 두고 골라 시작하는 것은 멀티 인스턴스가 아니다. | 사용자 승인 |

---

## 3. 법적 제약을 옮긴 공학 규칙

이 절의 규칙은 전부 CI나 스크립트로 강제한다. "주의한다"로 끝나는 규칙은 없다.

### R1. 독점 바이너리는 저장소에 들어오지 않는다
- `.gitignore`와 pre-commit 훅, GitHub Actions 검사에 다음 패턴을 넣고 매칭되면 커밋과 PR을 거부한다: `libndk_translation*`, `libhoudini*`, `houdini*`, `libberberis*`(자체 빌드 결과물 포함), `*.sfs`, `*.img`, `*.iso`, `*.qcow2`, `*.vdi`, `GmsCore*`, `Phonesky*`, `libwvdrm*`, `libwidevine*`, `*.apk`, `*.xapk`, `*.apks`.
- 예외는 `tests/fixtures/`의 자체 제작 arm64 hello 바이너리처럼 라이선스가 명확한 것뿐이며, 예외 목록은 `ci/allowlist.txt`에 경로 단위로 적는다.

### R2. 외부 산출물은 설치 시점에 사용자 PC가 내려받는다
- 게스트 ISO, GApps, 변환기는 제품 저장소나 제품 서버에서 재배포하지 않는다. 제품의 GitHub Releases도 제품 서버다. 설치기는 `manifests/artifacts.json`에 적힌 공식 배포처 URL(Bliss는 SourceForge 공식 프로젝트, 구글 자산은 `dl.google.com`)에서 받고 SHA-256을 대조한다.
- 매니페스트 항목은 `name, version, url, sha256, license, provenance_note, fetched_by(installer|builder)` 필드를 가진다. `sha256`이 비어 있으면 CI가 실패한다.
- 미러를 두어야 할 상황이 오면 작업을 멈추고 사용자에게 묻는다.

### R3. 자체 게스트 이미지는 블롭 없이 배포하고, 구글 블롭은 사용자 PC가 설치 시점에 넣는다 (ADR-0006)
- OME가 배포하는 자체 게스트 이미지는 블롭이 전혀 없는 변형뿐이다. 이 문서에서 'foss 변형'은 `BLISS_BUILD_VARIANT=vanilla`에 `USE_LIBNDK_TRANSLATION_NB`, `USE_CROS_HOUDINI_NB`, `USE_EMU_GAPPS`, `USE_OPENGAPPS`를 모두 끈 빌드를 가리킨다(정정 2026-10-01: Bliss의 `foss` 값은 microG 묶음을 고르는 것이지 브리지를 빼는 것이 아니다. `docs/P3-PLAN.md` 1절). 이 변형은 항상 빌드 가능해야 하고, CI에서 이 변형의 빌드 성공을 게이트로 둔다(정정 2026-10-02: 안드로이드 트리 빌드는 GitHub 호스트 러너에서 돌지 않으므로 그 게이트는 개발 PC 빌드 기록 `docs/evidence/P3/build-*.md`로 대신하고, 이미지는 개발 PC 빌드를 사전 릴리스로 올린다. ADR-0012).
- 구글 에뮬레이터 이미지에서 꺼낸 파일이 든 이미지는 릴리스, 제품 서버, 미러 어디에도 올리지 않는다. 안드로이드 SDK 라이선스 계약 3.4가 SDK 구성요소의 복사, 수정, 재배포를 금지하고, 1.1은 SDK에 "Android system files"가 포함된다고 적는다. `NOTICE`를 적는다고 이 금지가 풀리지 않는다.
- 블롭이 필요한 이미지 프로필은 설치 시점에 사용자 PC의 설치기가 `dl.google.com`에서 구글 에뮬레이터 시스템 이미지를 받아(매니페스트 `fetched_by: installer`, SHA-256 대조) 사용자 PC에서 변환기 묶음과 GApps를 꺼내고, 6절 계약으로 foss 게스트에 넣는다. 꺼낸 파일은 OME 홈 아래에만 있다. 이 흐름을 켤지와 기본값 여부는 8절 9번에서 사용자가 정하며, 켜면 설치기는 받기 전에 해당 이미지의 라이선스 전문을 보여 주고 동의를 받는다.
- 빌드 머신에서 Android-Generic의 `vendor_google_emu-x86` 흐름으로 블롭을 넣은 이미지는 개발과 증거 수집에만 쓴다. 그 출력물은 어떤 저장소에도 커밋되지 않고 배포되지 않는다.
- 릴리스 이미지의 `NOTICE-guest.md`에는 foss 이미지에 든 구성요소의 이름, 출처, 버전, 라이선스를 기계 생성으로 적는다.

### R4. QEMU(GPL-2.0) 경계
- 자체 코드는 QEMU를 `Start-Process`/`CreateProcess`로 띄우고 QMP(유닉스 소켓 또는 TCP)와 stdout으로만 대화한다. QEMU 헤더 포함, 정적/동적 링크, 소스 복사 모두 금지.
- 커스텀 QEMU 바이너리를 릴리스에 포함할 때는 같은 릴리스에 정확한 소스(상류 커밋 해시 + 적용한 패치 전부 + MSYS2 빌드 스크립트)를 tarball로 함께 올린다. 이 묶음을 만드는 CI 잡 이름은 `qemu-source-offer`이며, 이 잡이 실패하면 릴리스를 만들 수 없다.
- virglrenderer, libepoxy, SDL2, glib 등 링크되는 라이브러리의 라이선스와 소스 위치를 `THIRD_PARTY.md`에 자동 생성한다.

### R5. 라이선스 표기
- 모든 자체 소스 파일 첫머리에 `SPDX-License-Identifier: GPL-2.0-or-later` 헤더. `LICENSE`(GPL-2.0 원문), `NOTICE`, `THIRD_PARTY.md` 세 파일을 루트에 둔다. CI가 헤더 누락을 검사한다.
- `guest/overlay/`와 `translator/bundle-template/` 아래의 AOSP 파생 파일은 `SPDX-License-Identifier: Apache-2.0`을 유지하고, 수정해 배포할 때는 Apache-2.0의 변경 표시 의무를 지킨다(수정 파일 상단에 변경 사실 표기). 이 두 디렉터리는 헤더 검사에서 Apache-2.0을 허용한다.
- `NOTICE`에 호스트 코드(GPL)와 게스트 오버레이(Apache)가 별개의 저작물임을 적는다.

### R6. 상표
- 제품 이름, 아이콘, 저장소 이름에 Android, Google, Bliss, Trickcal, 에피드게임즈, 게임 캐릭터 이름과 이미지를 쓰지 않는다. 게임 이름은 호환성 목록과 프리셋 파일 이름에서만 텍스트로 쓴다.
- 제품 이름은 **Open Mobile Emulator**(줄여서 OME)다. 사용자가 폴더 이름 `Open Android Emulator`에서 Android만 바꾸라고 정했고, Droid(루카스필름 상표)와 Handset(Open Handset Alliance 연상)을 피해 Mobile을 골랐다(2026-09-25). 저장소 이름은 `Open-Mobile-Emulator`. 문장 안에서 안드로이드 앱을 돌린다고 서술하는 것은 상표의 서술적 사용이므로 허용한다.

### R7. 게임 약관과 안티치트
- 게임 프로세스에 후킹, 메모리 조작, 패킷 조작을 하지 않는다. 기기 프로필은 `ro.product.*`를 평범한 값으로 두는 것 이상으로 손대지 않는다.
- 에뮬레이터 탐지를 회피하는 코드는 만들지 않는다. 탐지로 실행이 막히는 게임은 "미지원"으로 문서화한다.

### R8. 구글 계정과 비인증 기기
- 게스트는 구글 비인증 기기다. 제품은 GSF 안드로이드 ID를 읽어 설정의 Google 계정 절에서 보여 주고, 클립보드에 복사한 뒤 `https://www.google.com/android/uncertified/` 등록 페이지를 연다. 자동 등록이나 인증 우회는 만들지 않는다. 첫 실행 마법사에는 이 단계가 없다(사용자 결정 2026-09-27, 근거는 `docs/M2-SCREENS.md` 10절 4번. 구글 계정 없이도 게임은 게스트 계정으로 돈다).
- Play 스토어와 인앱 결제가 동작하지 않을 수 있음을 같은 설정 절과 문서에 적는다.

### R9. 시스템 설정 변경은 명시적 동의 후에만
- WHPX(`HypervisorPlatform`) 기능 활성화는 사용자가 버튼을 눌러 동의한 뒤에만 수행하고, 재부팅이 필요하면 예고한다.
- `bcdedit /set hypervisorlaunchtype` 계열 명령은 어떤 코드 경로에서도 실행하지 않는다. 커널 안티치트 게임과의 충돌은 문서에 적는다.

### R10. 네트워크와 데이터
- 제품이 접속하는 엔드포인트는 `docs/NETWORK.md`에 전부 적고, 여기 없는 주소로 나가는 코드는 리뷰에서 거부한다. 초기 목록: SourceForge(Bliss ISO), `dl.google.com`(빌더 전용), GitHub Releases(자체 업데이트).
- 원격 측정은 없다. 크래시 로그는 로컬 파일로만 남기고 사용자가 직접 첨부해 보내는 방식으로 한다.

---

## 4. 저장소 구조

```
PRODUCT/
  CLAUDE.md                 이 문서
  LICENSE  NOTICE  THIRD_PARTY.md
  manifests/
    artifacts.json          외부 산출물 URL과 SHA-256 (R2)
    images/<id>.json        게스트 이미지 프로필 (D2, ADR-0004)
    device-profile.prop     게스트 기기 프로필 (D8)
  launcher/                 PowerShell 7 스크립트 (M0, M1)
    OME.Common.psm1         공용 모듈(경로, 매니페스트, QEMU/펌웨어/adb 탐색, QMP, adb 대기)
    Check-Host.ps1  Get-Artifacts.ps1  New-GuestDisk.ps1  Start-Guest.ps1  Stop-Guest.ps1
    Invoke-GuestTest.ps1  Get-GameApk.ps1  Enable-Whpx.ps1  Install-DevTools.ps1
  qemu-build/               MSYS2 빌드 스크립트와 패치 (M1)
    Build-Qemu.ps1  build-qemu.sh  common.sh  make-source-offer.sh  make-third-party.sh
    patches/                QEMU 소스 패치(소스 묶음에 포함)
    pins.env                QEMU 태그와 커밋, MSYS2 패키지 목록 고정
    out/                    빌드 산출물(무시 경로). 저장소 경로에 공백이 있어 실제 작업 트리는
                            %LOCALAPPDATA%\OpenMobileEmulator\qemu-build이고 out은 junction
  guest/
    kernel-cmdline.md       Bliss 부팅 인자 실험 기록
    overlay/                게스트에 넣을 자체 스크립트 (블롭 없음)
    build/                  Bliss 18 트리의 자체 빌드(WSL2 `OME-Build`)와 릴리스 자산 스크립트 (P3, ADR-0012)
  translator/               6절 계약, 설치 스크립트, 스모크 테스트
  host/                     M2 제품 껍데기: Rust 워크스페이스(crates/), Tauri 껍데기(app/), 웹뷰(ui/). 구조는 docs/ARCHITECTURE.md
  ci/release/               릴리스 보조: QEMU 릴리스 받기와 검증, 자산 정리, Tauri 설정 덧판 (M3, ADR-0011)
  ci/
    allowlist.txt  forbidden-patterns.txt  Check-*.ps1  Invoke-AllChecks.ps1  Install-Hooks.ps1
  .githooks/pre-commit      R1, R5 검사 (ci/Install-Hooks.ps1로 활성화)
  .github/workflows/        ci.yml(검사와 테스트), qemu-release.yml(러너 QEMU 빌드와 출처 증명), release.yml(태그에서 설치기 빌드와 릴리스),
                            m2-dod.yml과 m3-release-dod.yml(깨끗한 러너에서 마법사부터 게임까지 완주)
  docs/
    KNOWN_LIMITATIONS.md  NETWORK.md  evidence/M0  evidence/M1 ...
  tests/
    fixtures/               자체 제작 arm64 테스트 바이너리와 APK
```

---

## 5. 마일스톤과 완료 기준

각 마일스톤은 완료 기준(DoD)을 전부 만족하고 `docs/evidence/<M>/`에 증거가 있어야 끝난다. 증거는 스크린샷, `adb logcat` 발췌, 명령 출력 원문, 측정표다. 증거 없이 "동작 확인"이라고 쓰지 않는다. 각 마일스톤에는 중단 조건이 있고, 그 조건에 걸리면 추측으로 우회하지 말고 사용자에게 보고한다.

### M0. 변환 검증: 트릭컬이 libndk 위에서 도는가

질문은 하나다. Bliss 16.9.x의 libndk_translation 위에서 트릭컬의 Unity IL2CPP arm64 코드가 실용 성능으로 도는가. 윈도우 GPU 경로는 아직 다루지 않는다.

작업
1. `manifests/artifacts.json`에 Bliss 16.9.x GApps x86_64 ISO 항목을 만든다. SourceForge `blissos-x86/Official/BlissOS16/`에서 최신 GApps x86_64 파일명을 확인하고 SHA-256을 직접 계산해 기록한다. 파일이 여럿이면 x86_64-v2 대상 일반 빌드(Go, Surface, Zenith 제외)를 고른다.
2. 하네스는 둘 중 빠른 쪽을 쓴다. (a) VirtualBox 7.x: EFI 켬, 4 vCPU, 6~8 GB RAM, 32 GB 이상 VDI, 그래픽은 Bliss 문서의 VirtualBox 안내를 따른다. (b) 배포판 QEMU for Windows + WHPX + 표준 VGA(소프트웨어 렌더링). 기능 검증이 목적이므로 느려도 된다.
3. 라이브가 아니라 디스크 설치로 진행해 데이터가 남게 한다.
4. 부팅 후 확인: `adb shell getprop ro.dalvik.vm.native.bridge`가 `libndk_translation.so`이고 `ro.product.cpu.abilist`에 `arm64-v8a`가 있는지. 없으면 Bliss 설정에서 네이티브 브리지 토글을 찾아 켜고 재부팅한다(토글 위치는 Bliss 16 문서로 확인). `cat /proc/cpuinfo`에서 sse4_2, popcnt가 보이는지도 확인한다(x86_64-v2).
5. 트릭컬 APK 확보는 사용자 본인 기기에서 뽑는 방식을 1순위로 한다: 폰에 `adb` 연결 후 `adb shell pm path com.epidgames.trickcalrevive`로 base와 split 경로를 얻고 `adb pull`, 게스트에 `adb install-multiple base.apk split_config.arm64_v8a.apk ...`. 사용자가 다른 출처를 지정하면 그것을 쓰되 서명 지문 `e44ffeb582e01d533416db1304e5c2e41f3156c0`(SHA-1, 2025년 기준 공개 정보)과 대조한다.
6. 테스트 시나리오: 최초 실행과 리소스 다운로드, 게스트 계정 생성, 튜토리얼 전투, 풀보이스 스토리 1편 재생, 10분 연속 세션, 종료 후 재실행. 각 단계 스크린샷.
7. 측정: `adb shell top -m 10 -n 3`, `adb shell dumpsys gfxinfo com.epidgames.trickcalrevive`, `adb shell dumpsys meminfo com.epidgames.trickcalrevive`, 호스트 CPU/메모리(작업 관리자). 전투 중과 스토리 중 각각. `docs/evidence/M0/metrics.md`에 표로.
8. 구글 로그인 경로 검증: GSF ID를 얻어(루트 가능하면 gservices.db 조회, 아니면 기기 ID 조회 앱) 비인증 기기 등록을 한 번 통과시키고 Play 서비스 로그인이 되는지 본다. 이 단계의 절차를 `docs/GOOGLE_ACCOUNT.md` 초안으로 남긴다.

완료 기준
- 트릭컬이 게스트 계정으로 전투와 스토리까지 진행되고, 10분 세션에서 크래시가 없다.
- 위 측정표가 있고, 네이티브 브리지 속성값과 cpuinfo 출력이 증거 폴더에 있다.
- 호스트 사양, 하네스 설정, ISO 파일명과 SHA-256이 `docs/evidence/M0/environment.md`에 있다.

중단 조건
- 네이티브 라이브러리 로드 단계에서 크래시(`logcat`에 `dlopen failed`, `SIGILL`, `ndk_translation` 오류)가 나면 로그를 저장하고 사용자에게 보고한다. 사용자 승인 후 Bliss 15.9.x(libhoudini) ISO로 같은 절차를 한 번 더 한다. 둘 다 실패하면 M1로 가지 않는다.
- 로그인 서버가 기기를 거부하는 문구가 나오면 우회를 시도하지 말고 보고한다(R7).

### M1. 윈도우 실행 경로: WHPX + virgl QEMU

작업
1. `qemu-build/build-qemu.sh`: MSYS2 UCRT64에서 QEMU를 빌드한다. 상류 안정 태그를 `pins.env`에 고정한다. 패키지 목록과 configure 옵션(`--target-list=x86_64-softmmu --enable-whpx --enable-virglrenderer --enable-opengl --enable-sdl --enable-slirp` 등)은 MSYS2의 현재 패키지 이름을 조회해 확정하고 스크립트에 주석으로 출처를 남긴다. 빌드 산출물과 함께 R4의 소스 묶음을 만드는 스크립트를 같은 디렉터리에 둔다.
2. `launcher/Check-Host.ps1`: 가상화 지원, `HypervisorPlatform` 기능 상태, 빌드된 QEMU 존재, OVMF 펌웨어 존재, 디스크 여유를 점검하고 표로 출력한다. 아무것도 바꾸지 않는다.
3. `launcher/New-GuestDisk.ps1`: qcow2를 만들고 ISO로 부팅해 Bliss 설치기로 EFI+ext4 설치를 진행하게 한다(M1의 개발 스크립트 단계라 설치기는 사람이 손으로 넘겼고 스크립트는 안내와 실행만 했다. 제품의 요구는 사용자 승인 뒤 무인 설치다. 개정 2026-10-01).
4. `launcher/Start-Guest.ps1`: 다음을 출발점으로 삼고 실험 결과를 `guest/kernel-cmdline.md`에 기록한다.
   - `-accel whpx,kernel-irqchip=off`
   - `-machine q35 -m 6144 -smp 4`
   - CPU 모델: 기본 `qemu64`는 x86_64-v2에 못 미친다. `-cpu max`와 SSE4.2/POPCNT를 포함하는 명명 모델(`Skylake-Client` 등)을 WHPX에서 순서대로 시험하고 게스트 `/proc/cpuinfo`로 확인한다.
   - `-device virtio-vga-gl -display sdl,gl=on,show-cursor=on`
   - `-device virtio-net-pci -netdev user,id=n0,hostfwd=tcp::5555-:5555`
   - `-usb -device usb-tablet -device usb-kbd`
   - `-audiodev dsound,id=snd0 -device intel-hda -device hda-duplex,audiodev=snd0` (실패 시 sdl audiodev)
   - `-drive file=guest.qcow2,if=virtio -bios <OVMF>`
   - Bliss 부팅 인자에서 `HWC`, `GRALLOC`, `video=` 조합은 Bliss QEMU 설치 문서와 support 이슈를 참고해 virgl에서 마우스 커서와 UI가 정상인 조합을 찾는다.
5. QMP 소켓을 열어 `query-status`, `screendump`, `system_powerdown`, `input-send-event`가 되는지 확인한다. M2의 키 매핑은 `input-send-event`로 구현할 예정이므로 지연 시간을 측정해 둔다.
6. `launcher/Invoke-GuestTest.ps1`: `adb wait-for-device`, `sys.boot_completed` 대기, 앱 설치, 실행, 스크린샷 수집을 자동화한다.
7. M0의 테스트 시나리오와 측정을 반복한다.

완료 기준
- 커스텀 QEMU가 재현 가능한 스크립트로 빌드되고, 소스 묶음(R4)이 같이 나온다.
- WHPX 가속과 virgl GL이 켜진 상태에서 Bliss가 부팅하고 트릭컬이 M0과 같은 시나리오를 통과하며, 전투 프레임과 호스트 CPU가 M0 하네스와 같거나 낫다.
- 오디오, 마우스 절대 좌표, 키보드, adb, 재부팅 후 데이터 보존이 모두 된다.
- 위 사항을 인텔 내장 GPU와 외장 GPU 각각 한 대 이상에서 확인하거나, 확인하지 못한 조합을 `KNOWN_LIMITATIONS.md`에 적는다.

중단 조건
- virgl 초기화 실패(`virgl_renderer_init` 오류, 검은 화면)가 특정 GPU 드라이버에서 재현되면 우회 코드를 만들지 말고 드라이버와 로그를 기록한 뒤 보고한다.
- WHPX에서 특정 CPU 모델이 부팅 불가면 다른 모델로 넘어가되, 모든 모델이 x86_64-v2 미달이면 보고한다.

### M2. 제품 껍데기

프런트엔드 스택은 사용자가 정한다(D7). 후보와 판단 기준을 `docs/DECISION-frontend.md`에 정리해 제시한 뒤 결정을 기다린다. 기준은 윈도우 통합(창 재부모화, 트레이, 알림), 배포 크기, 코드 서명 흐름, Claude Code가 안정적으로 다루는 정도다.

기능 명세
1. 첫 실행 마법사: `Check-Host` 결과 표시 → WHPX 활성화 동의 버튼(R9) → 재부팅 안내 → ISO 내려받기와 SHA-256 대조(R2) → 설치 승인(`설치하기`)과 qcow2 생성과 무인 설치 → 부팅 대기 → GSF ID 표시와 비인증 등록 안내(R8) → APK 설치 화면. 설치에서 사용자가 하는 일은 승인뿐이다(개정 2026-10-01). 승인 단추는 다른 설치 마법사들의 관례대로 `설치하기`이고 화면 설명은 "여유 공간이 충분해 설치할 수 있습니다"는 식이다(사용자 결정 2026-10-01, ADR-0010 채택 때). 설치기 화면과 조작은 사용자에게 노출하지 않고 진행 화면만 보인다. 방법은 9번과 ADR-0010에서 정했다. 그 전까지 지금의 대화형 설치 화면은 개발용이다.
2. QEMU 감독: 프로세스 생성, QMP 연결, 비정상 종료 감지와 로그 보관, 정상 종료(`system_powerdown` 후 타임아웃 강제 종료).
3. 창: QEMU SDL 창을 호스트 창 안으로 재부모화하는 방식을 1안, 별도 창 유지 + 오버레이를 2안으로 두고 1안 실패 시 2안으로 간다. 창을 닫으면 기본으로 게스트를 끄고 앱을 종료한다. 트레이로 내리기는 설정이다(사용자 결정 2026-09-27).
4. 입력(사용자 요구 2026-09-26 밤, `docs/adr/0005-input-pipeline-and-overlay.md`): 입력 프로필(JSON, 논리 좌표) → 입력 파이프라인 → QMP `input-send-event`(멀티터치 장치, 없으면 단일 포인터). 다음을 모두 지원한다. 화면 좌표에 키를 지정하는 탭, 키를 누르는 동안 터치를 유지하는 홀드, 방향키와 WASD의 가상 조이스틱, 드래그와 스와이프, 마우스 버튼, 게임별 프로필 저장과 전경 앱에 따른 자동 적용, 해상도와 창 크기가 바뀌어도 어긋나지 않는 화면 비율 기준 좌표, 현재 매핑을 화면 위에 보이고 바로 고치는 오버레이 편집기, 매핑 전체를 잠시 끄는 단축키. 트릭컬 프리셋 1개 동봉. 매크로는 v1에 없으나 자리를 남긴다(D9).
4-1. 게스트 이미지: 마법사와 설정에서 이미지 프로필(`manifests/images/`)을 골라 게스트를 만들고, 첫 부팅 뒤 능력 조사를 돌려 결과를 게스트에 저장한다(D2, `docs/adr/0004-replaceable-guest-image.md`).
5. 표시: 해상도와 DPI 프리셋(1280x720, 1920x1080, 세로 모드)과 사용자 지정 해상도, 게스트 `wm size`/`wm density`와 부팅 인자 `video=` 동기화. 주사율(EDID `refresh_rate`, 기본 60 Hz, 30~240)과 수직 동기화(QEMU SDL 디스플레이 자체 패치, R4 소스 묶음에 포함)도 여기서 고른다(사용자 요구 2026-09-27).
6. 앱 관리: APK/XAPK/APKS 설치(split 처리), 제거, 실행, 게스트 스크린샷 저장.
6-1. 하드웨어 한도(사용자 요구 2026-09-27): 메모리와 vCPU의 상한은 호스트에서 읽는다(물리 메모리 − 4 GiB, 논리 프로세서 수). 고정 상한을 두지 않는다. 기본값은 8 GiB, 4 vCPU.
6-2. 루팅과 adb(사용자 요구 2026-09-27): 설정에서 앱 루트 권한을 켜고 끌 수 있고, adb 주소를 보여 주며 다른 PC에서의 연결은 기본 끔이다. adb 절에는 보안 경고를 둔다. R7과 R8은 그대로다(루트는 게임 프로세스에 손대는 일이 아니다).
7. 업데이트: GitHub Releases 조회, 서명 검증, 사용자가 누를 때만 설치.
8. 진단: 로그 묶음 내보내기(호스트 로그, QEMU stdout, `logcat` 최근 2000줄, 환경 표).
9. 무인 설치(사용자 요구 2026-10-01, 완료 실행 작업이 31회차로 끝난 뒤 시작했다. 방법은 ADR-0010으로 채택했고 검증 스파이크는 2026-10-01에 통과했다. `docs/evidence/M2/unattended-install-spike.md`. 구현은 같은 날 끝났고(크레이트 `ome-guest-install`, `BootMode`, 런타임 설치 상태 기계, S1.5 진행 화면) 완료 실행 33회차가 `설치하기` 뒤 사용자 입력 없이 설치 29초, 첫 부팅 48초로 게임까지 완주했다. `docs/evidence/M2/dod-local.md` 33회차): 원래 요구는 사용자가 설치를 승인하는 것이지 설치기를 조작하는 것이 아니었다(개정 2026-10-01). 설치기 화면을 가리고 제품이 직접 깐다면 화면을 조작할 이유가 없다. 제품이 쓸 수 있는 더 효과적인 설치 방법을 찾아 구현한다. 후보는 (가) 설치기 initrd의 무인 설치 커널 인자(`AUTO_INSTALL`, 조사 결과 무인이 아님)를 QEMU 직접 커널 부팅(`-kernel`, `-initrd`, `-append`)으로 넘겨 GRUB 메뉴와 대화형 설치기를 건너뛰는 것, (나) 호스트가 디스크 이미지를 직접 만드는 것(파티션 표와 ESP와 시스템 파일 복사를 호스트가 하며 ext4 쓰기가 관건), (다) 폴더 설치(FAT나 NTFS 파티션의 폴더에 시스템 파일을 두고 `SRC=`로 부팅), (라) ISO의 커널과 initrd에 제품 스크립트를 이어 붙인 숨은 도우미 부팅으로 설치하고 설치된 게스트는 GRUB 없이 직접 커널 부팅하는 것이며, ADR-0010이 (라)를 골랐다. 고른 방법으로 S1.5를 다시 만든다. 완료 실행 드라이버의 설치기 자동화(OCR과 키)는 그때 걷어낸다.

완료 기준
- 깨끗한 윈도우 11 VM 또는 새 사용자 계정에서 마법사만으로 트릭컬 실행까지 도달한다(개발자 도구 없이).
- 설치 단계는 사용자 입력 없이 진행 화면만으로 끝난다(9번, 2026-10-01 추가).
- R9, R10 위반이 없음을 코드 검색으로 확인한 기록이 있다.

### M3. 출시

1. 설치기: Tauri 번들러의 NSIS 설치기, 현재 사용자 범위(`%LOCALAPPDATA%\Open Mobile Emulator`, UAC 없음. 사용자 결정 2026-10-01, ADR-0011). MSIX는 서명 없이는 설치되지 않아 뺐다. 설치 경로에 QEMU 바이너리, OVMF, 승격 도우미(`ome-setup.exe`), 제품만 포함하고 게스트 이미지는 포함하지 않는다(R2). QEMU와 도우미는 릴리스 빌드 때만 쓰는 설정 덧판 `ci/release/tauri.release.conf.json`으로 동봉한다. 사용자가 `%LOCALAPPDATA%` 아래를 찾아가기 어려우므로 설정의 `정보` 절에 `프로그램 폴더 열기` 단추(`open_install_folder`)를 두고, `저장 위치` 절에 스크린샷 폴더 열기와 공유 폴더(PC 파일을 adb로 가상 머신의 `/sdcard/OME/`에 복사하는 `shared_push`, 한 방향)를 둔다(사용자 요구 2026-10-01 오후, ADR-0011 7번). 설치 범위 선택(`both`)은 Tauri 틀에서 UAC 없는 기본값과 양립하지 않아 v0.1.0에 넣지 않는다(ADR-0011 8번).
2. 코드 서명: 하지 않는다(사용자 결정 2026-09-30, 8절 3번). 릴리스는 서명 없이 나가고, SmartScreen 경고를 지나는 방법과 SHA-256·빌드 출처 증명(ADR-0007)으로 파일을 확인하는 절차를 릴리스 노트와 `docs/help/`에 적는다. 인증서 비용을 대는 후원이 생기면 그때 서명 단계를 더한다.
3. R4 소스 묶음, `THIRD_PARTY.md`, `NOTICE`가 릴리스에 포함되는지 CI로 검사한다.
4. `docs/KNOWN_LIMITATIONS.md` 확정. 최소 항목:
   - ARM 변환기는 Bliss 빌드가 포함한 구글의 비공개 바이너리이며 이 프로젝트가 만든 것이 아니다. 오픈소스 교체는 후속 작업이다.
   - 구글이 이 변환기를 싣는 에뮬레이터 이미지의 SDK 라이선스 계약은 3.1에서 호환 안드로이드용 앱을 개발하는 목적에만 사용권을 준다. 게임 실행은 그 범위에 들지 않는다(ADR-0006 열린 문제).
   - 32비트 ARM 앱(armeabi-v7a, armeabi)은 공개 번역기가 없어 비공개 번역기로만 돈다(ADR-0008).
   - GPU 가속은 OpenGL(virgl) 경로뿐이라 Vulkan 필수 게임은 미지원이다.
   - 게스트는 구글 비인증 기기라 Play 스토어와 인앱 결제는 등록 절차를 거쳐도 보장되지 않는다.
   - WHPX를 켜면 윈도우가 하이퍼바이저 위에서 동작하므로 같은 PC의 커널 안티치트 게임과 충돌할 수 있고, 제품은 이 설정을 임의로 바꾸지 않는다.
   - 에뮬레이터를 탐지하는 게임은 지원 범위 밖이다.
   - 확인된 GPU/드라이버 조합 목록과 미확인 조합.
5. 릴리스 노트에 트릭컬 프리셋과 검증한 게임 목록, 호스트 최소 사양(측정치 기반)을 적는다.
6. 빌드 출처 증명(ADR-0007): 릴리스에 올리는 실행 파일(설치기, 제품 exe, 커스텀 QEMU와 동봉 DLL, OVMF)과 R4 소스 묶음은 태그 커밋에서 GitHub Actions 윈도우 러너로만 빌드하고, `actions/attest@v4`로 파일마다 빌드 출처 증명을 붙인다. 개발 PC에서 만든 파일은 릴리스에 올리지 않는다. 확인 명령 `gh attestation verify <파일> --repo 115dkk/Open-Mobile-Emulator`를 릴리스 노트와 `docs/help/`에 적는다. 이 잡이 실패하면 릴리스를 만들 수 없다.
7. 릴리스 구조(ADR-0011): QEMU는 `qemu-release.yml`(잡 `qemu-source-offer`)이 러너에서 빌드해 출처 증명을 붙인 `qemu-<태그>-<접미사>` 사전 릴리스로 따로 내고, 제품 릴리스 `release.yml`은 `manifests/qemu-release.json`이 가리키는 그 릴리스의 런타임 zip과 소스 묶음을 받아 SHA-256과 출처 증명을 확인한 뒤 설치기에 넣고 같은 릴리스에 다시 올린다. 설치기 자산은 `Open-Mobile-Emulator-<버전>-x64-setup.exe` 하나와 그 `.sha256`이다. 첫 릴리스 v0.1.0은 사전 릴리스로 올린다(사용자 결정 2026-10-01). 사전 릴리스는 제품의 자동 업데이트가 보지 않는다.

상태(2026-10-01): M3 완료. 1~7번을 구현했고 첫 사전 릴리스 v0.1.0이 태그 e79c270에서 러너로 빌드되어 올라갔다(`docs/evidence/M3/release.md`). 완료 기준 1, 3, 4는 그 릴리스로, 2는 개발 PC의 새 홈(35회차)과 GitHub 러너(`m3-release-dod.yml` 실행 36826094020, 앞선 두 시도는 SourceForge의 HTTP 522 장애로 이미지 내려받기에서 멈췄다)로 증거가 있다. 두 번째 사전 릴리스 v0.1.1(태그 c4a494d, 2026-10-02)은 0.1.0을 쓰면서 나온 창과 표시 문제를 고친 것이며 같은 절차로 올렸다(`docs/release-notes/v0.1.1.md`, `docs/evidence/M3/release.md` 7절).

완료 기준(사용자 승인 2026-10-01)
- 태그 커밋에서 릴리스 워크플로가 설치기, QEMU 묶음, 소스 묶음, SHA256SUMS를 만들고 전부 출처 증명을 붙여 올린다.
- 그 설치기로 깨끗한 계정에 설치해 마법사만으로 게임 실행까지 간다. 완료 실행 드라이버를 설치된 제품에 대고 돌리며, 개발 PC의 새 홈과 GitHub 러너(`m3-release-dod.yml`) 둘 다에서 한다.
- `docs/KNOWN_LIMITATIONS.md` 확정, 릴리스 노트(`docs/release-notes/v<버전>.md`), `docs/help/install.md`의 SmartScreen과 검증 절차가 있다.
- 릴리스에 소스 묶음, `THIRD_PARTY.md`, `NOTICE`가 들었는지 CI가 검사한다(`ci/Check-ReleaseAssets.ps1`).
- 증거는 `docs/evidence/M3/`에 있다.

### 출시 후 트랙 (순서는 사용자가 정한다)

- P1. gfxstream: 윈도우 호스트에서 `virtio-gpu-rutabaga`를 빌드할 수 있는지 조사 스파이크. 리눅스 전제 문서를 윈도우로 옮기는 데 필요한 변경을 목록화하고, 불가하면 대안(ANGLE 위 virgl, Venus)을 평가한다. 결과 문서만 산출하고 코드는 승인 후.
- P2. Digitalis 교체: `translator/` 계약(6절)에 맞춰 Digitalis 바이너리 번들을 설치하는 스크립트를 만들고, libndk와 A/B로 트릭컬 시나리오와 측정표를 비교한다. 성능이나 안정성이 미달이면 결과를 기록하고 기본값은 libndk로 유지한다. ADR-0006 뒤로 Digitalis(Apache-2.0, AOSP Berberis 수정판)는 후보 변환기 가운데 구글 SDK 계약의 사용 범위 문제(3.1)가 없는 유일한 것이기도 하다. Digitalis는 AOSP 16(API 36)에 맞춰 빌드되고 다른 API 레벨의 이미지에 섞어 넣는 것을 지원하지 않으므로, 통째로 포크하지 않고 버전에 묶인 플랫폼 층(게스트 bionic, `native_bridge_support` 프록시)만 OME가 소스에서 API 레벨별로 빌드한다(ADR-0008, 빌드 위치는 확인 필요). 순서는 API 33(지금 게스트, 이 묶음이 생기면 P3를 기다리지 않고 A/B를 시작한다), API 34~37, API 30~32다. Digitalis에는 ARM64 백엔드만 있다.
- P3. 게스트 갱신: Bliss 18(`voyager-x86`, 안드로이드 15) 기반 foss 이미지를 빌드하고, 블롭은 사용자 PC가 설치 시점에 넣는다(R3, ADR-0006). 2026-10-01 시작. 같은 날 저녁 빌드 호스트가 없어 한때 보류했다가, 개발 PC의 쓰지 않던 페도라 파티션(200 GiB)을 지우고 C:를 늘려 여유 400 GB를 확보한 뒤 **이 PC의 WSL2(`OME-Build`, Ubuntu 22.04, C:\WSL)**를 빌드 호스트로 삼아 다시 열었다(사용자 결정). 빌드 스크립트는 `guest/build/`, 계획과 남은 결정은 `docs/P3-PLAN.md`에 있다. 첫 ISO(안드로이드 15, 블롭 없음)는 2026-10-02 새벽에 나왔고 제품 QEMU로 라이브 부팅까지 확인했다(`docs/evidence/P3/build-20261002.md`). 같은 날 사용자가 정했다. 이미지는 개발 PC에서 빌드해 사전 릴리스 `guest-android-15-<날짜>`로 올리고(ADR-0007의 예외. GitHub 자산 한도 때문에 1 GiB 조각으로 나누며 제품이 이어 붙인다), 프로필 `ome-android-15`는 `candidate`로 들어가며, 그 게스트의 adb(`ro.adb.secure=1`)는 설치 때 도우미가 호스트 adb 공개키를 시스템 이미지의 `/adb_keys`에 넣어 승인한다(ADR-0012). 0절의 목표(새 안드로이드를 가장 빨리 따라가기)의 중심이다. 구글은 새 안드로이드마다 베타 단계부터 x86_64 에뮬레이터 이미지를 내므로(2026-09-28 색인에 `android-37.2-beta3`, `android-canary-20260909` 항목), 이 흐름이 갖춰지면 갱신 속도는 구글의 이미지 출시 속도를 따른다. 게스트를 올리게 만드는 것은 앱의 minSdk다. targetSdk는 설치를 막지 않는다. 트릭컬 10644는 targetSdk 36, minSdk 26으로 API 33 게스트에서 돈다(`docs/evidence/M0/findings-20260926.md`). gfxstream 게스트 드라이버도 자체 이미지에 묶인다(`docs/DECISION-gpu-roadmap.md`).
- P4. 다른 게임: 사용자가 지정한 게임마다 APK ABI 확인 → 시나리오 → 프리셋 → 호환성 표 갱신. 탐지 차단 게임은 미지원 표기(R7).
- P5. D9 재검토: 멀티 인스턴스와 매크로는 게임 약관 검토 뒤 사용자 결정.

---

## 6. 변환기 교체 계약 (`translator/`)

이 계약 덕분에 D4의 후속 교체가 부품 교체가 된다.

- 번들 디렉터리 구조는 AOSP 네이티브 브리지 배치를 따른다: `system/bin/<arch>/`, `system/lib/<arch>/`, `system/lib64/<arch>/`, 브리지 라이브러리(`libndk_translation.so` 또는 `libberberis.so` 등), `system/etc/binfmt_misc/`, `system/etc/init/` 스크립트.
- 번들에는 `translator.json`이 있다: `name, version, provider(google|intel|digitalis), android_api, host_arch, guest_abis, bridge_lib, props{ro.dalvik.vm.native.bridge, ro.dalvik.vm.isa.arm, ro.dalvik.vm.isa.arm64, ro.enable.native.bridge.exec, ro.product.cpu.abilist*}, source_url, source_sha256, license`.
- `translator/install.ps1 -Bundle <dir> -Target <guest>`: 게스트의 system을 쓰기 가능하게 마운트하거나 `adb root; adb remount`로 파일을 밀어 넣고 속성을 반영한 뒤 재부팅한다. 되돌리기(`-Uninstall`)를 반드시 구현한다.
- `translator/smoke.ps1`: (1) `tests/fixtures/hello_arm64`(자체 빌드, 소스 포함) 실행 결과 확인, (2) 자체 제작 arm64 전용 테스트 APK 설치와 실행, (2-1) 자체 제작 armeabi-v7a 전용 테스트 APK(`tests/fixtures/arm32-probe`) 설치와 실행(`guest_abis`에 32비트 ABI가 없는 묶음은 "해당 없음"으로 기록, ADR-0008), (3) 트릭컬 시나리오 축약판. 각 단계 결과를 `docs/evidence/translator-<name>-<version>.md`에 기록한다.
- `android_api`가 게스트 API 레벨과 다르면 설치를 거부한다. 버전 불일치는 이 계층에서 가장 흔한 실패 원인이다.
- 설치기가 사용자 PC에서 구글 에뮬레이터 이미지로부터 꺼낸 묶음(ADR-0006)도 이 계약으로 들어온다. 그때 `source_url`은 `dl.google.com`의 이미지 zip 주소, `source_sha256`은 매니페스트의 값, `license`는 색인의 `uses-license` 값(예: `android-sdk-license`)이다.

---

## 7. Claude Code 작업 규칙

- 외부 저장소, 패키지 이름, 문서 위치, 명령 옵션은 쓰기 전에 실제로 조회해 확인한다. 기억으로 쓴 값은 "확인 필요"로 표시한다.
- 모든 마일스톤 산출물에는 증거 폴더가 따른다. 증거가 없으면 완료가 아니다.
- 중단 조건에 걸리면 우회하지 않고 보고한다. 특히 탐지 회피, 인증 우회, 시스템 설정 무단 변경은 어떤 상황에서도 만들지 않는다(R7, R8, R9).
- 내려받는 모든 파일은 매니페스트의 SHA-256과 대조하고, 매니페스트에 없는 URL에서는 받지 않는다(R2, R10).
- 스크립트는 PowerShell 7 기준, 멱등하게 작성한다. 두 번 실행해도 상태가 같아야 한다.
- 커밋은 마일스톤 하위 작업 단위로 작고 자주. 커밋 전 `ci/forbidden-patterns.txt` 검사를 로컬에서 돌린다(R1).
- 게임 관련 상표와 이미지는 저장소에 넣지 않는다(R6). 스크린샷 증거는 `docs/evidence/`에만 두고 릴리스와 README에는 쓰지 않는다.
- 코드와 경로의 제품 이름은 `Open Mobile Emulator`, 식별자는 `OME`(환경 변수 `OME_HOME` 등), 사용자 데이터 디렉터리 기본값은 `%LOCALAPPDATA%\OpenMobileEmulator`다. 이 문서의 `PRODUCT`는 모두 이 이름을 가리킨다.

---

## 8. 사용자가 정해야 할 것

1. ~~제품 이름(R6를 지키는 이름).~~ 정함: Open Mobile Emulator (2026-09-25).
2. ~~프런트엔드 스택(M2 진입 시).~~ 정함: Tauri 2 + Rust, 웹뷰는 React 19 + TypeScript (2026-09-26). 아키텍처는 `docs/ARCHITECTURE.md`, 용어는 `CONTEXT.md`, 결정 기록은 `docs/adr/`.
3. ~~코드 서명 인증서 여부와 예산(M3).~~ 정함 2026-09-30: 서명하지 않는다. 인증서를 살 돈이 없어서이고, 비용을 대는 후원이 생기면 그때 한다. 무료 후보는 SignPath Foundation(OSI 라이선스의 오픈소스 프로젝트에 OV 서명을 무료로 제공. 이미 낸 릴리스가 있어야 신청할 수 있고, 팀 전원의 2단계 인증과 홈페이지의 서명 정책 게시가 조건. 2026-09-30 확인)이며, 첫 릴리스 뒤 신청할지는 사용자가 정한다. 후원 창구는 사용자가 페이트론 계정을 만든 뒤 `.github/FUNDING.yml`에 `patreon: <계정>` 한 줄로 넣는다.
4. ~~트릭컬 APK 출처~~ 정함: 본인 기기에서 `adb pull` (2026-09-25).
5. ~~M0 하네스~~ 정함: 배포판 QEMU + WHPX + 표준 VGA (2026-09-25). 이 PC는 Hyper-V가 켜져 있어 VirtualBox도 WHPX 위에서만 돌고, VirtualBox는 설치되어 있지 않다. WHPX(`HypervisorPlatform`) 활성화는 사용자가 동의했고 Claude가 UAC 승격으로 한 번 실행하며, 재부팅은 사용자가 한다(R9).
6. ~~출시 후 트랙 P1~P5의 우선순위.~~ 정함 2026-10-01(M3 완료 직후): **P3 → P1 → P2** 순서로 간다. P4와 P5는 그 뒤에 다시 정한다. P3의 빌드 호스트는 같은 날 저녁 개발 PC의 WSL2로 정했다(페도라 파티션을 지워 디스크를 확보, P3 항목 참조). 8절 9번(구글 변환기의 사용 범위)은 P3 안에서 따로 정한다.
7. ~~M2 화면의 여섯 가지 질문(`docs/M2-SCREENS.md` 10절)~~ 정함 2026-09-27: 아이콘 레일, fps 기본 숨김, 창 닫으면 게스트 끄기(트레이는 설정), 구글 안내는 첫 실행에서 제외, F12, 이미지 카드는 최신이 앞이고 알파가 아닌 최신이 기본.
8. D8을 다시 열지(다른 상용 에뮬레이터처럼 인증 기기 지문을 쓸지). 현재는 변경 불가 그대로이며 사용자가 열기 전에는 손대지 않는다.
9. 구글 변환기의 사용 범위(ADR-0006 열린 문제). 안드로이드 SDK 라이선스 계약 3.1은 앱 개발 목적에만 사용권을 준다. (가) 구글 변환기를 기본으로 두고 사실을 동의 화면과 문서에 적는다, (나) Digitalis가 P2를 통과하면 기본을 바꾸고 구글 변환기는 동의 뒤 고르는 선택지로 둔다, (다) 구글 이미지 조립 흐름을 만들지 않는다 가운데 고른다. 정하기 전에는 R3의 사용자 PC 조립 흐름을 구현하지 않는다. 어느 쪽을 골라도 32비트 ARM 앱은 비공개 번역기에 기대므로(ADR-0008), 이 선택은 arm64의 기본 번역기를 정하는 문제다.
10. ~~설치기 설치 범위~~ 정함 2026-10-01: 현재 사용자 범위(UAC 없음). 대신 설정에 `프로그램 폴더 열기` 단추를 둔다. 모든 사용자 선택지는 필요할 때만 승격하는 사용자 정의 NSIS 틀로 가능하지만 틀 복제본의 유지 비용 때문에 같은 날 오후 다시 두지 않기로 했다. ADR-0011 8번.
11. ~~첫 릴리스의 표시~~ 정함 2026-10-01: v0.1.0은 사전 릴리스로 올린다. 정식 릴리스로 바꾸는 시점은 사용자가 정한다.

---

## 9. 개발 호스트 (2026-09-25 확인)

| 항목 | 값 |
|---|---|
| CPU | Intel Core i5-12600KF, 10코어 16스레드 |
| RAM | 64 GB |
| GPU | NVIDIA GeForce RTX 2080 SUPER, 드라이버 32.0.16.1664 |
| OS | Windows 11 Education 22621 |
| Hyper-V | 켜짐(하이퍼바이저 동작 중). `HypervisorPlatform`은 2026-09-25 13:12에 `dism.exe`로 켬(종료 코드 3010, 재부팅 필요). PowerShell 7의 `Enable-WindowsOptionalFeature`는 이 PC에서 0x80040154로 실패하므로 dism.exe를 쓴다 |
| 도구 | git 2.55, gh 2.96, PowerShell 7.6.6, adb 37.0.1(Android SDK platform-tools), NDK 27/28, build-tools 35/36, JDK 17, winget |
| 설치함(2026-09-25) | QEMU 11.1.0 → `C:\Program Files\qemu` (winget `SoftwareFreedomConservancy.QEMU`), MSYS2 20260611 → `C:\msys64` (winget `MSYS2.MSYS2`) |
| M2 툴체인(2026-09-26 확인) | Rust 1.97.0 MSVC(rustup stable), tauri-cli 2.11.4(`cargo tauri`), cargo-deny 0.20.2, Node 24.1.0, npm 11.12.1, VS Build Tools 2022/2026 + Windows 11 SDK 26100, WebView2 런타임 153(윈도우 내장). `host/target`과 `host/ui/node_modules`는 무시 경로다. 저장소 경로의 공백이 어떤 도구를 깨면 qemu-build처럼 junction으로 `%LOCALAPPDATA%\OpenMobileEmulator` 아래로 뺀다 |
| 없음 | VirtualBox |
| WHPX 확인 사항(2026-09-25 저녁) | `-cpu max`는 쓰지 않는다. WHPX에서는 QEMU가 게스트 CPUID를 자기 모델로 답하고 `max`는 TCG 정의(AMD 벤더)라 Bliss가 init 배너 뒤에서 멈춘다. 런처 기본값은 `Skylake-Client-v4`. 게스트 주도 리셋(재부팅, QMP `system_reset`)은 QEMU 11.1.0 WHPX에서 xsave 상태 오류로 VM이 멈추므로 런처는 `-action reboot=shutdown`으로 QEMU를 끝내고 다시 띄운다. 표준 VGA에서는 `nomodeset HWACCEL=0` 항목만 부팅한다. 근거는 `docs/evidence/M0/guest-install.md` |
| 게스트 메모리(2026-09-26 사용자 결정) | 런처 기본값 8192 MiB, 최대 16 GiB까지 승인. 6144 MiB에서는 트릭컬 전투 중 게스트 여유 메모리가 80~115 MB까지 떨어졌다(`docs/evidence/M0/metrics.md`) |

개발용 도구(QEMU, MSYS2) 설치는 winget으로 하고, 이는 제품의 네트워크 목록(R10)이 아니라 개발 환경 준비다.
