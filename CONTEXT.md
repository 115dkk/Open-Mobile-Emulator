# 도메인 용어

이 파일은 코드, 테스트, CI, 아키텍처 문서가 함께 쓰는 낱말을 고정한다. 여기 있는 이름은
모듈 인터페이스의 일부다. 새 구현에서 비슷한 말로 바꿔 쓰지 않는다. 마일스톤과 규칙(R1~R10,
D1~D9)은 `CLAUDE.md`에, 모듈 배치는 `docs/ARCHITECTURE.md`에, 결정 기록은 `docs/adr/`에 있다.

## 제품과 게스트

**Open Mobile Emulator (OME)**
: 제품 이름. 식별자는 `OME`, 사용자 데이터 디렉터리 기본값은 `%LOCALAPPDATA%\OpenMobileEmulator`
(환경 변수 `OME_HOME`으로 바꾼다). 이 디렉터리를 코드에서는 **OME 홈(OmeHome)** 이라 부른다.

**게스트(Guest)**
: 이름이 붙은 Bliss OS 가상 머신 한 대. 디스크(`vm/<이름>/disk.qcow2`), 펌웨어 변수 파일,
메타데이터, 로그로 이루어진다. v1의 게스트 이름은 `default` 하나뿐이다(D9).

**게스트 설정(GuestConfig)**
: 게스트를 어떻게 띄우는지 적은 검증된 값의 묶음. 메모리(MiB), vCPU 수, GPU 모드(`std`,
`virtio`, `virgl`), 가속(`whpx`, `tcg`), CPU 모델, QMP 포트, adb 포트, 오디오와 디스플레이
백엔드. 검증을 통과한 값만 존재하며, 검증되지 않은 문자열은 이 타입이 되지 못한다.

**QEMU 호출(QemuInvocation)**
: 게스트 설정에서 만들어지는 실행 파일 경로와 인자 목록. QEMU 플래그를 아는 곳은 이 모듈
하나다. `-action reboot=shutdown`, `virtio-vga-gl,edid=off`, `Skylake-Client-v4`처럼 증거로
정한 값은 여기에 출처 주석과 함께 있다. 런처 `Get-OmeQemuArguments`의 출력과 인자 단위로
같아야 하며, 그 동일성은 테스트로 지킨다.

**감독자(Supervisor)**
: QEMU 프로세스 하나의 생애를 소유하는 모듈. 시작, 준비(QMP가 응답), 실행 중, 종료 요청,
종료(계획된 종료와 비정상 종료), 게스트 재부팅 뒤 자동 재기동을 상태 기계로 갖는다. QEMU를
링크하거나 소스를 복사하지 않고 프로세스 생성과 QMP, 표준 출력으로만 대화한다(D5, R4).
QEMU 프로세스는 잡 오브젝트에 묶여 제품이 죽으면 함께 끝난다.

**게스트 상태(GuestState)**
: 감독자가 외부에 알리는 상태 값. `Stopped`, `Starting`, `Running`, `Stopping`, `Restarting`,
`Failed`. `Running`은 QMP가 응답한다는 뜻이고 안드로이드가 부팅을 마쳤다는 뜻은 아니다.
부팅 완료는 adb의 `sys.boot_completed`로 따로 안다.

**QMP 채널(QmpChannel)**
: 루프백 TCP로 QEMU와 JSON을 주고받는 클라이언트. 능력 협상, 명령 실행, 이벤트 수신을
갖는다. 제품이 쓰는 QMP 명령은 `query-status`, `screendump`, `system_powerdown`,
`input-send-event`, `send-key`, `query-display-options`, `quit`이다.

**무대(Stage)**
: 웹뷰 안에서 게스트 화면이 놓일 직사각형 자리. 웹뷰는 이 자리의 CSS 픽셀 사각형만 Rust에
알리고, Rust가 배율을 곱해 게스트 창을 그 자리에 맞춘다.

**게스트 창(GuestWindow)**
: QEMU SDL 디스플레이가 만든 최상위 창(HWND). **창 담기(WindowHosting)** 는 이 창을 제품 창의
자식으로 바꿔 무대에 붙이는 일이고, 스타일 전환, `SetParent`, DPI 처리, 크기 맞춤, 분리를
포함한다. 창 담기가 실패하면 별도 창 유지 + 오버레이(2안)로 간다(CLAUDE.md M2 3번).

## 사용자와 대화하는 층

**스냅숏(AppSnapshot)**
: 웹뷰가 그리는 유일한 입력. 호스트 점검 결과, 마법사 단계, 게스트 상태, 앱 목록, 설정,
업데이트 상태, 최근 알림을 한 값으로 묶은 읽기 전용 투영이다. Rust가 만들고 이벤트로 밀어
준다. 웹뷰는 스냅숏에서 권한이나 판단을 끌어내지 않는다.

**명령(Command)**
: 웹뷰가 Rust에 보내는 타입이 정해진 사용자 의도. 이름과 인자가 고정되어 있고 자유 형식
문자열이나 경로, 명령줄을 싣지 않는다. 한 번에 하나만 실행된다(**입장(Admission)**: 실행 중인
명령이 있으면 새 명령은 대기열에 들어가지 않고 `busy`로 거절된다).

**앱 문제(AppIssue)**
: 명령이 실패했을 때 웹뷰가 받는 값. 코드, 사용자에게 보일 한국어 문장, 다음 행동으로
이루어진다. 원문 오류 문자열은 여기 실리지 않고 로그로 간다.

