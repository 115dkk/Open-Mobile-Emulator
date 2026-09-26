# M2 프런트엔드 스택 결정 자료 (D7)

조사일 2026-09-26. 이 문서는 CLAUDE.md 5절 M2가 요구하는 후보와 판단 기준 정리다.
결정은 사용자가 내리며(D7), 결정이 나기 전에는 M2를 시작하지 않는다. 버전과
가격, 라이선스 같은 외부 사실은 같은 날 웹에서 확인한 것이고 출처는 마지막 절에
있다. 확인하지 못한 값은 "확인 필요"로 표시했다.

## 0. 요약과 권고

권고는 Qt 6 Widgets(C++)를 1순위, Tauri 2(Rust)를 2순위로 두는 것이다. M2에서 가장
위험한 항목은 다른 프로세스가 소유한 QEMU 창을 제 창 안에 담는 일인데, Qt는 이것을
문서화된 API(`QWindow::fromWinId` + `QWidget::createWindowContainer`)로 제공하고, 윈도우
플랫폼 플러그인 소스에서 스타일 전환과 `SetParent`를 수행하는 코드를 직접 확인했다.
Qt 6.10.1 MSVC 빌드가 이 PC에 이미 있고, 사용자가 EQ APO XT에서 같은 Qt Widgets +
Velopack 조합으로 제품을 배포하고 있으며, Dolphin이 같은 GPL-2.0-or-later + Qt 6
선례다. Qt 부담은 실측 약 36 MB다.

Tauri 2를 고르면 설치기가 30 MB쯤 작아지고, 서명 검증을 끌 수 없는 업데이터가 설정으로
끝나며, 화면을 Opus가 가장 잘 그리는 HTML/CSS로 만들고 Rust 백엔드, 웹 화면, IPC
연결의 세 갈래가 워커 분업에 그대로 들어간다. 대신 창 담기와 좌표 환산을 직접 쓰고,
v2의 권한 체계 때문에 워커 작업서를 더 단단히 써야 한다. .NET(Avalonia, WPF)은 외부
창 컨테이너와 툴체인이 갖춰져 있어 세 번째이고, Electron은 크기 때문에, Wails는 v3
베타와 Go 툴체인 부재 때문에 뒤로 둔다. 그 밖의 후보는 제외한다.

어느 쪽을 고르든 M2의 첫 작업은 같다. 지금 돌고 있는 QEMU 창을 호스트 창에 `SetParent`로
넣어 DPI, 포커스, 크기 변경, 종료 처리를 재는 하루짜리 스파이크다. 스택 결정 전에
PowerShell + P/Invoke로 스택 무관 시험을 먼저 해 볼 수도 있다.

## 1. 판단 기준

CLAUDE.md M2 절이 정한 네 기준을 그대로 쓰고, 이 프로젝트에만 있는 제약 세 가지를
더했다.

