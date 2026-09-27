# ADR-0006: 자체 게스트 이미지는 블롭 없이 배포하고, 구글 블롭은 사용자 PC가 설치 시점에 넣는다

상태: 제안, 2026-09-28. R2, R3, D4의 문장을 바꾸므로 사용자 승인 사항이며, 이 ADR을 담은 PR의
병합을 승인으로 본다. CLAUDE.md 8절 9번(구글 변환기의 사용 범위)은 병합과 별개로 사용자가 정한다.

## 배경

사용자 목표(2026-09-28): 가장 중요한 목표는 소스가 공개되어 있고 새 안드로이드 버전을 가장
빨리 따라가는, 누구나 쓸 수 있는 에뮬레이터다. 트릭컬은 그 목표 가운데 하나다. 소스를 공개하는
이유는 비공개 소스가 늘 의혹을 낳기 때문이다.

이 목표에서는 P3(자체 이미지)가 중심이 된다. Bliss 공개 이미지는 중단되었고(1.3절), 받을 수 있는
공식 ISO 가운데 가장 새것은 2024-10-11의 16.9.7이다. 반면 구글은 새 안드로이드마다 베타 단계부터
에뮬레이터용 x86_64 시스템 이미지를 낸다. 2026-09-28에 구글 SDK 저장소 색인
`https://dl.google.com/android/repository/sys-img/google_apis/sys-img2-5.xml`을 읽었을 때
`system-images;android-37.0;google_apis;x86_64`, `android-37.1`, `android-37.2-beta1`부터
`beta3`, `android-canary-20260909` 같은 x86_64 항목이 있었다. 1.3절대로 API 34~37의 Google APIs
x86_64 이미지에 ARM64 변환기가 들어 있으니, 거기서 변환기를 가져오면 OME의 갱신 속도는 구글이
이미지를 내는 속도를 따른다.

문제는 R3의 셋째 항목이다. R3는 그렇게 만든 이미지를 릴리스 산출물로 배포하면서 들어간 블롭의
목록을 `NOTICE-guest.md`에 적겠다고 했다. 이것은 구글 바이너리의 재배포다. 같은 색인과
`google_apis_playstore/sys-img2-5.xml`에서 x86_64 이미지는 대부분 `uses-license
ref="android-sdk-license"`를 가리킨다. 일부 버전은 `android-sdk-preview-license`를, API 30과 31의
Play 스토어 x86_64 이미지는 `android-sdk-arm-dbt-license`를 가리킨다. 색인에 실린
`android-sdk-license` 전문(2019-01-16 판)에서 확인한 조항은 다음과 같다. 1.1과 3.4는
developer.android.com/studio/terms의 안드로이드 SDK 라이선스 계약(2026-04-28 판)에서도 문구가 같다
(2026-09-28 대조).

- 1.1: SDK는 "the Android system files, packaged APIs, and Google APIs add-ons"를 명시적으로
  포함한다.
- 3.1: 사용권은 "solely to develop applications for compatible implementations of Android"로
  주어진다.
- 3.4: "you may not copy (except for backup purposes), modify, adapt, redistribute, decompile,
  reverse engineer, disassemble, or create derivative works of the SDK or any part of the SDK."
- `android-sdk-arm-dbt-license`에만 있는 4.7: ARM 명령어 집합 실행 파일을 실행하게 해 주는 이
  소프트웨어는 "for application development and debug only"로 쓴다고 적는다.

`NOTICE-guest.md`를 적는다고 3.4의 금지가 풀리지는 않는다. 지금은 R2 덕분에 OME가 Bliss ISO를
재배포하지 않으므로 이 위험을 Bliss Labs가 진다. 하지만 블롭이 든 자체 이미지를 내는 순간 OME가
직접 지게 된다. 출처가 분명한 부품을 정체성으로 삼는 프로젝트가 재배포 문제로 릴리스나 저장소를
내리게 되면 회복하기 어렵다.

저장소 안에서도 문장이 서로 어긋난다. R2 첫 항목은 설치기가 "구글 자산은 `dl.google.com`"에서
받는다고 적었지만, `docs/NETWORK.md`는 `dl.google.com`을 "image builder only (P3), never the
installed product"로 적었다. 필요한 부품은 이미 있다. R3는 블롭 없는 foss 변형의 빌드 성공을 CI
게이트로 두었고, 6절은 변환기 묶음을 게스트에 넣고 되돌리는 계약을 정했다.

## 결정

1. 구글 에뮬레이터 이미지에서 꺼낸 파일이 든 게스트 이미지는 릴리스(GitHub Releases), 제품 서버,
   미러 어디에도 올리지 않는다. OME가 배포하는 자체 게스트 이미지는 블롭 없는 변형
   (`BLISS_BUILD_VARIANT=foss`)뿐이다. R3의 `NOTICE-guest.md`는 foss 이미지에 든 구성요소의
   라이선스 목록이 된다.
2. 블롭이 필요한 이미지 프로필은 설치 시점에 사용자 PC가 조립한다. 설치기는
   `manifests/artifacts.json`의 `fetched_by: installer` 항목으로 `dl.google.com`에서 구글 에뮬레이터
   시스템 이미지 zip을 받아 SHA-256을 대조한다. 그다음 사용자 PC에서 변환기 묶음(6절 배치와
   `translator.json`)과 GApps를 꺼내 6절 계약으로 foss 게스트에 넣는다. 꺼낸 파일은 OME 홈 아래에만
   있고 어디로도 올라가지 않는다. 관리자가 매니페스트 항목을 더할 때는 받은 zip을 색인
   (`sys-img2-*.xml`)의 SHA-1과 대조하고 그 사실을 `provenance_note`에 적는다.
