# 루팅과 adb 지원 (사용자 요구 2026-09-27)

작성 2026-09-27. 사용자 요구: "루팅 및 adb도 지원해야 합니다. 안 지원하는 제품이 더 희귀하며,
오늘날 휴대폰 루팅이 어려워진 시대에 역설적으로 가치가 높은 옵션입니다. 단, adb의 경우 보안
위험이 있다는 경고를 제시해야 합니다." 이 문서는 조사한 사실, 그 사실이 설계에 준 결과, 남은
확인 항목을 적는다. 화면은 `M2-SCREENS.md` 6절 고급 절, 계약은 `ARCHITECTURE.md` 8.2절과
8.5절이다. 웹 조사는 ASTRA 워커가 2026-09-27에 했고 아래 URL은 모두 그날 접근한 것이다.

## 1. 루팅

### 사실

- Bliss OS 16은 루트를 **KernelSU**로 제공한다. 공식 문서는 KernelSU 관리자 앱이 Bliss OS에
  기본 설치되어 있고, 앱별로 `Superuser` 탭에서 권한을 주면 그 앱이 `su`를 쓸 수 있다고 적는다
  (https://docs.blissos.org/knowledgebase/troubleshooting/disable-touchpad/). LineageOS식
  개발자 옵션 `Root access`나 `persist.sys.root_access` 속성은 공식 문서에 없다(확인 못 함).
- Bliss가 유지하는 KernelSU 소스(`Bliss-Bass/KernelSU` v1.0.1)에서 앱별 권한은
  `/data/adb/ksu/.allowlist`에 저장되고 부팅 때 다시 읽힌다. 재부팅 뒤에도 남는다
  (https://github.com/Bliss-Bass/KernelSU/blob/v1.0.1/kernel/allowlist.c,
  https://github.com/Bliss-Bass/KernelSU/blob/v1.0.1/kernel/ksud.c).
- `adb root`와 앱 루트는 별개다. `adb root`는 userdebug 빌드의 `service.adb.root`로 adbd를
  루트로 다시 띄우는 것이고(https://github.com/mirror/platform_packages_modules_adb/blob/main/docs/dev/root.md),
  이 프로젝트의 Bliss 16.9.7은 userdebug라 M0에서 이미 동작을 확인했다(`docs/GOOGLE_ACCOUNT.md`).
- 2024-09-11 빌드에서 KernelSU 권한을 줘도 앱이 `su`를 못 쓴다는 사용자 보고가 있다
  (https://github.com/BlissRoms-x86/support/issues/99). 이 프로젝트가 쓰는 2024-10-11 빌드에서의
  동작은 확인 못 했다. 2024년 6월 Bliss 글은 KernelSU 0.9.3 이상에서 설치 시스템이 "미설치"로
  보이는 문제와 Zenith 커널의 권한 문제를 따로 적었다(https://blog.blissos.org/bliss-06-updates/).
- 루팅은 Play Integrity 판정에 영향을 준다. 구글은 루팅과 검사에 실패하는 에뮬레이터를
  `MEETS_DEVICE_INTEGRITY` 미달 사유로 적는다(https://developer.android.com/google/play/integrity/verdicts).
  이 게스트는 어차피 비인증 기기라 루트를 켜기 전에도 그 판정은 받지 못한다(R8). 루트를
  켰을 때 구글 로그인이나 결제가 추가로 막히는지는 측정하지 않았다.
- 다른 제품: LDPlayer는 `설정 → 기타 → ROOT 권한 켜기`, 새 설치의 기본값은 끔
  (https://www.ldplayer.net/blog/introduction-to-version-4.0.37-and-3.102-features.html). MuMu는
  설정 센터에 ROOT 스위치가 있고(https://www.mumuplayer.com/help/win/emulator-function-shortcuts.html),
  NoxPlayer는 `Root` 체크박스와 함께 "루팅된 기기에서는 일부 게임이 제대로 돌지 않을 수
  있다"는 경고를 둔다(https://www.bignox.com/blog/introduction-of-system-settings-on-noxplayer/).
  BlueStacks 공식 설정 문서에는 루트 토글이 없다.

### 설계에 준 결과

- 설정의 `루트 권한` 토글은 세대 어댑터가 구현한다(`ARCHITECTURE.md` 3.15절, 명령
  `guest_root_set`). Bliss 16 세대에서의 구현 경로는 KernelSU다. `adb root` 셸에서 KernelSU의
  `ksud`(있다면 `/data/adb/ksud`)로 앱 프로필을 넣거나, 그것이 안 되면 KernelSU 관리자 앱을
  열어 사용자가 앱별로 허용하게 안내한다. 어느 쪽인지는 게스트에서 `ksud`의 존재와 명령을
  확인한 뒤 정한다(2절의 확인 항목).
- 기본값은 끔이다. 도움 문장은 NoxPlayer의 경고를 우리 말로 옮긴 `일부 게임과 결제 기능은
  루트가 켜진 기기에서 동작하지 않습니다.`다.
- 능력 조사 항목 `root`가 KernelSU의 존재와 `su` 동작을 확인하고, 없으면 토글은 없는 버튼이다.
- R7은 그대로다. 루트는 운영체제의 기능을 사용자에게 여는 것이고 게임 프로세스에 손대는 일이
  아니다.

## 2. adb

### 사실

- 이 제품의 adb는 QEMU의 `hostfwd=tcp:127.0.0.1:5555-:5555`로 호스트 루프백에만 열린다. 구글의
  안드로이드 에뮬레이터도 "localhost에서 오는 연결만 받는다"고 적는다
  (https://developer.android.com/studio/run/emulator-console).
- 네트워크에 열린 adb는 실제로 공격받았다. 2018년 2월 ADB.Miner는 TCP 5555를 스캔해
  `adb connect`, `adb push`, `adb shell`로 퍼졌고 하루 최대 약 7,000대가 감염되었으며 한국이
  주요 피해국이었다(https://blog.netlab.360.com/early-warning-adb-miner-a-mining-botnet-utilizing-android-adb-is-now-rapidly-spreading-en/,
  https://blog.netlab.360.com/adb-miner-more-information-en/). 2018년 7월(Satori 변종 추정)과
  2019년 6월(SSH로 재전파하는 채굴 봇넷)에도 같은 경로의 캠페인이 있었고 후자도 한국이 가장
  많았다(https://www.trendmicro.com/en_us/research/18/g/open-adb-ports-being-exploited-to-spread-possible-satori-variant-in-android-devices.html,
  https://www.trendmicro.com/en_us/research/19/f/cryptocurrency-mining-botnet-arrives-through-adb-and-spreads-through-ssh.html).
- PC 에뮬레이터도 예외가 아니었다. BlueStacks 3.0.0~4.31.55는 5555/TCP 디버그 포트가 인증 없이
  연결을 받아 같은 네트워크의 공격자가 VM 셸을 얻고 앱을 설치할 수 있었다(CVE-2018-0701,
  https://jvn.jp/en/jp/JVN60702986/index.html). CCS 2021 논문은 BlueStacks, LDPlayer, MEmu, MuMu,
  NoxPlayer, GameLoop에서 게스트 안의 악성 앱이 adb를 거쳐 권한을 얻는 경로를 실증했고,
  BlueStacks와 LDPlayer만 adb 스위치가 있었으며 다섯 제품의 adb가 루트였다
  (https://diaowenrui.github.io/paper/ccs21-xu.pdf, 4.4절). 사용자가 언급한 "짱깨 에뮬레이터가
  털린 사건" 가운데 NoxPlayer의 2021년 Operation NightScout는 업데이트 서버가 뚫린
  공급망 공격이고 adb와는 무관하다(https://www.welivesecurity.com/2021/02/01/operation-nightscout-supply-chain-attack-online-gaming-asia/).
- 다른 제품의 제시 방식: LDPlayer는 adb를 기본 끔으로 두고 `연결 끊기 / 로컬 연결 열기 /
  원격 연결 열기` 셋 중 고르게 하며 원격에는 "같은 LAN의 다른 컴퓨터가 adb로 에뮬레이터에
  접근할 수 있습니다(권장하지 않음)"라고 적고, 악성 프로그램이 에뮬레이터를 공격하거나 정보를
  훔칠 수 있다고 경고한다. MuMu 12는 브리지 모드에서 게스트 IP의 5555로 LAN 접근을 문서화하고
  경고는 없다(https://mumu.163.com/help/20240703/35047_1164744.html). BlueStacks 5는
  `고급 → Android Debug Bridge` 토글이고 경고는 없다. MEmu는 호스트 127.0.0.1:21503을 게스트
  5555로 넘긴다.

### 설계에 준 결과

- adb는 항상 켜져 있다. 제품 자체가 앱 관리에 쓰고, 루프백에만 열려 있으며, 사용자가 자기
  도구(Android Studio, scrcpy 등)로 붙을 주소 `127.0.0.1:5555`를 설정 고급 절에서 보여 주고
  복사하게 한다.
- 경고는 두 겹이다. 주소 옆 도움 문장 `adb로 연결한 프로그램은 운영체제 안의 모든 앱과
  데이터를 다룰 수 있습니다. 신뢰하는 프로그램만 연결하십시오.`는 CCS 2021이 실증한 경로(같은
  PC의 프로그램이 adb로 게스트를 조작)를 가리킨다. `다른 PC에서 연결 허용` 토글(기본 끔)을 켜면
  `같은 네트워크의 누구나 이 운영체제에 접근할 수 있습니다.` 경고 상자가 뜬다. 이것은
  ADB.Miner와 CVE-2018-0701의 경로다.
- `SettingsView.adb_access`가 `Network`면 `hostfwd=tcp:0.0.0.0:<port>-:5555`로 바뀐다. 다시
  시작해야 적용된다.
- adb 포트를 완전히 끄는 선택지는 두지 않는다. 제품이 같은 포트로 앱 관리를 하므로 끄면 앱
  화면이 동작하지 않는다. 같은 PC의 다른 프로그램을 막으려면 adb 자체의 키 인증
  (`ro.adb.secure=1`)이 필요한데, userdebug Bliss의 값은 2절 확인 항목이다.

## 3. 이 PC의 게스트에서 잰 것 (2026-09-27, 부팅 A)

`docs/evidence/M2/sizing-20260927/bootA-guest.txt`:

- `adb root`는 동작한다(`id`가 uid 0, SELinux 문맥 `u:r:su:s0`). `service.adb.root=1`, `ro.adb.secure=0`,
  `ro.debuggable=0`.
- KernelSU 관리자 앱(`me.weishu.kernelsu`)은 설치되어 있지만 `/data/adb/ksu`, `/data/adb/ksud`, `su`
  바이너리가 전부 없다. 즉 커널 쪽 KernelSU가 살아 있지 않아 **앱 루트는 이 이미지에서 되지 않는다**.
  issue #99의 증상이 2024-10-11 빌드에서도 재현된다.
- `ro.adb.secure=0`이므로 adb에는 키 인증이 없다. 같은 PC의 어떤 프로그램이든 `127.0.0.1:5555`에
  붙으면 루트 셸까지 얻는다. 고급 절의 첫 경고 문장은 이 사실에 맞다.

이 결과가 설계에 주는 것:

- 안드로이드 13(Bliss 16.9.7) 프로필에서 능력 조사 항목 `root`는 `unavailable`이고 `루트 권한`
  토글은 없는 버튼이다. 사용자에게는 설정 고급 절에 `이 운영체제 이미지는 앱 루트 권한을 지원하지
  않습니다. adb 루트는 사용할 수 있습니다.` 한 줄을 둔다.
- 앱 루트가 되는 이미지(KernelSU가 살아 있는 Bliss 빌드, 또는 자체 빌드 R3)가 검증되면 그
  프로필에서만 토글이 나타난다. 커널 쪽 상태(`/sys/module`의 ksu, `dmesg`)는 부팅 C에서 한 번 더
  본다.
- `KNOWN_LIMITATIONS.md`에 적었다.

## 4. 남은 확인 항목

- 커널 쪽 KernelSU의 흔적(`/sys/module`, `dmesg`, 부팅 인자)과 다른 Bliss 빌드(16.9.6, 17.x)에서의 동작.
- 루트를 켠 뒤 구글 로그인과 Play 게임즈 로그인이 그대로 되는지(선택).

측정 결과는 `docs/evidence/M2/sizing-20260927/`의 게스트 캡처와 뒤에 올 `docs/evidence/M2/root.md`에
적는다.