| 기준 | 무엇을 보는가 | 출처 |
|---|---|---|
| 윈도우 통합 | 다른 프로세스가 소유한 창(QEMU의 SDL 창)을 제 창 안에 넣는 공식 API가 있는가, 트레이 아이콘, 토스트 알림 | CLAUDE.md M2 |
| 배포 크기 | 최소 앱의 설치 크기. 비교 기준선은 QEMU 묶음 자체의 크기(2.1절) | CLAUDE.md M2 |
| 코드 서명 흐름 | signtool로 exe와 설치기를 서명하는 데 특별한 절차가 붙는가, MSIX 서명이 강제되는가 | CLAUDE.md M2 |
| Claude Code 안정성 | Claude와 GPT 워커가 그 언어와 프레임워크를 얼마나 정확히 다루는가. 측정치는 없고 경험에 근거한 판단 | CLAUDE.md M2 |
| 라이선스 결합 | 자체 코드 GPL-2.0-or-later와 결합할 수 있는가. Apache-2.0 부분은 GPL-3.0 아래에서 결합하며, 이 방식은 D6에서 이미 받아들였다 | D6 |
| 워커 분업 | 백엔드는 Daybreak(Rust, C++, C#), 화면은 Opus 5.5(HTML/CSS를 가장 잘 그림), 연결은 ASTRA. 이 분업에 자연스럽게 들어가는가 | gpt-workers 규칙 |
| 개발 호스트 준비 | 이 PC에 이미 있는 도구로 빌드되는가, 새로 설치할 것이 무엇인가 | 4절 |

D5(QEMU는 별도 프로세스로 띄우고 QMP로만 제어)는 어느 후보든 똑같이 지키므로
기준에 넣지 않았다. 파이썬 기반 스택은 사용자 지시로 처음부터 제외했다.

## 2. M2가 실제로 요구하는 기술

M2 기능 명세 여덟 항목 중 스택에 따라 달라지는 것은 세 가지다. 다른 프로세스 창을
재부모화하는 공식 API가 있는가, 서명 검증이 들어간 업데이터가 내장되어 있는가,
그리고 배포 크기다. 나머지는 어느 스택에서도 같은 Win32 호출과 표준 라이브러리로
해결된다.

| M2 항목 | 필요한 기술 | 스택에 따라 다른가 |
|---|---|---|
| QEMU 감독 | 프로세스 생성과 stdout/stderr 수집, QMP TCP 소켓(현재 런처도 `tcp:127.0.0.1:4444`), 종료 감지, `-action reboot=shutdown` 뒤 자동 재기동 | 아니오. 모든 후보의 표준 라이브러리에 있다 |
| 창 1안(재부모화) | QEMU SDL 창의 HWND를 `SetParent`로 제 창의 자식으로 넣고, 배치가 바뀔 때마다 `MoveWindow`. 프로세스 간 부모-자식은 입력 큐가 결합되고 DPI 인식이 어그러지면 크기가 틀어질 수 있다 | 예. WPF `HwndHost`, Qt `createWindowContainer`, Avalonia `NativeControlHost`처럼 외부 창을 담는 컨테이너가 있는 스택과, 웹뷰 위에서 CSS 픽셀과 물리 픽셀을 직접 환산해 맞춰야 하는 스택으로 나뉜다 |
| 창 2안(오버레이) | QEMU 창은 그대로 두고 투명 레이어드 창을 위에 겹친다 | 아니오 |
| 창 3안(예비) | 커스텀 QEMU에 패치를 넣어 SDL 창을 지정한 HWND의 자식으로 만들게 한다. `qemu-build/patches/`에 들어가고 R4 소스 묶음에 포함된다. 1안과 2안이 막힐 때만 | 아니오 |
| 키 매핑 표시 | 네이티브 자식 창은 그 영역에서 항상 맨 위에 그려지므로(WPF 문서가 airspace라고 부르는 제약) 게임 화면 위의 키 안내는 별도의 투명 레이어드 창으로 그린다 | 아니오. 웹뷰든 XAML이든 Qt든 같은 제약 |
| 키 입력 가로채기 | 제 창이 포커스를 가지고 모든 키를 QMP `input-send-event`로 넘기거나, `WH_KEYBOARD_LL` 훅을 건다 | 아니오. Win32 호출 하나 |
| 트레이, 토스트 | `Shell_NotifyIcon`. 토스트는 비패키지 앱이면 AppUserModelID와 시작 메뉴 바로가기가 필요 | 프레임워크가 감싸 주는지만 다르다 |
| ISO 내려받기 | 2~3 GB 스트림 다운로드, 이어받기, SHA-256 대조(R2) | 아니오 |
| 앱 관리 | adb 호출, XAPK/APKS는 zip 해제 후 `install-multiple` | 아니오 |
| 업데이트 | GitHub Releases 조회, 배포물 서명 검증, 사용자가 누를 때만 설치 | 예. 서명 검증이 들어간 업데이터를 내장한 스택은 이 항목이 설정으로 끝난다 |
| 진단 묶음 | 로그 수집과 zip | 아니오 |

### 2.1 크기 기준선: QEMU 묶음

프런트엔드 크기는 이 숫자와 나란히 놓고 봐야 한다. 2026-09-26에 이 PC에서 직접
재었다.

| 구성 | 크기 |
|---|---|
| 커스텀 QEMU v11.1.1 `bin/` 전체(exe 2개, DLL 24개) | 120 MB |
| 같은 폴더, `qemu-system-x86_64.exe`의 디버그 정보를 벗기면 | 68 MB (exe 83 MB → 29 MB) |
| 배포본 QEMU 11.1.0의 `qemu-system-x86_64.exe` | 25 MB |
| EDK2 펌웨어(code + vars) | 4 MB |

지금 빌드 스크립트는 exe를 스트립하지 않는다(`.debug_info` 32 MB, `.debug_line`
6 MB가 들어 있음). M3 패키징에서 스트립하거나 심볼을 따로 내는 것이 맞고, 이
문서에서는 바꾸지 않는다. 그러므로 실제 기준선은 70 MB 안팎이고, 프런트엔드가
10 MB인지 100 MB인지는 설치 크기가 1.15배가 되는지 2.4배가 되는지의 차이다.
게스트 이미지(ISO 약 2 GB, 디스크 32 GB)는 사용자 PC가 받으므로(R2) 설치기
크기에 들어가지 않는다.

### 2.2 스택과 무관한 플랫폼 사실

창 1안을 어느 스택으로 만들든 같이 따라오는 사실이다. 2026-09-26에 공식 문서와
소스를 열어 확인했다.

- 프로세스 간 `SetParent`는 허용된다. Raymond Chen의 답은 "Yes, it is technically
  legal."이고, 경고는 "implicitly attaches the input queues of the threads which those
  windows belong to, and this attachment is transitive."이다. QEMU 쪽이 멈추면 제 창의
  입력도 같이 멈출 수 있으므로 QEMU 감시는 별도 스레드에 둔다.
- `SetParent`는 `WS_CHILD`와 `WS_POPUP` 스타일을 바꾸지 않는다. 부모를 붙이기 전에
  `WS_POPUP`을 지우고 `WS_CHILD`를 켜야 하고, 떼어낼 때는 반대로 한다.
- DPI: 프로세스 간 부모-자식에서 DPI 인식 모드가 다르면 윈도우가 자식 프로세스의 DPI
  인식을 강제로 다시 설정한다. `SetThreadDpiHostingBehavior(DPI_HOSTING_BEHAVIOR_MIXED)`는
  서로 다른 인식의 자식을 담게 해 주지만 호스트 창을 만들기 전에 설정해야 한다. SDL2는
  DPI 인식을 기본으로 선언하지 않고(`SDL_HINT_WINDOWS_DPI_SCALING` 기본 꺼짐), QEMU
  11.1의 `ui/sdl2.c`도 DPI 힌트를 설정하지 않는다. QEMU 창의 실제 DPI 인식 상태는 M2
  첫 스파이크에서 재어야 한다.
- `WH_KEYBOARD_LL` 훅은 설치한 스레드의 메시지 루프에서 호출되고 `LowLevelHooksTimeout`
  (최대 1000 ms) 안에 돌아와야 하며, 넘기면 조용히 제거된다. 전용 스레드에 둔다.
- 비패키지 앱의 토스트: 고전 경로는 AppUserModelID와 시작 메뉴 바로가기가 필수다
  ("Without a valid shortcut ... you cannot raise a toast notification from a desktop
  app"). Windows App SDK의 `AppNotificationManager.Register`는 바로가기 없이 등록하지만
  관리자 권한으로 실행 중이면 지원되지 않는다.
- WebView2: "The Evergreen WebView2 Runtime will be included as part of the Windows 11
  operating system." 윈도우 10은 1803 이후 대부분 있지만 보장이 아니어서 설치기가
  부트스트래퍼를 가지고 있어야 한다.
- QEMU 11.1.0의 `-display sdl` 부속 옵션은 `gl`, `grab-mod`, `show-cursor`,
  `window-close` 네 개다. `window-close=off`로 사용자가 X를 눌러 QEMU를 끝내는 일을
  막을 수 있다. `ui/sdl2.c`는 `SDL_CreateWindow`만 쓰고 부모 HWND 옵션이 없으므로 창
  3안은 소스 패치가 필요하다. QMP는 윈도우에서도 AF_UNIX 소켓(`unix:`)을 소스 수준에서
  지원하지만(런타임 미확인) 현재 TCP 계약을 바꿀 이유는 없다.
- 선례: mpv의 `--wid`는 지정한 HWND 아래에 제 창을 만드는 협력형 방식("mpv will create
  its own window and set the wid window as parent")이라 별도 프로세스 mpv.exe로도 된다.
  Lively Wallpaper는 외부 프로그램 창을 `SetParent`로 바탕화면 WorkerW에 붙인다. Mobile
  Research v0.5.0은 Android Emulator(QEMU) 창을 찾아 `SetParent`로 패널에 넣었다고
  릴리스 노트에 적었다(자체 보고).

## 3. 후보 비교표

M2 이후를 맡길 수 있는 후보는 Qt Widgets, Tauri 2, Avalonia, WPF, Electron 다섯이고,
Wails v3는 정식 버전이 나오면 합류한다. 열은 1절의 일곱 기준이다. 근거는 각 후보 절에
있다.

| 후보 | 윈도우 통합 | 배포 크기 | 코드 서명 | Claude와 워커 | 라이선스 | 워커 분업 | 호스트 준비 |
|---|---|---|---|---|---|---|---|
| Qt 6 Widgets (C++) | 외부 창 컨테이너 공식 API, 스타일 전환과 SetParent를 Qt가 처리. 트레이 있음. 토스트는 밸룬 | 약 36 MB (실측) | signtool | C++은 Daybreak. 화면(Widgets/QML)은 Opus가 HTML만큼은 아니다. 컴파일 주기 김 | LGPL-3 → GPL-3 결합 | 한 언어여서 연결 단계가 얇다 | 6.10.1 MSVC 있음 |
| Tauri 2 (Rust) | 외부 창은 수동 SetParent + 좌표 환산. 트레이, 알림, 단일 인스턴스 플러그인. 업데이터 minisign(끌 수 없음) | 수 MB + WebView2 내장 | 설정(thumbprint 또는 signCommand) | Rust Daybreak, HTML Opus, IPC ASTRA. v2 권한 체계 예제 혼란 주의 | MIT/Apache-2 → GPL-3 | 세 갈래 분업에 정확히 맞음 | Rust MSVC, Build Tools, Node 있음 |
| Avalonia 12 (C#) | NativeControlHost 공식 API. TrayIcon 내장. 토스트는 툴킷 패키지 | 트리밍과 NativeAOT 가능, 수치 미확인 | signtool | C# Daybreak. .axaml은 Opus, WPF와 방언 다름 | MIT | 한 언어 | .NET 10 SDK 있음 |
| WPF (.NET 10, C#) | HwndHost 공식 API. 트레이, 토스트는 MIT 패키지 | 자체 포함은 크고 트리밍 불가, 아니면 런타임 60 MB 설치 | signtool | C# Daybreak, XAML Opus | MIT | 한 언어 | 있음 |
| Electron (TS) | 외부 창은 수동(koffi FFI). 트레이, 알림, 단일 인스턴스 내장. electron-updater가 Authenticode 검증 | 80~100 MB 압축 | electron-builder 설정 | 가장 능숙 | MIT | 세 갈래 분업에 맞음 | Node 있음 |
| Wails v3 (Go) | 외부 창은 수동. 트레이, 알림, 단일 인스턴스, 업데이터 내장(서명은 선택) | 약 15 MB (문서) | signtool 직접 | Go 무리 없음, v2/v3 혼용 위험 | MIT | 세 갈래 분업에 맞음 | Go 없음. v3는 베타 |
| WinUI 3 (C#) | 외부 창 API 없음(수동). 트레이 서드파티, 토스트 App SDK | App SDK 런타임 추가(수치 미확인) | 비패키지 signtool, MSIX는 서명 필수 | XAML 컴파일러 오류 이슈 이력 | MIT | 한 언어 | 있음 |

Flutter, Compose, Slint, egui/iced, Lazarus, wxWidgets, PowerShell + WPF는 3.6절에 제외
이유를 적었다. 서명 검증 업데이터를 내장한 것은 Tauri, Electron, Wails v3 세 개이고,
나머지는 Velopack(해시 대조만)을 붙이거나 직접 만든다.

### 3.1 C++ + Qt

Qt는 외부 창을 담는 공식 API(`QWindow::fromWinId` + `QWidget::createWindowContainer`)와
에뮬레이터 프런트엔드 선례가 가장 많고, 이 PC에 6.10.1 MSVC 빌드가 이미 있어 바로
빌드된다. 대가는 C++ 개발 속도와 Widgets의 외관, 그리고 토스트가 예전 밸룬 API라는
점이다.

| 항목 | 확인한 사실 |
|---|---|
| 버전 | 최신 6.11.2(2026-08-18). LTS는 6.8이지만 6.8.9(2026-09-16)부터 패치는 상용 라이선스 전용이므로 오픈소스는 최신 계열을 따라가야 한다. 이 PC의 6.10.1은 download.qt.io의 오픈소스 바이너리 |
| 라이선스 | Core, Gui, Widgets, Network, Svg는 LGPL-3.0(qtbase 헤더는 GPL-2.0-only와 GPL-3.0-only도 제시). GPL-3.0 전용 모듈(Quick 3D, Virtual Keyboard, Qml Compiler 등 14개)은 쓸 일이 없다. 동적 링크에도 LGPL 고지, 리링크 가능, Qt 소스 제공 의무는 남는다. GPL-2.0-or-later와는 GPL-3.0 아래에서 결합 |
| 구하는 길 | Qt 온라인 설치기는 Qt 계정 필수. 이 PC처럼 aqtinstall로 같은 바이너리를 계정 없이 받을 수 있고(파이썬 도구이지만 개발 환경 준비에만 쓴다), MSYS2 UCRT64에도 qt6-base 6.11.2-2(windeployqt 포함), qt6-declarative 6.11.2-1이 있다 |
| 배포 크기(실측) | EQ APO XT Editor의 windeployqt 결과: Qt6Core 9.6 + Gui 9.1 + Widgets 6.3 + Network 1.7 + Svg 0.6 = 27 MB, 플러그인 1.6 MB, 번역 2.7 MB, RHI용 d3dcompiler_47.dll 4.5 MB. Widgets 앱의 Qt 부담은 약 36 MB. MSVC 공식 바이너리는 ICU를 쓰지 않지만 MSYS2 빌드는 ICU에 의존한다(패키지 93 MB, 배포 크기는 확인 필요). Qt Quick은 Qml/Quick DLL이 더 붙고 수치는 확인 필요 |
| 외부 HWND 담기 | `QWindow::fromWinId`는 문서에 "a window created by another process"를 위한 API라고 적혀 있고, 재부모화 이외의 조작은 "untested"라고 못박는다. `createWindowContainer`는 "stack on top of the widget hierarchy as an opaque box" |
| 트레이, 토스트 | `QSystemTrayIcon`이 트레이. `showMessage`는 6.11.2 소스에서 `Shell_NotifyIcon(NIF_INFO)` 밸룬 API를 부르므로 윈도우 11에서 배너로는 보이지만 알림 센터에 남지 않는다. 필요하다면 WinRT 토스트를 C++로 직접 호출 |
| 그 밖의 M2 부품 | `QProcess`(stdout 시그널), `QTcpSocket`(QMP), `QNetworkReply` + `QCryptographicHash`(스트림 SHA-256), `QWizard`(마법사). 업데이터는 없고 EQ APO XT처럼 Velopack을 붙이거나 직접 만든다 |
| 서명 | signtool 그대로 |
| Claude와 워커 | C++은 Daybreak가 다루는 언어. 화면은 Widgets + QSS 또는 QML이고 Opus가 HTML/CSS만큼 자연스럽게 그리지는 않는다. 컴파일 주기가 길고 메모리 안전은 개발자 몫 |
| 선례 | Dolphin(GPL-2.0-or-later, Qt 6 Widgets)이 같은 라이선스 조합의 직접 선례. PCSX2(GPL-3), RPCS3(GPL-2.0-only, Qt 6.7+), melonDS(GPL-3+), 구글 Android Emulator의 Qt UI(Apache-2.0), Genymotion. DuckStation도 Qt이지만 현재 라이선스가 CC-BY-NC-ND라 선례로 셀 수 없다 |
| 툴체인 | MSVC 킷(Build Tools 2022/2026)으로 바로. Qt 공식 지원표는 MSVC 2022와 MinGW-w64 13.1을 적고 있고, MSYS2의 GCC 16 조합은 MSYS2가 패키징하는 동작 구성이지 Qt가 시험한 구성은 아니다 |

Qt 6.11.2의 윈도우 플랫폼 플러그인 소스(`qwindowswindow.cpp` 1462행,
`QWindowsForeignWindow::setParent`)를 2026-09-26에 직접 열어 보니, 외부 창을 자식으로 넣을
때 `WS_OVERLAPPEDWINDOW`와 `WS_POPUPWINDOW`를 지우고 `WS_CHILD`를 켠 다음 `SetParent`를
부르고, 떼어낼 때는 원래 스타일을 복원한다. 즉 M2 창 1안의 Win32 절차가 툴킷 안에 이미
구현되어 있고, 다른 후보는 이 코드를 직접 써야 한다. DPI와 입력 큐 결합 문제는 Qt도
해결해 주지 않는다.

사용자가 EQ APO XT에서 같은 Qt 6 Widgets + Velopack 조합으로 제품을 배포하고 있으므로,
빌드와 배포 절차를 이미 알고 있다는 점이 이 후보의 실질적인 장점이다.

### 3.2 Electron

Electron은 Claude와 Opus가 가장 확실하게 다루는 스택이고 트레이, 알림, 단일 인스턴스,
서명 검증 업데이터가 갖춰져 있지만, 공식 문서가 밝힌 크기가 압축 상태로 80~100 MB다.
QEMU 묶음 70 MB 위에 더 큰 껍데기를 얹는 셈으로, 사용자가 미리 지적한 대로 제품 취지와
맞지 않는다.

| 항목 | 확인한 사실 |
|---|---|
| 버전 | 44.4.5 (2026-09-23) |
| 런타임 | Chromium과 Node를 함께 심는다. WebView2 의존 없음. 사용자 PC에 설치할 것 없음 |
| 배포 크기 | 공식 문서: "Zipped Electron apps are usually around 80 to 100 Megabytes." 윈도우 x64 최소 앱의 현재 수치는 확인 필요. 2018년 비교에서는 압축 50 MB, 해제 118 MB |
| 외부 HWND 담기 | `BrowserWindow.getNativeWindowHandle()`로 제 HWND는 얻지만 외부 창을 담는 API는 없다. `SetParent`는 네이티브 애드온이나 FFI(koffi, 윈도우 x64 미리 빌드 제공)로 호출하고, 웹 레이아웃의 CSS 픽셀을 물리 픽셀로 환산해 위치를 맞춰야 한다 |
| 트레이, 토스트, 단일 인스턴스 | `Tray`, `Notification`, `app.requestSingleInstanceLock()` 내장. 토스트는 AppUserModelID와 시작 메뉴 바로가기 설정 필요 |
| 업데이터 | electron-builder의 electron-updater(NSIS)가 내려받은 설치기의 Authenticode 서명과 배포자 이름을 확인한다(`verifyUpdateCodeSignature` 기본 켜짐). `publisherName`이 비어 있으면 검증을 건너뛰므로 꼭 적어야 한다 |
| 코드 서명 | electron-builder v26의 `win.signtoolOptions`와 `win.azureSignOptions`(Azure Trusted Signing). v27 문서는 미공개 미리보기이므로 섞지 않는다 |
| 백엔드 | Node 본 프로세스의 `child_process.spawn`, `net`(QMP), `crypto`(SHA-256)만으로 M2 백엔드가 끝난다. 다른 언어의 사이드카를 쓸 이유는 없고, 쓰면 IPC만 늘어난다 |
| 라이선스 | MIT(Electron), BSD(Chromium), MIT(Node). GPL-2.0-or-later와 문제 없음 |
| Claude와 워커 | 화면은 HTML/CSS로 Opus의 제일 잘 그리는 매체. 백엔드 TypeScript는 Daybreak가 쓴다. 네이티브 애드온을 직접 빌드하려면 node-gyp가 파이썬과 VS C++ 도구를 요구하므로(둘 다 이 PC에 있음) 미리 빌드된 koffi로 피한다 |
| 툴체인 | Node 24 있음. 추가 설치 없음 |

### 3.3 Rust + Tauri 2

Tauri 2는 이 목록에서 가장 작고(실행 파일 수 MB, WebView2는 윈도우 11 내장), 서명 검증을
끌 수 없는 업데이터와 트레이, 알림, 단일 인스턴스 플러그인을 1차 플러그인으로 갖고, 백엔드
Rust와 화면 HTML/CSS가 Daybreak와 Opus의 분업에 그대로 들어맞는다. 약점은 외부 창을
담는 API가 없어 `SetParent`와 좌표 맞춤을 직접 쓰고, v1과 달라진 권한 체계 때문에 생성
코드가 예전 예제를 따르면 깨지는 것이다.

| 항목 | 확인한 사실 |
|---|---|
| 버전 | Rust 코어 tauri 2.11.5 (2026-07-01). CLI, JS API, 번들러, 플러그인은 각각 따로 버전이 매겨진다 |
| 런타임 | WebView2 Evergreen. 설치기 기본은 `downloadBootstrapper`(약 1.8 MB)로 없을 때만 받는다 |
| 배포 크기 | 홈페이지는 "as little as 600KB". 윈도우 x64 최소 앱의 설치기와 해제 크기는 확인 필요(문서에 없음). 오프라인 설치기를 심으면 약 127 MB가 더 붙는다 |
| 백엔드 | Rust: `std::process::Command`(QEMU, adb), tokio TCP(QMP), reqwest + sha2(ISO), `windows` 크레이트(Win32). 웹 쪽은 플러그인 `shell`로도 spawn과 stdout 수집이 가능 |
| 외부 HWND 담기 | `Window::hwnd()`로 제 HWND는 얻지만 외부 창을 담는 API는 없다. Rust에서 `windows::Win32::UI::WindowsAndMessaging::SetParent`를 직접 부르고, 웹 레이아웃이 자리표시 요소의 사각형을 IPC로 넘기면 Rust가 배율을 곱해 `MoveWindow`한다. 창 크기를 바꿀 때 QEMU 창이 한 박자 늦게 따라오는 것을 감수해야 한다 |
| 트레이, 토스트, 단일 인스턴스 | 코어 `tray-icon` 기능, 1차 플러그인 `notification`(설치된 앱에서 동작), `single-instance` |
| 업데이터 | 1차 플러그인 `updater`가 minisign 서명을 검증하고, 문서가 검증을 끌 수 없다고 적는다. NSIS/MSI 산출물에 `.sig`가 따라다닌다. M2 7번이 설정으로 끝난다 |
| 코드 서명 | `bundle.windows.certificateThumbprint`, `digestAlgorithm`, `timestampUrl`, 또는 `signCommand`(`%1`이 파일 경로). Azure Trusted Signing은 `signCommand`로 |
| 툴체인 | Rust MSVC 킷 + VS Build Tools + Windows SDK. 이 PC에 전부 있음. Node는 JS 프런트엔드 번들러를 쓸 때만. 문서는 "Tauri officially only supports the MSVC Windows target"이므로 GNU 킷으로 우회하지 않는다 |
| 라이선스 | MIT OR Apache-2.0. MIT를 골라도 되고 Apache-2.0 부분은 GPL-3.0 아래에서 결합. WebView2 SDK(로더)는 BSD-3-Clause, 런타임은 윈도우 구성 요소 |
| Claude와 워커 | 화면 HTML/CSS는 Opus, Rust 백엔드는 Daybreak, IPC 연결은 ASTRA. 이 세 갈래 분업이 설계대로 들어가는 유일한 후보. 위험은 v2의 capabilities/permissions 체계와 플러그인 분리를 모르는 예제(`@tauri-apps/api/tauri` 같은 v1 import)가 자주 섞여 들어오는 것. 버전을 고정하고 작업서에 v2 문서 URL을 박으면 막을 수 있다 |

같은 계열의 Dioxus desktop 0.7.10(MIT OR Apache-2.0)은 같은 wry/tao 위에 Rust로 UI도
쓰는 방식인데, 트레이는 있지만 토스트, 단일 인스턴스, 서명 검증 업데이터가 없고 번들러
기본이 WebView2 오프라인 설치기 포함이므로 Tauri에 밀린다. Neutralinojs 6.9.0(MIT)은
네이티브 핸들 API가 없고 업데이터가 리소스 파일만 갈아 서명 검증이 없어 제외한다.

### 3.4 C# 계열: WPF, WinUI 3, Avalonia

.NET 계열은 이 PC에서 새 설치 없이 빌드되고, 외부 창을 담는 컨테이너(WPF `HwndHost`,
Avalonia `NativeControlHost`)가 문서화된 API로 있다. 대신 배포 크기가 크고(WPF는
트리밍이 막혀 있음), 서명 검증 업데이터는 직접 만들어야 한다. 네 갈래 중 고를 만한
것은 WPF(.NET 10)와 Avalonia 둘이다.

| | WPF (.NET 10) | WPF (.NET Framework 4.8.1) | WinUI 3 (Windows App SDK 2.5.1) | Avalonia 12.1.3 |
|---|---|---|---|---|
| 런타임 | .NET 10 LTS(2025-11-11 출시, 지원 종료 2028-11-14). 윈도우에 내장되지 않아 설치기가 Desktop Runtime(x64 설치기 60 MB)을 같이 깔거나 자체 포함으로 배포 | 윈도우 11 22H2 이후 내장. 추가 런타임 없음 | .NET 10 + Windows App SDK 런타임. 비패키지(MSIX 아님) 자체 포함 배포 가능, 단일 exe는 불가 | .NET 10. 자체 포함, 트리밍, NativeAOT 모두 문서화 |
| 외부 HWND 담기 | `HwndHost` 공식 API. 그 위에 WPF 픽셀을 그리는 것은 문서가 금지(airspace) | 같음 | 동등한 API 없음. 제안 #10050이 2024-10부터 열려 있고, 창 HWND를 얻어 `SetParent`를 직접 호출 | `NativeControlHost` 공식 API. 네이티브 뷰가 항상 위에 그려지는 제약은 같음 |
| 트레이, 토스트 | H.NotifyIcon.Wpf 2.4.1(MIT), Microsoft.Toolkit.Uwp.Notifications 7.1.3(MIT, 7.0부터 시작 메뉴 바로가기 불필요) | 같음 | H.NotifyIcon.WinUI, `AppNotificationManager`(비패키지 등록 지원) | 내장 `TrayIcon`. 토스트는 WPF와 같은 패키지 |
| 배포 크기 | 프레임워크 종속이면 앱 수 MB + 런타임 설치기 60 MB. 자체 포함이면 런타임 전체가 들어가고 `PublishTrimmed`는 WPF에서 비활성(공식 문서) | exe 수 MB, 런타임 0 | 자체 포함 시 App SDK 추가분은 공식 수치 없음(확인 필요). NativeAOT는 1.6부터 | 트리밍과 NativeAOT 가능. 윈도우 크기 공식 수치 없음(확인 필요) |
| 코드 서명 | signtool 그대로 | 같음 | 비패키지면 signtool. MSIX는 서명 없이 설치 불가(테스트용 예외만) | signtool 그대로 |
| 언어 | C# 14 | C# 7.3까지만 공식 지원(레코드, switch 식, nullable 분석 없음) | C# 14 | C# 14 |
| Claude와 워커 | XAML과 P/Invoke를 잘 다룸. Opus의 화면은 XAML | 옛 문법 제약이 생성 코드와 자주 충돌 | XAML 컴파일러 오류가 불투명하다는 이슈 이력. VS 없이 `dotnet build`는 2026-09 문서로 가능 | WPF와 XAML 방언이 다르므로(.axaml, 스타일 선택자) WPF 지식이 그대로 옮겨지지 않음 |
| 선례 | Playnite(게임 런처, WPF, MIT) | 없음 | 없음 | Ryujinx(스위치 에뮬레이터) |
| 라이선스 | MIT | 런타임은 윈도우 구성 요소 | MIT | MIT |

공통 사항은 세 가지다.

- 업데이터는 Velopack 1.2.158(MIT, 2026-09-21)이 설치기와 업데이트를 붙여 주고 signtool
  연동도 있다. 다만 패키지 검증은 릴리스 피드의 SHA-256 대조이고 배포자 서명을 인증하지는
  않아, M2 7번의 서명 검증은 따로 구현해야 한다.
- Fluent 스타일은 WPF-UI 4.3.0(MIT, 2026-05-04)이 맡고, ModernWpf 0.9.x는 지원이 끝났다.
- NativeAOT는 VS의 C++ 데스크톱 워크로드가 필요하고 이 PC에는 있다. .NET 10의 지원
  매트릭스는 윈도우 11 23H2 이상을 적고 있어, 이 PC의 22H2(22621)는 개발은 되지만 M2의
  깨끗한 VM 시험은 지원 범위 안의 빌드에서 해야 한다.

### 3.5 Go 계열: Wails, Fyne

Go 계열은 툴체인이 이 PC에 없고, 쓸 만한 Wails v3가 아직 베타다. Wails v2는 안정하지만
트레이와 서명 검증 업데이터가 없고, Fyne은 자기 그림 위젯이라 외부 창을 담는 자리가 없다.
Tauri가 같은 구조(네이티브 백엔드 + WebView2)를 안정 버전으로 제공하므로 Go를 골라야 할
이유가 남지 않는다.

| | Wails v2 (2.16.0, 2026-09-14) | Wails v3 (3.0.0-beta.26, 2026-09-25) | Fyne 2.8.1 | Gio 0.10.2, walk |
|---|---|---|---|---|
| 상태 | 안정 | 베타(알파 아님, 최종 아님). 릴리스 노트는 API 안정을 주장 | 안정 | Gio 안정, walk는 2021-01 이후 커밋 없음 |
| 요구 사항 | Go 1.25, WebView2. CGO 불필요 | Go 1.25, WebView2. CGO 불필요 | CGO와 C 컴파일러(MSYS2 MinGW 권장) | Gio는 윈도우에서 추가 의존 없음 |
| 크기 | 확인 필요 | 문서 추정 약 15 MB(최적화 바이너리) | 작음(수치 확인 필요) | 작음 |
| 외부 HWND | 공개 HWND getter 없음. 창 찾기부터 직접 | `NativeWindow()`이 HWND를 줌. `SetParent`는 직접 | `RunNative`로 HWND는 얻지만 GL 캔버스 위젯과 겹쳐 쓰기 어렵다 | Gio는 `Win32ViewEvent.HWND`, walk는 위젯별 HWND. 모두 수동 |
| 트레이, 토스트 | 트레이 없음. 알림은 2.16.0부터 `SendNotification`(go-toast) | 트레이, 알림 서비스(윈도우 토스트), 단일 인스턴스 내장 | 트레이 내장(2.2부터) | Gio 알림 패키지는 윈도우 미지원 |
| 업데이터 | 없음 | `pkg/updater`: Ed25519/ECDSA 서명과 SHA-256. 다만 서명이 없어도 해시만 맞으면 통과하므로(선택사항) 강제하는 설정을 확인해야 한다 | 없음 | 없음 |
| 서명 | 빌드 뒤 signtool을 직접 호출(문서 예제) | 윈도우 패키징 가이드 있음, 자동 서명 설정은 확인 필요 | signtool 직접 | signtool 직접 |
| 라이선스 | MIT | MIT | BSD-3 | Unlicense OR MIT, BSD-3 |
| Claude와 워커 | Go는 무리 없지만 v2/v3 API가 다르고 혼용 예제가 많다 | 같음 | Fyne 위젯은 자기 그림이어서 네이티브 외관이 아니다 | 자료가 적다 |

### 3.6 그 밖의 후보

아래 일곱은 조사했지만 M2 이후를 맡기기에 모자란다. 공통된 이유는 외부 창을 담는 API가
없거나, 런타임이 무겁거나, Claude와 워커가 자주 다루는 언어가 아니라는 것이다. 버전은
2026-09-26에 공식 릴리스에서 확인했다.

| 후보 | 버전, 라이선스 | 외부 HWND | 트레이와 토스트 | 크기와 툴체인 | 판정 |
|---|---|---|---|---|---|
| Flutter | 3.47.5, BSD-3 | 윈도우 플랫폼 뷰 미구현(flutter/flutter#31713 열려 있음). 서드파티 flutter_native_view는 WIP | tray_manager 0.7.0, windows_notification 1.3.0 (MIT) | 릴리스 크기 공식 수치 없음. VS C++ 워크로드 필요(있음), Flutter SDK 설치 필요 | 외부 창을 담을 길이 없어 제외 |
| Compose Multiplatform | 1.12.1, Apache-2.0 | `SwingPanel` 안의 AWT `Canvas` HWND에 `SetParent`하는 것이 이론상 가능(미검증) | `Tray` 컴포저블 | jpackage/jlink로 JVM을 묶음. 크기 공식 수치 없고 공동체 보고는 수십~100 MB. JDK 17 있음 | JVM 묶음과 워커 분업 불일치로 제외 |
| Slint | 1.18.1, GPL-3.0 또는 로열티 프리 또는 상용 | 외부 창 컨테이너 없음. Slint를 밖의 HWND에 그리는 반대 방향만 지원 | `SystemTrayIcon` 내장(1.17부터). 토스트 없음 | Rust 또는 C++. 작음 | GPL-3 옵션은 D6와 맞지만 창 컨테이너가 없고 생태계가 어려 제외 |
| egui / iced | 0.36.2 (MIT/Apache-2.0), 0.14.0 (MIT) | raw-window-handle로 HWND는 얻지만 담는 위젯은 없음 | tray-icon, notify-rust 크레이트 | 작음. Rust 툴체인 있음 | Rust를 고를 거라면 Tauri가 같은 일을 더 적은 손질로 함 |
| Lazarus | 4.8, FPC 3.2.2. LCL은 링크 예외 붙은 수정 LGPL | Win32 네이티브 위젯, `TWinControl.Handle`로 HWND. 수동 `SetParent` | 직접 Win32 | exe 작음 | Claude와 워커 모두 Pascal은 다루는 언어가 아니어서 제외 |
| wxWidgets | 3.2.11 (2026-07-07), wxWindows Licence | `wxNativeWindow`는 HWND를 서브클래스하므로 다른 프로세스 창에는 부적합(MS 문서). 수동 `SetParent` | 직접 Win32 | 작음. MSYS2 또는 MSVC | Qt가 같은 자리를 더 많은 문서와 선례로 맡음 |
| PowerShell 5.1 + WPF | 윈도우 내장 | `HwndHost`는 상속이 필요해 스크립트에서 쓰기 어렵고, 메시지 루프와 훅 관리가 버겁다 | 가능 | 0 | D7이 PowerShell을 M0, M1 전용으로 정했고, 제품 감독자를 스크립트로 배포할 이유가 없어 제외 |

## 4. 개발 호스트 준비 상태

이 PC에는 Rust, .NET, Node, Qt 6.10.1(MSVC), Visual Studio Build Tools가 이미 있어서
Tauri, Electron, .NET 계열, Qt Widgets는 새 설치 없이 바로 빌드할 수 있다. Go와 Flutter만
툴체인을 새로 받아야 한다. 2026-09-26에 PATH와 레지스트리, vswhere, 그리고 EQ APO XT
프로젝트의 Qt 폴더에서 확인한 값이다.

| 도구 | 상태 | 쓰는 후보 |
|---|---|---|
| Rust 1.97.0 stable, `x86_64-pc-windows-msvc` 툴체인 (rustup) | 있음 | Tauri, Dioxus, egui/iced |
| Visual Studio Build Tools 2022 (17.14)와 2026 (18.6), VC 도구, Windows 11 SDK 26100 | 있음 | Rust MSVC 링커, Qt MSVC 빌드, Flutter, C++ 전반 |
| signtool 10.0.26100 | 있음 | 모든 후보의 코드 서명 |
| .NET SDK 10.0.300, WindowsDesktop 런타임 10.0.8 (8.0, 9.0도 있음) | 있음 | WPF, WinUI 3, Avalonia, WinForms |
| .NET Framework 4.8.09032 | 윈도우 내장 | WPF(.NET Framework) |
| Node.js 24.1.0 | 있음 | Electron, 웹 프런트엔드 번들러(Tauri, Wails) |
| WebView2 런타임 153.0.4234.48 (Evergreen) | 윈도우 내장 | Tauri, Wails, WebView2 기반 전부 |
| CMake 4.4.0, Ninja와 GCC 16.2 (MSYS2 UCRT64) | 있음 | Qt MinGW 빌드(예비 경로), wxWidgets. QEMU 빌드에 이미 쓰는 중 |
| Qt 6.10.1 `win64_msvc2022_64` (qtbase, qttools, qtsvg, qttranslations: Core, Gui, Widgets, Network, Svg, OpenGL, Sql 등, windeployqt 포함. qtdeclarative는 없어 Qt Quick은 불가) | 있음. `E:\EqualizerAPO-XT\Qt\6.10.1\msvc2022_64` (213 MB). EQ APO XT 개발용으로 aqtinstall 3.2.1이 2026-07-10에 download.qt.io에서 받은 오픈소스 바이너리. OME에서는 같은 방식으로 따로 받거나 `CMAKE_PREFIX_PATH`로 이 경로를 가리킨다. Qt Quick을 쓰려면 qtdeclarative 아카이브만 더 받으면 된다 | Qt (Widgets). MSVC 킷이므로 Build Tools와 함께 쓴다. EQ APO XT는 VS 2026 툴셋으로 이 Qt를 쓰는 중 |
| Go | 없음 | Wails, Fyne |
| Flutter SDK | 없음 | Flutter |
| JDK 17 | 있음 | Compose Desktop (Gradle은 wrapper로) |
| Python 3.13 | 있음(스택으로는 제외) | Electron 네이티브 애드온을 node-gyp로 빌드할 때만 관계 |

Rust 툴체인에는 안드로이드 타깃 네 개와 UEFI 타깃이 이미 추가되어 있다. 개발 도구를
새로 설치하는 일은 제품의 네트워크 목록(R10)이 아니라 개발 환경 준비다.

## 5. 라이선스 정리

어느 후보도 GPL-2.0-or-later 자체 코드와 결합하는 데 문제가 없다. Apache-2.0과 LGPL-3.0
부분은 GPL-3.0 아래에서 결합하며, D6가 or-later를 고른 이유가 바로 이것이다. 다만
링크되는 모든 구성 요소를 `THIRD_PARTY.md`에 기계 생성으로 적는 R4 절차가 프런트엔드
의존성도 덮도록 늘려야 한다.

| 구성 요소 | 라이선스 | GPL-2.0-or-later와 결합 |
|---|---|---|
| Qt 6 Core, Gui, Widgets, Network, Svg | LGPL-3.0 (qtbase 헤더는 GPL-2.0-only, GPL-3.0-only도 제시) | GPL-3.0 아래에서 결합. 동적 링크로 리링크 가능을 보장하고, LGPL 고지와 Qt 소스 입수 경로를 적는다 |
| Tauri, wry, tao, 1차 플러그인 | MIT OR Apache-2.0 | MIT를 골라도 되고 Apache-2.0 부분은 GPL-3.0 결합 |
| WebView2 SDK 로더(Microsoft.Web.WebView2 1.0.4191.47) | BSD-3-Clause (NuGet 라이선스 파일 원문 확인) | 문제 없음 |
| WebView2 런타임 | Microsoft 소프트웨어 라이선스 조건(윈도우 11 구성 요소) | 시스템 구성 요소로 쓴다. 재배포하지 않고 부트스트래퍼가 Microsoft 서버에서 받으므로 그 주소를 `docs/NETWORK.md`(R10)에 추가해야 한다 |
| .NET 런타임, WPF, WinForms, Windows App SDK, Avalonia | MIT | 문제 없음. 프레임워크 종속 배포면 런타임 설치기 주소도 R10 목록에 |
| Electron, Chromium, Node | MIT, BSD, MIT | 문제 없음 |
| Wails, Go 표준 라이브러리, Fyne | MIT, BSD-3, BSD-3 | 문제 없음 |
| Slint | GPL-3.0 또는 로열티 프리 또는 상용 | GPL-3.0 옵션으로 결합. 로열티 프리는 소스 공개 자유를 제한해 부적합 |
| Velopack, H.NotifyIcon, WPF-UI, Microsoft.Toolkit.Uwp.Notifications, koffi | MIT | 문제 없음 |
| wxWidgets | wxWindows Library Licence (LGPL + 링크 예외) | 문제 없음 |

기준은 FSF의 라이선스 호환표(https://www.gnu.org/licenses/license-list.html)와 Qt의
오픈소스 의무 안내(https://www.qt.io/development/open-source-lgpl-obligations)다.

## 6. 결정을 위해 사용자가 답할 것

1. Qt 6 Widgets(1순위)와 Tauri 2(2순위) 중 어느 쪽으로 M2를 시작하는가. 또는 스택 결정
   전에 PowerShell + P/Invoke로 창 1안 스파이크를 먼저 하는가.
2. Qt를 고르면 두 가지. Widgets로 갈지 Qt Quick(QML, qtdeclarative 추가 다운로드 필요)으로
   갈지, 그리고 Qt를 `E:\EqualizerAPO-XT\Qt`에서 그대로 참조할지 OME 아래에 따로 받을지.
3. 업데이트의 서명 검증(M2 7번)을 Velopack의 해시 대조로 갈음할지, Tauri처럼 배포자 키
   서명을 요구할지. Qt나 .NET을 고르면 후자는 직접 구현이다.
4. 설치기(M3)를 Inno Setup으로 할지 Velopack으로 할지. 지금 정하지 않아도 되지만 스택과
   같이 정하면 M2의 업데이트 항목 설계가 한 번에 끝난다.

## 7. 출처

모두 2026-09-26에 열어 확인했다. 각 절의 표 아래에 적지 않은 주요 출처만 모았다.

- Qt: https://www.qt.io/blog/qt-6.11.2-released , https://doc.qt.io/qt-6/licensing.html ,
  https://doc.qt.io/qt-6/qwindow.html#fromWinId , https://doc.qt.io/qt-6/qwidget.html#createWindowContainer ,
  https://raw.githubusercontent.com/qt/qtbase/v6.11.2/src/plugins/platforms/windows/qwindowswindow.cpp (1462행),
  https://raw.githubusercontent.com/qt/qtbase/v6.11.2/src/plugins/platforms/windows/qwindowssystemtrayicon.cpp ,
  https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-qt6-base , https://doc.qt.io/qt-6/windows.html ,
  https://github.com/dolphin-emu/dolphin , https://raw.githubusercontent.com/RPCS3/rpcs3/master/3rdparty/qt6.cmake
- Electron: https://www.electronjs.org/docs/latest/why-electron , https://www.electronjs.org/docs/latest/api/browser-window ,
  https://www.electron.build/v26/docs/api/electron-builder.interface.windowsconfiguration/ ,
  https://raw.githubusercontent.com/electron-userland/electron-builder/v26.0.12/packages/electron-updater/src/windowsExecutableCodeSignatureVerifier.ts ,
  https://koffi.dev/
- Tauri: https://v2.tauri.app/release/tauri/ , https://v2.tauri.app/distribute/windows-installer/ ,
  https://v2.tauri.app/reference/config/ , https://v2.tauri.app/plugin/updater/ ,
  https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/crates/tauri/src/window/mod.rs ,
  https://v2.tauri.app/security/capabilities/ , https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.4191.47/License
- .NET: https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core ,
  https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/hosting-win32-content-in-wpf ,
  https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/technology-regions-overview ,
  https://learn.microsoft.com/en-us/dotnet/core/deploying/trimming/incompatibilities ,
  https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/downloads ,
  https://github.com/microsoft/microsoft-ui-xaml/issues/10050 , https://docs.avaloniaui.net/docs/app-development/native-interop ,
  https://www.nuget.org/packages/Avalonia/12.1.3 , https://github.com/velopack/velopack , https://github.com/HavenDV/H.NotifyIcon ,
  https://github.com/lepoco/wpfui
- Go: https://github.com/wailsapp/wails/releases/tag/v2.16.0 , https://v3.wails.io/concepts/build-system/ ,
  https://raw.githubusercontent.com/wailsapp/wails/v3.0.0-beta.26/v3/pkg/updater/verify.go ,
  https://docs.fyne.io/started/quick/ , https://pkg.go.dev/fyne.io/fyne/v2/driver
- 그 밖: https://docs.flutter.dev/platform-integration/windows/setup , https://github.com/flutter/flutter/issues/31713 ,
  https://kotlinlang.org/docs/multiplatform/compose-native-distribution.html , https://github.com/slint-ui/slint/releases ,
  https://www.lazarus-ide.org/ , https://wxwidgets.org/downloads/
- 플랫폼: https://devblogs.microsoft.com/oldnewthing/20130412-00/?p=4683 ,
  https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setparent ,
  https://learn.microsoft.com/en-us/windows/win32/api/windef/ne-windef-dpi_hosting_behavior ,
  https://wiki.libsdl.org/SDL2/SDL_HINT_WINDOWS_DPI_SCALING , https://gitlab.com/qemu-project/qemu/-/raw/v11.1.0/ui/sdl2.c ,
  https://raw.githubusercontent.com/qemu/qemu/v11.1.0/qemu-options.hx ,
  https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc ,
  https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid ,
  https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution ,
  https://raw.githubusercontent.com/mpv-player/mpv/master/DOCS/man/options.rst
- 실측: 이 PC의 `%LOCALAPPDATA%\OpenMobileEmulator\qemu-build\out\bin`(QEMU 크기),
  `E:\EqualizerAPO-XT\build-Editor-x64\release`(Qt 배포 크기), PATH와 레지스트리, vswhere(호스트 준비 상태)

## 8. 결정 (2026-09-26)

사용자는 Tauri 2 + Rust를 골랐다. 이유는 세 가지다. 첫째, 다른 프로세스의 HWND를 받아
제 창에 표시하는 일은 사용자의 MacType Control Center(Tauri)에서 이미 해 본 일이라 결격
사유가 아니다. 남는 문제는 창 크기가 바뀔 때 QEMU 창이 한 박자 늦게 따라오는 지연인데,
이것은 코드로 풀 문제다. 둘째, 백엔드를 짤 때 C++가 Rust보다 낫다고 볼 근거가 없다.
셋째, 그렇다면 Rust 백엔드가 주는 이점만으로 Tauri가 앞선다.

6절의 질문은 이렇게 정리된다.

| 질문 | 결정 |
|---|---|
| 1. Qt와 Tauri 중 택일 | Tauri 2 + Rust. 스택 무관 스파이크는 하지 않고 M2 안에서 Rust로 바로 잰다 |
| 2. Widgets인지 Quick인지, Qt 재사용 여부 | 해당 없음 |
| 3. 업데이트 서명 검증 | Tauri updater 플러그인의 minisign 서명 검증을 쓴다. 이 검증은 끌 수 없다 |
| 4. 설치기 | Tauri 번들러의 NSIS(perMachine)로 간다. Inno Setup과 Velopack은 쓰지 않는다 |

같은 날 덧붙인 지침이 둘 있다. Tauri 껍데기와 웹뷰에는 지능을 주지 않고 Rust 크레이트가
지능을 갖는다. `unsafe`는 사용자의 두 앱(MacType Control Center, UAC 원격 승인기)과 같은
방식으로 윈도우 전용 크레이트의 ffi 모듈에만 허용하고 나머지는 전부 `forbid`한다. 두 앱에서
가져온 방법론의 목록과 웹뷰 스택, 아키텍처는 `docs/ARCHITECTURE.md`와 `docs/adr/`에 있다.