**첫 실행 마법사(FirstRunWizard)**
: 호스트 점검부터 APK 설치까지 CLAUDE.md M2 1번의 여덟 단계를 잇는 상태 기계. 단계는
`HostCheck`, `WhpxConsent`, `RebootPending`, `ArtifactDownload`, `GuestInstall`, `FirstBoot`,
`GoogleRegistration`, `AppInstall`, `Done`. 단계 전이는 순수 함수이고, 각 단계가 시키는 일은
다른 모듈이 한다.

**설정 대리(SetupBroker)**
: 관리자 권한이 필요한 고정 동작을 대신하는 승격 프로세스. v1의 동사는 `enable-whpx`
하나이고 `dism.exe`로 `HypervisorPlatform`을 켠다. 사용자가 버튼을 눌러 동의한 뒤에만 뜨고(R9),
임의 명령줄이나 경로를 받지 않으며, `bcdedit`는 어떤 경로에서도 실행하지 않는다.

## 부품과 자료

**산출물(Artifact)**
: `manifests/artifacts.json`에 적힌, 설치 시점에 사용자 PC가 내려받는 외부 파일(R2).
이름, 버전, URL, SHA-256, 라이선스, 출처 메모, 누가 받는지를 갖는다. **산출물 저장소
(ArtifactStore)** 는 매니페스트를 읽고, 허용 호스트(R10, `docs/NETWORK.md`)를 대조하고,
이어받기와 SHA-256 검증을 하는 모듈이다. 매니페스트에 없는 URL에서는 받지 않는다.

**호스트 점검(HostReadiness)**
: 가상화 지원, `HypervisorPlatform` 상태, 재부팅 대기, QEMU와 펌웨어 존재, adb, 디스크 여유를
읽기만 하는 모듈. 결과는 항목마다 `Ready`, `Attention`, `Blocked` 중 하나와 세부 문장이다.
아무것도 바꾸지 않는다.

**adb 세션(AdbSession)**
: 게스트의 `127.0.0.1:5555`에 붙은 adb 대화. 부팅 대기, 속성 읽기, 앱 목록, 설치(split 포함),
제거, 실행, 스크린샷, logcat 발췌, `wm size`/`wm density`, 미디어 볼륨을 갖는다. adb는 별도
프로세스로 띄운다.

**앱 패키지(AppPackage)**
: 설치할 안드로이드 앱 파일. APK 하나, XAPK(zip 안의 base와 split), APKS(split 묶음)를 한
타입으로 다루고 열어 보기 전에는 설치하지 않는다. 게임 이름은 호환성 항목(`compat/`)과
프리셋 파일 이름에서만 텍스트로 쓴다(R6).

**키 매핑 프로필(KeymapProfile)**
: 키보드 키를 게스트 화면의 절대 좌표 탭이나 스와이프로 바꾸는 JSON 프로필. 트릭컬 프리셋
하나를 동봉한다. 매핑 계산은 순수 함수이고, 실제 전송은 QMP `input-send-event`다. 키 입력
가로채기는 `WH_KEYBOARD_LL` 훅으로 하며 게임 프로세스에는 손대지 않는다(R7).

**표시 프리셋(DisplayPreset)**
: 해상도와 DPI 조합(1280x720, 1920x1080, 세로 모드). 게스트 `wm size`/`wm density`와 부팅
인자 `video=`를 함께 맞춘다.

**진단 묶음(DiagnosticBundle)**
: 호스트 로그, QEMU 표준 출력과 오류, logcat 최근 2000줄, 환경 표를 zip 하나로 묶은 것.
사용자가 직접 첨부해 보낸다. 원격 측정은 없다(R10).

**업데이트(Update)**
: GitHub Releases에서 새 버전을 찾아 minisign 서명을 검증한 뒤, 사용자가 누를 때만
설치한다. 검증은 끌 수 없다.

## 책임 지도

- 웹뷰(React)는 스냅숏을 그리고 명령을 보낸다. 판단, 파일, 네트워크, 프로세스에는 손대지
  않는다.
- Tauri 껍데기(`host/app`)는 창을 만들고, 명령을 받아 입장 검사를 한 뒤 런타임에 넘기고,
  스냅숏을 이벤트로 흘린다. 그 밖의 지능은 없다.
- 런타임(`ome-runtime`)은 스냅숏 투영, 설정 저장, 명령 분배를 소유한다. 껍데기는 이 크레이트
  하나만 부른다.
- 핵심 크레이트들은 각자의 도메인(게스트 설정, 감독자, QMP, 산출물, 호스트 점검, adb, 창
  담기, 키 매핑, 마법사, 진단)을 소유하고 `unsafe`를 금지한다.
- 윈도우 플랫폼 크레이트(`ome-platform-win`)는 Win32 호출을 소유하고, `unsafe`는 그 안의
  ffi 모듈에만 있다. 원시 핸들과 포인터는 이 크레이트 밖으로 나가지 않는다.
- QEMU, adb, dism은 별도 프로세스다. 링크하지 않는다.

## 아키텍처 언어

아키텍처 작업은 **모듈**, **인터페이스**, **구현**, **이음새(seam)**, **어댑터**, **깊이**,
**지렛대(leverage)**, **국소성(locality)** 을 저장소의 표준 뜻으로 쓴다. 인터페이스가 곧
테스트 표면이다. 어댑터가 실제로 둘 이상 달라질 때만 이음새를 만들고, 윈도우 지식을
호출자마다 흩뿌리는 통과 모듈보다 불변식이 한 곳에 머무는 깊은 모듈을 고른다. "컴포넌트",
"서비스", "API", "경계"라는 말은 이 뜻으로 쓰지 않는다.