3. 구글 이미지를 쓰는 조립을 켤지, 켠다면 기본값으로 둘지는 사용자가 정한다(8절 9번). 켜는 경우
   설치기는 구글 이미지를 받기 전에 색인의 `uses-license`가 가리키는 계약 전문을 보여 주고
   사용자가 동의해야 진행한다. 동의 화면은 3.1의 사용 범위를 가리지 않는다. 동의하지 않은
   사용자에게는 foss 이미지와, 6절 스모크를 통과한 경우 Digitalis 묶음만 남는다.
4. 빌드 머신에서 `vendor_google_emu-x86`로 블롭을 넣은 이미지는 개발과 증거 수집에만 쓰고 배포하지
   않는다. `docs/NETWORK.md`의 `dl.google.com` 행은 "설치기(사용자 동의 뒤)와 개발용 이미지 빌더"로
   고친다.
5. 조립 흐름은 6절 계약만 보므로 변환기 공급원을 가리지 않는다. 구글 이미지에서 꺼낸 묶음이든
   Digitalis 묶음이든 같은 `install.ps1`(제품에서는 같은 계약을 구현한 모듈)로 들어간다.

## 결과

- CLAUDE.md의 0절, D4, R2, R3, M3 4번, P2, P3, 6절, 8절과 `docs/NETWORK.md`, `translator/README.md`,
  `docs/KNOWN_LIMITATIONS.md`를 이 ADR에 맞춰 고쳤다.
- 새 안드로이드 버전을 지원하는 일은 foss 이미지 빌드, 매니페스트의 구글 이미지 항목 추가,
  이미지 프로필과 능력 조사(ADR-0004)가 된다. 구글이 베타 이미지를 내면 베타 프로필도 만들 수 있다.
- 설치기에 구글 이미지 내려받기, 계약 동의, 추출, 6절 설치 단계가 생긴다. 첫 실행 마법사의 어느
  단계에 넣을지는 이 ADR을 채택한 뒤 M2 계약 개정에서 정한다.
- 추출을 어디서 할지 정해야 한다(확인 필요). 최근 에뮬레이터 이미지 zip의 `system.img`는 파티션이
  든 디스크 이미지이고 안의 파일 시스템은 ext4 또는 EROFS일 수 있어, 윈도우에서 직접 읽으려면
  파서가 필요하다. 대안은 foss 게스트 자신을 추출 도구로 쓰는 것이다. zip을 게스트로 밀어 넣고
  `adb root`(Bliss 16.9.7에서 동작 확인, `docs/DECISION-root-adb.md`)로 루프 장치에 마운트해 파일을
  복사하면 윈도우 쪽 파서가 필요 없다. 같은 문서대로 `adb root`는 userdebug 빌드의
  `service.adb.root`에 기대므로 foss 이미지도 userdebug로 빌드해야 한다. 게스트 커널의 루프, ext4,
  EROFS 지원은 foss 이미지를 만들 때 확인한다.
- R1은 그대로다. 꺼낸 파일의 이름은 `libndk_translation*`, `GmsCore*`, `Phonesky*` 패턴에 걸리므로
  실수로 커밋되지 않는다.

## 열린 문제: 구글 변환기의 사용 범위 (8절 9번)

사용자 PC 조립은 OME의 재배포 문제를 없애지만, 3.1의 사용 범위 문제는 없애지 못한다. 3.1은
호환 안드로이드용 앱을 개발하는 목적에만 사용권을 주고, ARM 변환을 따로 언급한 4.7은 "application
development and debug only"라고 적는다. 게임 실행은 이 범위에 들지 않는다. 같은 문제는 지금의 Bliss
프로필에도 있다. `translator/README.md`의 기록대로 Bliss 16.9.x의 `libndk_translation`도 구글
에뮬레이터 이미지에서 꺼낸 것이기 때문이다. 다만 지금은 사용자가 계약에 동의하는 단계 없이 Bliss가
넣어 둔 파일을 받으므로 문제가 드러나지 않았을 뿐이다.

선택지는 셋이다.

- (가) 구글 변환기를 계속 기본으로 두고, 사용 범위를 동의 화면과 `docs/KNOWN_LIMITATIONS.md`에
  적는다.
- (나) Digitalis가 6절 스모크와 트릭컬 A/B(P2)를 통과하면 기본을 Digitalis로 바꾸고, 구글 변환기는
  계약에 동의한 뒤 고르는 선택지로 둔다.
- (다) 구글 이미지 조립 흐름을 만들지 않고, 블롭이 필요한 새 버전은 Digitalis가 준비될 때까지
  foss 이미지로만 낸다.

이 문서는 계약 문면과 확인한 사실만 적은 것이며 법률 자문이 아니다. 이 결정 뒤로 Digitalis(P2)는
성능 비교 대상에 그치지 않는다. Digitalis는 AOSP Berberis를 수정한 Apache-2.0 저장소이고, README는
바이너리 묶음이 구글 비공개 파일 없이 만들어진다고 적는다(2026-09-28 확인). 후보 변환기(구글,
인텔, Digitalis) 가운데 구글 SDK 계약의 사용 범위 문제가 없는 것은 Digitalis뿐이다.
