# ADR-0008: 안드로이드 9 프로필을 1차 목표에서 빼고, 번역기의 공개 원칙은 ABI 기준으로 세운다

상태: 채택, 2026-09-28. D2, D4, 6절, P2를 바꾸므로 사용자 승인 사항이다. 사용자가 이 ADR을 담은
PR을 CI 통과와 충돌 없음을 조건으로 병합하도록 지시했다.

## 배경

ADR-0004는 1차 목표를 안드로이드 9(Pie), 13, 15, 16, 17 프로필로 정했다. 파이를 넣은 이유는
사용자 말로 "상용 에뮬레이터도 아직 파이를 유지하는 곳이 많고(특히 LDPlayer), 아무 이유 없이 옛
버전을 유지하지는 않을 것"이었다(2026-09-28). 사용자는 그 이유가 LDPlayer의 기술 부채일 뿐이라면
파이를 빼는 것도 검토하자고 했다.

상용 에뮬레이터가 파이를 유지하는 이유를 2026-09-28에 확인했다.

- LDPlayer 9(안드로이드 9)는 32비트와 64비트 APK를 모두 돌리고, LDPlayer 14(안드로이드 14)는
  64비트 전용이다. LD9의 공식 최소 RAM은 2 GB이고, LD14는 권장 사양 16 GB가 사실상 최소다. 2026년
  8월 기준 안정 버전은 LD9이고 LD14는 베타다. 매크로와 공략 자료가 LD9 기준으로 쌓여 있다
  (Codersera 비교 기사). LD14 발표(2026-07-13)는 새 버전의 이유로 오래된 안드로이드 환경과 Hyper-V
  충돌을 들었다.
- 블루스택 5.22의 기본 코어는 2021년 5월부터 파이이고, 안드로이드 13은 2026년 7월 기준 베타다.
  MuMu는 안드로이드 15로 옮겼다.
- 국내 사용자가 LD를 고르는 이유로 가벼움과 다중 실행을 드는 글이 반복되고, LD의 2022년 안드로이드
  9 출시 보도자료도 다중 실행을 앞세웠다.

즉 상용 에뮬레이터가 파이를 유지하는 이유는 저사양 PC와 다중 실행, 매크로 생태계, 32비트 앱,
안정 버전이라는 신뢰, 그리고 이식 비용이다. 안드로이드 10의 APEX, system-as-root, 범위 지정
저장소가 루팅, 매크로, 호스트 파일 공유를 위해 시스템을 고쳐 온 업체의 이식 비용을 키웠을 것으로
짐작하지만, 이는 업체가 밝힌 사실이 아니다.

이 가운데 OME에 옮겨 오는 이유는 32비트 앱 하나다.

- 다중 실행과 매크로는 D9가 v1에서 뺐다.
- OME는 WHPX와 virgl을 요구하고, 검증한 게임 하나가 게스트 안에서 PSS 2 GB 남짓을 쓴다
  (`docs/evidence/M0/metrics.md`). 2 GB RAM PC는 처음부터 대상이 아니다.
- 매크로 생태계와 안정 버전 신뢰는 OME가 파이를 고른다고 생기지 않는다.
- 32비트 앱은 파이 없이도 받는다. 지금 게스트(Bliss 16.9.7, 안드로이드 13)의
  `ro.product.cpu.abilist`는 `x86_64,arm64-v8a,x86,armeabi-v7a,armeabi`다
  (`docs/evidence/M0/bridge-props.txt`). 다만 32비트 전용 앱을 실제로 돌려 본 기록은 없다.

그리고 파이는 곧 하한선이 된다. 구글 Play 서비스 릴리스 노트에서 `play-services-basement`와
`play-services-base`의 `minSdkVersion`은 2025-09-10에 23으로, 2026-09-17에 24로 올랐다. 해마다
한 단계씩 오르는 추세가 이어지면 API 28은 2030년 무렵 지원이 끝난다(추세를 연장한 추정이다).
게임 쪽도 최소 버전을 올리고 있다(예: Evony는 2026년 최소 안드로이드 9를 요구했다).

번역기 쪽 사정도 확인했다(2026-09-28, 저장소를 받아 읽음).

- Digitalis는 ARM64 백엔드만 있다. `platform_frameworks_libs_binary_translation` 포크의
  `decoder`, `interpreter`, `lite_translator` 디렉터리에는 `arm64`와 `riscv64`뿐이다. 공개된 ARM32
  → x86 번역기는 확인한 범위에 없다.
- Digitalis가 AOSP에서 고친 저장소는 `frameworks/libs/binary_translation`,
  `device/generic/goldfish`, 자체 저장소 셋이다(`DigitalisX64/manifest`의 `digitalis.xml`, 기반은
  `android16-qpr2-release`).
- 네이티브 브리지 입구는 옛 버전과 맞는다. Digitalis의 `native_bridge.cc`는 인터페이스 버전 2부터
  8까지를 호환한다고 답하고, 콜백 표의 앞부분 순서가 안드로이드 9 `libnativebridge`(버전 4까지)와
  같다.
- 버전에 묶인 것은 게스트 ARM64 bionic과 프록시 트램펄린이다. 이들은 AOSP
  `frameworks/libs/native_bridge_support`에서 호스트 이미지와 같은 플랫폼 소스로 빌드되고,
  Digitalis 문서는 다른 플랫폼 릴리스의 묶음을 섞는 것을 지원하지 않는다고 적는다. AOSP git의
  릴리스 브랜치로 보면 `native_bridge_support`는 안드로이드 11부터, `binary_translation`은
  안드로이드 14부터 있다.
- Digitalis 코드는 APEX 경로(`/apex/com.android.runtime/bin/linker64`,
  `kernel_api/open_emulation.cc`)와 C++20 기능을 쓴다. 안드로이드 9는 APEX 이전이다.

## 결정

1. 1차 목표 프로필은 안드로이드 13, 15, 16, 17이다. 안드로이드 9(Pie)는 1차 목표에서 뺀다.
   상용 에뮬레이터가 파이를 유지하는 이유는 대부분 그들의 고객층(저사양 PC, 다중 실행, 매크로)
   사정이고, OME에 옮겨 오는 32비트 앱은 안드로이드 13 이상의 프로필이 받는다. 사용자 요구가 다시
   생기면 이 ADR을 다시 연다.
2. `ome-guest-image`의 `Legacy` 세대 어댑터(API 29 이하)는 지운다고 얻는 것이 없으므로 남긴다.
   사용자가 옛 이미지를 가져올 때와 능력 조사의 대체 경로에 쓰인다. 다만 이 세대의 이미지 프로필은
   만들지 않는다.
3. 번역기의 공개 원칙은 안드로이드 버전이 아니라 ABI 기준으로 세운다.
   - arm64-v8a 앱은 공개 번역기(Digitalis)를 기본으로 삼는 것이 목표다(P2).
   - armeabi-v7a와 armeabi 앱은 공개 번역기가 없으므로 비공개 번역기(지금은 게스트의
     libndk_translation)를 쓰고, 그 이유를 `docs/KNOWN_LIMITATIONS.md`와 제품 도움말에 밝힌다.
   - 한 게스트의 `ro.dalvik.vm.native.bridge`는 파일 이름 하나지만, 32비트 zygote는 `/system/lib`,
     64비트 zygote는 `/system/lib64`에서 그 이름의 파일을 읽는다. 따라서 64비트는 Digitalis,
     32비트는 비공개 번역기로 나누는 구성이 원리상 가능하다(확인 필요, P2에서 시험한다).
4. 6절 스모크 테스트에 32비트 단계를 넣는다. `tests/fixtures/`에 arm64-probe와 같은 코드를
   armeabi-v7a 전용으로 빌드한 테스트 APK(`arm32-probe`)를 더하고, `translator.json`의
   `guest_abis`에 32비트 ABI가 있는 묶음은 그 단계를 통과해야 한다. 32비트 ABI가 없는 묶음(예:
   Digitalis)은 그 단계를 "해당 없음"으로 기록한다. 픽스처 빌드는 NDK가 있는 개발 PC에서 한다.
5. Digitalis는 통째로 포크하지 않는다. 번역기 본체(JIT, 인터프리터, 게스트 로더)는 상류를 따라가고,
   안드로이드 버전에 묶인 플랫폼 층(게스트 bionic, `native_bridge_support` 프록시)만 OME가
   소스에서 API 레벨별로 빌드한다. 순서는 API 33(지금 게스트, P2 A/B를 바로 시작할 수 있다),
   그다음 API 34~37, 그다음 API 30~32다. 플랫폼 층 작업은 가능하면 상류에 먼저 제안한다. AOSP
   트리 빌드는 GitHub 호스트 러너의 디스크와 시간을 넘을 수 있어 빌드 위치는 확인 필요이며,
   릴리스에 싣는 묶음이라면 어디서 빌드하든 ADR-0007의 출처 증명을 지킬 방법을 함께 정한다.

## 결과

- CLAUDE.md의 D2, D4, P2, 6절, 8절 9번, M3 4번과 ADR-0004 결정 5, `docs/ARCHITECTURE.md` 3.15절,
  `CONTEXT.md`의 게스트 세대, `docs/KNOWN_LIMITATIONS.md`, `translator/README.md`,
  `tests/fixtures/README.md`를 고쳤다.
- 8절 9번의 어느 선택지를 고르든 32비트 ARM 앱은 비공개 번역기에 기댄다. 선택지는 arm64의
  기본 번역기를 정하는 문제로 좁혀진다.
- `arm32-probe` 픽스처와 `translator/smoke.ps1`의 32비트 단계는 아직 없다. 이 ADR은 계약만
  정했다.
- API 33용 Digitalis 플랫폼 층이 생기면 P3(자체 이미지)를 기다리지 않고 지금 게스트에서 P2를
  시작할 수 있다.
