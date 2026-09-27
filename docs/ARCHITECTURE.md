# M2 아키텍처: Rust 코어, 얇은 Tauri 껍데기, 스냅숏을 그리는 웹뷰

작성 2026-09-26. 스택 결정은 `DECISION-frontend.md` 8절, 낱말은 `CONTEXT.md`, 개별 결정은
`adr/`에 있다. 이 문서는 모듈이 어디에 놓이고 무엇을 소유하는지, 이음새가 어디인지, 무엇을
테스트 표면으로 삼는지를 적는다. 사용자의 두 Tauri 제품(MacType Control Center, UAC 원격
승인기)에서 가져온 방법론은 마지막 절에 목록으로 있다.

## 1. 한 장 그림

```
┌──────────────────────────────────────────────────────────────────────────┐
│ host/ui  (React 19, TypeScript)                                          │
│   스냅숏(AppSnapshot) → 화면.  명령(Command) → bridge.ts → invoke        │
│   판단 없음. 파일, 네트워크, 프로세스 접근 없음. 플러그인 권한 없음.     │
└───────────────▲───────────────────────────────┬──────────────────────────┘
                │ 이벤트 snapshot, progress      │ 타입 명령 20여 개
┌───────────────┴───────────────────────────────▼──────────────────────────┐
│ host/app  (Tauri 2 껍데기, #![forbid(unsafe_code)])                      │
│   창 생성(create:false → setup에서), 내비게이션 허용 목록, 트레이,        │
│   단일 인스턴스, 입장(Admission) 검사, 명령 → 런타임 위임                 │
└───────────────────────────────┬──────────────────────────────────────────┘
                                │ 크레이트 하나만 부른다
┌───────────────────────────────▼──────────────────────────────────────────┐
│ ome-runtime  스냅숏 투영, 설정 저장(원자적 JSON), 명령 분배, 마법사 진행  │
├───────────┬──────────┬───────────┬───────────┬───────────┬───────────────┤
│ guest-    │ guest-   │ super-    │ qmp       │ artifacts │ host-check    │
│ image     │ config   │ visor     │           │           │               │
├───────────┼──────────┼───────────┼───────────┼───────────┼───────────────┤
│ adb       │ window-  │ input     │ overlay   │ wizard    │ diagnostics   │
│ (세대별)  │ host     │ (파이프라인)│ (두번째 창)│           │ (전부 forbid) │
└───────────┴──────────┴───────────┴───────────┴───────────┴───────────────┘
                                │ 소유된 타입만 오간다(원시 핸들 없음)
┌───────────────────────────────▼──────────────────────────────────────────┐
│ ome-platform-win  Win32 호출의 유일한 자리. unsafe는 src/ffi/* 안에만.    │
│   OwnedHandle, JobObject, ProcessLaunch, 창 찾기/재부모화/DPI, 키 훅,     │
│   승격 실행(runas), WHPX 가용성(WHvGetCapability)                         │
└───────────────────────────────┬──────────────────────────────────────────┘
                                │ 별도 프로세스 (링크 금지, D5·R4)
        ┌───────────────┬───────┴───────┬────────────────┐
        │ qemu-system-  │ adb.exe       │ ome-setup.exe  │
        │ x86_64.exe    │ (127.0.0.1:   │ (승격, 고정 동사│
        │ (QMP 4444)    │  5555)        │  enable-whpx)  │
        └───────────────┴───────────────┴────────────────┘
```

지능의 위치는 셋째 상자다. 웹뷰는 스냅숏을 받아 그리고 명령을 보내는 일만 하고, Tauri
껍데기는 창과 입장 검사만 한다. QEMU와 adb와 dism은 프로세스 밖에 있다.

## 2. 디렉터리

```
host/
  Cargo.toml              워크스페이스. lints: unsafe_code = "forbid", clippy all = warn,
                          dbg_macro/todo/unimplemented = deny
  rust-toolchain.toml     1.97.0, clippy, rustfmt, rust-analyzer
  deny.toml               cargo-deny 라이선스 허용 목록과 advisories
  crates/
    ome-platform-win/     Win32 (unsafe는 src/ffi/ 안에만, 크레이트 수준 deny)
    ome-guest-image/      GuestImageProfile, GuestFamily 어댑터, CapabilityProbe (3.15절)
    ome-guest-config/     GuestConfig, QemuInvocation
    ome-qmp/              QmpChannel
    ome-supervisor/       Supervisor 상태 기계와 프로세스 어댑터
    ome-artifacts/        Manifest, AllowedHosts, ArtifactStore
    ome-host-check/       HostReadiness
    ome-adb/              AdbSession, AppPackage
    ome-window-host/      Stage 기하와 GuestWindowHost
    ome-input/            입력 파이프라인: 포착, 문, 해석, 합성, 일정, 프로필 저장, 자동 적용 (3.16절)
                          (뼈대 첫 판의 ome-keymap을 이 이름으로 넓힌다)
    ome-overlay/          OverlayWindow 기하와 편집기 프로토콜 (3.17절)
    ome-wizard/           FirstRunWizard 상태 기계
    ome-diagnostics/      DiagnosticBundle
    ome-runtime/          AppRuntime, AppSnapshot, Command, AppIssue, 설정 저장
    ome-setup/            승격 도우미 실행 파일(고정 동사)
  app/                    Tauri 껍데기: Cargo.toml, tauri.conf.json, capabilities/, permissions/,
                          src/{main,lib,commands,admission,events,tray,window}.rs, icons/
  ui/                     Vite + React: index.html, src/{main.tsx, App.tsx, bridge.ts,
                          contracts.ts, screens/, components/, styles/}, public/fonts/
  presets/                입력 프로필 프리셋 JSON (게임 이름은 파일 이름에만, R6)
manifests/images/         게스트 이미지 프로필 JSON (3.15절). 산출물 URL과 SHA-256은 manifests/artifacts.json
  tools/                  node --test 게이트(권한 목록, 빌드 정책), quality.mjs
  package.json, package-lock.json, .npmrc(save-exact, engine-strict), .node-version
```

`ci/`에는 저장소 전체 게이트가 있다. M2에서 추가하는 것은 `ci/Check-UnsafeScope.ps1`
(unsafe 위치 검사, 3.2절)과 SPDX 검사 확장(.tsx, .mjs, .css, .toml)이다.

## 3. 모듈과 인터페이스

각 항목은 모듈 이름, 인터페이스(호출자가 알아야 할 전부), 이음새 뒤에 숨는 것, 테스트
표면 순서다. 인터페이스는 코드가 아니라 문장으로 못박고, 시그니처는 크레이트의 `lib.rs`
문서 주석이 정본이다.

### 3.1 ome-guest-config

- 인터페이스: 검증되지 않은 값(`RawGuestConfig`)을 `GuestConfig`로 검증한다. 실패는 필드
  이름과 사용자 문장을 가진 `ConfigIssue`다. `QemuInvocation::for_boot`와 `for_install`이
  `GuestConfig`, 게스트 경로, QEMU 설치 위치를 받아 실행 파일 경로와 인자 목록을 돌려준다.
- 뒤에 숨는 것: QEMU 플래그 전부. `-accel whpx,kernel-irqchip=off`, `-cpu Skylake-Client-v4`,
  `virtio-vga-gl,edid=off`, `-display sdl,show-cursor=on,gl=on`, `-action reboot=shutdown`,
  `-qmp tcp:127.0.0.1:<port>,server=on,wait=off`, `hostfwd=tcp:127.0.0.1:<adb>-:5555`,
  usb-tablet, usb-kbd, dsound + intel-hda. 각 값의 출처 주석은 런처에서 옮긴다.
- 테스트 표면: 런처 `Start-Guest.ps1 -DryRun`의 인자 출력을 픽스처로 저장해 두고 Rust 출력과
  인자 단위로 대조한다. 검증 실패 경로는 표로 테스트한다.
- 이미지 프로필과의 관계: `GuestConfig`는 이미지 프로필(3.15절)이 허용 목록 안에서 준 재정의
  (GPU 장치 문자열, 디스플레이 추가 옵션, 부팅 인자)를 받아 인자에 반영한다. 재정의는 열거형과
  검증된 값이며 자유 문자열 인자는 받지 않는다. 입력 파이프라인용 `-device virtio-multitouch-pci`
  는 `GuestConfig.multitouch` 플래그로 켠다. 기본은 켬이지만 런처 동등성 픽스처는 끔으로 잰다
  (런처에는 이 장치가 없다). 게스트가 이 장치를 인식하는지는 능력 조사가 확인한다.

### 3.2 ome-platform-win

- 인터페이스: 소유된 타입만 내보낸다. `OwnedHandle`(한 번만 닫힘), `JobObject`(kill-on-close),
  `ProcessLaunch`(표준 출력과 오류를 파일 핸들로 받아 자식 생성, 상속 핸들 목록 명시),
  `Child`(대기, 종료 코드, 종료 요청), `find_windows_of_process(pid)`, `WindowHandle`(클래스
  이름, 스타일 전환, 재부모화, 위치와 크기, DPI), `set_thread_dpi_hosting_mixed()`,
  `KeyboardHook`(전용 메시지 루프 스레드, 채널로 키 이벤트), `launch_elevated(exe, verb)`
  (ShellExecuteExW runas, 종료 코드 반환), `whpx_available()`(WHvGetCapability),
  `optional_feature_state(name)`.
- 뒤에 숨는 것: 모든 `unsafe`. 크레이트 루트는 `#![deny(unsafe_code)]`이고 `src/ffi/*.rs`
  모듈만 `#![allow(unsafe_code)]`를 단다. `#![deny(clippy::undocumented_unsafe_blocks)]`로
  모든 unsafe 블록에 `SAFETY:` 주석을 강제한다. 원시 HANDLE, HWND, 포인터, 버퍼 길이는 이
  크레이트 밖으로 나가지 않는다.
- 테스트 표면: 핸들 소유권과 잡 오브젝트는 실제 프로세스(`cmd /c exit 3`)로 테스트한다.
  창 담기는 SDL 창을 만드는 작은 픽스처 프로세스가 필요하므로 M2 첫 스파이크에서 만든다.
- 확인한 사실(2026-09-26, ASTRA 조사): tauri 2.11.6은 `windows` 0.61에 의존하고 이 크레이트는
  0.62.2를 쓰므로 `WebviewWindow::hwnd()`가 주는 HWND는 정수 값으로 받아 이 크레이트의
  `WindowHandle`로 다시 감싼다(두 버전의 뉴타입은 다른 타입이다). `WH_KEYBOARD_LL`은 전역
  훅이라 전용 스레드의 메시지 루프에서 돌리고, 콜백은 1000 ms 안에 끝내며(윈도우 10 1709+
  상한), 게스트 창이 전경일 때만 이벤트를 소비하고 어떤 키 입력도 기록하지 않는다.
  `AttachThreadInput`은 `Win32_System_Threading` 기능에 있고 포커스 문제의 손쉬운 해법이
  아니다. SDL2의 기본 창 클래스 이름은 `SDL_app`이지만 호출자가 바꿀 수 있으므로 게스트 창은
  QEMU 프로세스 ID로 찾고 클래스 이름은 보조 확인에만 쓴다. 프로세스 간 `SetParent`는 자식
  프로세스의 DPI 인식을 재설정할 수 있으므로 스파이크가 이를 측정한다.
- 게이트: `ci/Check-UnsafeScope.ps1`가 (1) `unsafe` 토큰이 주석 밖에 나타나는 파일이
  `host/crates/ome-platform-win/src/ffi/` 아래에만 있는지, (2) 다른 모든 크레이트의 `lib.rs`와
  `main.rs` 첫 줄 근처에 `#![forbid(unsafe_code)]`가 있는지, (3) 워크스페이스 lints가
  `unsafe_code = "forbid"`인지 확인한다. 어긋나면 커밋과 CI가 실패한다.

### 3.3 ome-qmp

- 인터페이스: `QmpChannel::connect(addr, timeout)`가 능력 협상까지 마친 채널을 준다.
  `execute(command, args) -> Value`와 타입 도우미(`query_status`, `system_powerdown`,
  `screendump`, `input_send_event`, `send_key`, `query_display_options`, `quit`). 이벤트는
  `poll_event()`로 읽는다. 동기 `std::net::TcpStream`이며 읽기 시간 제한을 갖는다.
- 뒤에 숨는 것: 인사 메시지 파싱, `qmp_capabilities`, 응답과 이벤트 분리, 부분 읽기 조립.
- 테스트 표면: 스레드 안의 가짜 QMP 서버(녹음된 응답)로 전부 테스트한다. 런처
  `Measure-OmeQmpLatency`의 지연 측정은 실제 QEMU가 있을 때만 도는 통합 테스트다.

### 3.4 ome-supervisor

- 인터페이스: `Supervisor::new(process_adapter, qmp_factory, log_dir, policy)`,
  `start(GuestConfig)`, `request_stop()`, `state()`, `subscribe()`. 상태는 `GuestState`.
- 뒤에 숨는 것: 시작 뒤 QMP 준비 대기(런처는 10초 기한, 250 ms 간격), 표준 출력과 오류
  파일 이름(`logs/qemu-<이름>-<시각>.stdout.log`), 정상 종료 순서(adb가 붙어 있으면
  `adb reboot -p`, 아니면 QMP `system_powerdown` → 30초 기한 → 강제 종료. 안드로이드는 ACPI
  전원 신호를 키 입력으로 다루므로 adb 경로가 먼저다), `-action reboot=shutdown` 뒤의 자동
  재기동(QMP `SHUTDOWN` 이벤트의 이유가 게스트 리셋일 때만. 전원 끄기와 비정상 종료는
  재기동하지 않는다. 이벤트 필드는 QEMU 11.1 소스로 확인한다), 비정상 종료 시 로그 보관과
  `Failed` 전이, 잡 오브젝트로 묶기. 지금의 런처에는 이 감독 루프가 없다. Start-Guest는 QMP
  응답을 한 번 기다리고 끝나며, 재부팅 뒤 재기동은 문서에만 있는 규칙이다. 감독자가 새로
  더하는 가장 큰 행동이 이것이다.
- QMP TCP chardev는 한 번에 한 클라이언트만 받으므로 세션의 주인은 감독자 하나다. 스크린샷,
  입력 전송 같은 다른 호출은 감독자를 거친다.
- 이음새: `ProcessAdapter` 트레이트에 어댑터가 둘이다. 실제 어댑터는 platform 크레이트를
  쓰고, 테스트 어댑터는 스크립트대로 종료 코드를 낸다. 어댑터가 둘이므로 이음새가 실재한다.
- 테스트 표면: 상태 기계 `Lifecycle::on(event) -> (state, actions)`는 순수 함수라 전이표를
  전부 테스트한다. 재부팅 재기동과 비정상 종료의 구분, 두 번 시작 거절, 종료 중 시작 거절이
  핵심 사례다.

### 3.5 ome-artifacts

- 인터페이스: `Manifest::load(path)`, `AllowedHosts::permits(url)`(접미사 일치),
  `ArtifactStore::fetch(name, progress_sink) -> VerifiedFile`, `verify(path)`. 진행 상황은
  바이트 수와 속도로 보고한다.
- 뒤에 숨는 것: 리다이렉트 추적(SourceForge 미러), `Range`로 이어받기, 스트리밍 SHA-256,
  임시 파일 뒤 원자적 이름 바꾸기, 크기 검사, 매니페스트에 없는 URL 거절(R2, R10).
- 이음새: `HttpFetch` 트레이트. 실제 어댑터는 `ureq`(rustls), 테스트 어댑터는 메모리 서버.
- 테스트 표면: 이어받기, 해시 불일치, 허용되지 않은 호스트, 중단 뒤 재시작.

### 3.6 ome-host-check

- 인터페이스: `HostReadiness::inspect(probe) -> Report`. 항목은 CPU 가상화, 하이퍼바이저
  실행 중, `HypervisorPlatform` 상태, 재부팅 대기, WHPX 가용성, QEMU 존재와 버전, 펌웨어,
  adb, 디스크 여유. 각 항목은 `Ready`, `Attention`, `Blocked`와 문장이다.
- 이음새: `HostProbe` 트레이트(실제: platform 크레이트와 파일 시스템, 테스트: 표 값).
- 테스트 표면: 항목 조합에 따른 전체 판정(하나라도 `Blocked`면 마법사가 멈춘다).

### 3.7 ome-adb

- 인터페이스: `AdbSession::new(adb_exe, addr)`, `wait_for_boot(timeout)`, `getprop(name)`,
  `packages()`, `install(AppPackage)`, `uninstall`, `launch`, `screencap() -> PngBytes`,
  `logcat_tail(lines)`, `apply_display(DisplayPreset)`, `set_media_volume(index)`.
  `AppPackage::open(path)`는 APK, XAPK, APKS를 열어 base와 split 목록을 만든다.
- 뒤에 숨는 것: adb 명령줄 전부(`adb -s 127.0.0.1:5555 ...`), 인용 규칙, 출력 파싱, XAPK zip
  풀기, `install-multiple` 순서, 첫 부팅 뒤 미디어 볼륨 15 설정(KNOWN_LIMITATIONS).
- 세대 의존성: 안드로이드 세대에 따라 달라지는 명령(미디어 볼륨, 기기 ID, 전경 앱, 앱 목록
  출력 형식)은 이 크레이트가 직접 알지 않고 `ome-guest-image`의 `FamilyAdapter`에서 받는다.
  `AdbSession`은 세션(어디에 붙는가)이고 어댑터는 방언(무슨 말을 하는가)이다.
- 이음새: `CommandRunner` 트레이트(실제 프로세스, 테스트 녹음).
- 테스트 표면: 출력 파싱과 패키지 열기는 픽스처로, 설치 순서는 녹음으로 테스트한다.

### 3.8 ome-window-host

- 인터페이스: `StageGeometry::physical(css_rect, scale) -> PhysicalRect`(순수),
  `GuestWindowHost::attach(parent_window, qemu_pid) -> Hosted`, `Hosted::place(rect)`,
  `Hosted::detach()`. 실패는 `HostingIssue`이고 호출자는 2안(별도 창)으로 간다.
- 뒤에 숨는 것: SDL 창 찾기(pid + 클래스), `WS_POPUP` 제거와 `WS_CHILD` 부여, `SetParent`,
  `SWP_FRAMECHANGED`, DPI 혼합 호스팅, 포커스 넘기기, QEMU 종료 시 분리.
- 테스트 표면: 기하 계산은 순수 테스트. 실제 담기는 M2 첫 스파이크에서 SDL 픽스처로 잰다.

### 3.9 ome-input

- 뼈대 첫 판에는 키 → 탭/스와이프의 순수 매퍼와 프리셋 로더만 있다. 사용자가 2026-09-26 밤에
  요구한 입력 기능 전체(홀드, 가상 조이스틱, 마우스 버튼, 게임별 자동 적용, 논리 좌표,
  오버레이 편집기, 일시 중지 단축키, 매크로 자리)는 3.16절의 `ome-input`이 소유하고, 이
  크레이트는 그 이름으로 바뀐다.

### 3.10 ome-wizard

- 인터페이스: `WizardState`, `Step`, `advance(state, outcome) -> WizardState`(순수), 저장과
  복원.
- 뒤에 숨는 것: 여덟 단계의 순서와 건너뛰기 규칙(R8 등록은 건너뛸 수 있고, R9 동의는
  건너뛸 수 없으나 `tcg`로는 진행할 수 없다), 재부팅 뒤 이어 가기.
- 테스트 표면: 전이표.

### 3.11 ome-diagnostics

- 인터페이스: `DiagnosticBundle::collect(sources) -> PathBuf`.
- 뒤에 숨는 것: zip 구성, logcat 2000줄, 환경 표, 개인 정보가 실릴 수 있는 값(계정, 시리얼)
  제외 규칙.

### 3.12 ome-runtime

- 인터페이스: `AppRuntime::open(home, adapters)`, `snapshot()`, `apply(Command) ->
  Result<AppSnapshot, AppIssue>`, `events()`. 설정은 버전 있는 JSON을 스테이징 파일 뒤 이름
  바꾸기로 저장하고 크기 상한을 둔다.
- 뒤에 숨는 것: 명령마다 어느 모듈을 어떤 순서로 부르는지, 스냅숏 조립, 마법사 상태 저장,
  실패의 사용자 문장 번역.
- 테스트 표면: 모든 하위 모듈의 테스트 어댑터를 꽂아 명령 → 스냅숏을 통합 테스트한다.
  이것이 제품 논리의 주된 테스트다.

### 3.13 host/app (Tauri 껍데기)

- `tauri`는 `default-features = false`에 `wry`, `compression`, `common-controls-v6`,
  `tray-icon`만 켠다. 웹뷰가 부를 수 있는 것은 이 앱의 타입 명령과 `core:event`의 listen,
  unlisten뿐이다(`adr/0003`). 창은 `create: false`로 두고 `setup`에서
  `WebviewWindowBuilder::from_config`로 만들며 `on_navigation`은 앱 자신의 출처만 허용한다.
- 명령은 `admission`의 입장권을 얻은 뒤 `spawn_blocking`에서 런타임을 부른다. 입장권이 없으면
  `busy`로 즉시 거절하고 대기열을 만들지 않는다.
- 파일 선택 대화상자는 Rust가 `rfd`로 열고 검증된 경로만 런타임에 넘긴다. 웹뷰에는 dialog,
  fs, shell, http 플러그인 권한이 없다.
- 트레이 아이콘과 알림은 껍데기가 소유한다. 알림은 `tauri-plugin-notification`을 Rust 쪽에서만
  부른다.
- 창은 둘이다. `main`과, 무대 위에 얹는 투명한 `overlay`(3.17절). 오버레이 창은 `main`이 소유하고
  장식이 없으며 Rust가 무대 사각형에 맞춰 놓는다. 편집 모드가 아니면
  `set_ignore_cursor_events(true)`로 클릭이 통과한다. 두 창은 같은 권한 목록을 쓰고, 오버레이
  웹뷰가 부를 수 있는 명령도 앱의 타입 명령뿐이다.

### 3.14 host/ui (웹뷰)

- `contracts.ts`는 `AppSnapshot`, `Command`, `AppIssue`의 TypeScript 형이고, Rust 형과 같은
  내용이다. `bridge.ts`는 `invoke`와 `listen`을 감싼 유일한 IPC 표면이다. 화면은
  `(snapshot, viewState) => JSX`이며 훅 안에서도 판단을 만들지 않는다.
- 스타일은 `styles/tokens.css`(디자인 시스템과 같은 값)와 화면별 CSS다. 컴포넌트 라이브러리는
  쓰지 않는다. 폰트는 저장소에 담아 오프라인으로 싣는다(R10).
- QA 갤러리(`ui/qa.html`, 합성 스냅숏)는 제품 빌드에 절대 들어가지 않는다. Vite 플러그인이
  제품 번들에 QA 모듈이 섞이면 빌드를 실패시킨다.

### 3.15 ome-guest-image

사용자 요구(2026-09-26 밤): 1차 목표는 13, 15, 16, 17을 지원하는 것이고(처음 목표에 있던
Pie(API 28)는 ADR-0008로 2026-09-28에 뺐다), 그 뒤로 새 안드로이드가 나오는 대로 따라간다. 그러므로 안드로이드 게스트는 처음부터 교체 가능한 부품이다
(ADR-0004). D2가 정한 Bliss 16.9.x(안드로이드 13)는 첫 프로필이고, 다른 세대는 프로필을 더해
지원한다.

- 인터페이스(2026-09-27 저녁 확정, `src/family.rs`): `GuestImageProfile::load_all(dir)`
  (`manifests/images/*.json`), `GuestFamily::for_api_level(api)`, `adapter_for(api) -> Box<dyn
  FamilyAdapter>`. 어댑터는 명령과 파서만 갖는다(`*_command() -> Option<ShellCommand>`, `None`이면 그
  세대에 방법이 없다는 뜻이고, `parse_*`는 출력에서 값을 꺼낸다). 명령을 실제로 돌리는 것은
  `ShellRunner`(`shell`, `root`, `push`)이며 런타임이 adb 세션으로 구현하고 테스트는 녹음 출력으로
  구현한다. `CapabilityProbe::run(&runner, &adapter) -> ProbeOutcome`은 첫 부팅 뒤 한 번 돌아
  항목별 `Available | Unavailable | Unknown`과 읽은 값(기기 ID, 미디어 볼륨, Google 계정 수, 전경 앱,
  루트 상태, 화면, 앱 목록)을 돌려주고, 런타임이 `<home>/vm/<id>/guest.json`에 저장한다.
  `CompatEntry::load(package)`.
- 이미지 프로필의 필드: `id`, `display_name`(사용자 말로, 예: "안드로이드 13"), `android_version`,
  `api_level`, `distribution`(`bliss`, `android_x86`, `self_built`), `artifact`(`manifests/artifacts.json`의
  산출물 이름), `translator`(`houdini`, `ndk_translation`, `digitalis`, `none`; `translator/`
  계약의 `android_api`와 대조), `boot_args`, `grub_entry_hint`, `install_guide`(설치기 안내 문장
  목록), `qemu_overrides`(허용 목록 안의 열거형 값만: GPU 장치, 디스플레이 옵션, EDID), `status`
  (`verified`, `candidate`, `deprecated`)와 검증 기록.
- 세대 어댑터가 소유하는 방언: 앱 목록(`pm list packages -3 --show-versioncode`의 유무),
  앱 라벨 읽기, 미디어 볼륨(`cmd media_session volume`, 없으면 `service call audio` 대체),
  기기 ID(GSF `content query`, 권한이 막히면 안내 문장으로 대체), 전경 앱(`dumpsys activity
  activities`의 `topResumedActivity` 또는 `mResumedActivity`), 부팅 완료 표지, 네이티브 브리지
  속성(`ro.dalvik.vm.native.bridge`의 값), 화면 크기와 밀도 명령. 세대는 `Legacy`(28~29),
  `Modern`(30~34), `Current`(35~)로 시작하고 조사 결과에 따라 나눈다. 모르는 API 레벨은
  `Current`로 시작한다.
- 뒤에 숨는 것: 세대별 명령 문자열과 출력 파서 전부. 호출자(런타임, 마법사, 입력 파이프라인)는
  `adapter.foreground_package()`처럼 뜻으로만 부른다.
- 테스트 표면: 각 세대 어댑터의 명령 문자열 표와 출력 파서를 녹음 픽스처로 테스트한다. 능력
  조사는 가짜 adb로 항목별 성공과 실패를 테스트한다. 실제 이미지에서의 조사 결과는
  `docs/evidence/M2/images/<id>.md`에 남긴다.
- 후보 이미지(2026-09-26 기준, 버전 대응은 확인 필요): 안드로이드 13은 Bliss 16.9.7(검증됨),
  안드로이드 15와 16은 Bliss 18 또는 arcadia-x86 트리의 자체 빌드(R3), 안드로이드 17은 상류가
  나오는 대로. `Legacy` 세대(API 29 이하)의 어댑터는 사용자가 가져온 옛 이미지와 능력 조사의
  대체 경로를 위해 남기지만, 이 세대의 이미지 프로필은 만들지 않는다(ADR-0008). 32비트 ARM 앱은
  안드로이드 13 이상의 프로필이 비공개 번역기로 받는다.

### 3.16 ome-input

사용자 요구(2026-09-26 밤): 좌표 탭, 홀드, 방향키와 WASD의 가상 조이스틱, 드래그와 스와이프,
마우스 버튼, 게임별 프로필 저장과 자동 적용, 해상도와 창 크기에 흔들리지 않는 논리 좌표,
화면 위에서 바로 고치는 편집기, 전체 매핑을 잠시 끄는 단축키. 그리고 나중에 키보드 매크로 같은
편의 기능이 더해질 수 있다(ADR-0005).

파이프라인은 다섯 단계이고 단계마다 모듈이다. 현재 포착, 문, 키 합성은 구현되어 있다.
`KeyboardHook`이 읽은 스캔 코드와 확장 플래그는 keycodemapdb에서 생성한 표로 브라우저 코드와
QEMU qcode가 된다. 런타임은 전경 창, 무대 표시, 부팅 완료, 편집, 일시 중지, 선택 프로필을 문에
넘기며, 통과 입력은 감독자 채널을 거쳐 QMP `input-send-event`로 간다. 감독자 스레드만 QMP를
소유하므로 입력 호출은 런타임을 막지 않는다. 실행 중이 아닐 때 온 입력은 버리고 개수를 로그에
남긴다.

| 단계 | 모듈 | 인터페이스 | 뒤에 숨는 것 |
|---|---|---|---|
| 포착 | `KeyboardHook`(구현), `MouseSource`(예정) | `KeyboardHook::install(Sender<KeyEvent>)` | 플랫폼 크레이트의 `WH_KEYBOARD_LL` 훅과 전용 스레드가 키 스캔 코드, 확장 여부, 눌림·뗌을 보낸다. 콜백은 입력을 막거나 기록하지 않는다. 마우스 훅은 아직 없다 |
| 문 | `Gate`(키 구현) | `admit(&HostKey, &GateFacts) -> Decision` | 게스트 실행, 무대 표시, 앱 전경 여부를 먼저 검사한다. F12 등 일시 중지 단축키의 누름만 상태를 바꾸고 게스트에는 보내지 않는다. 부팅 뒤 선택 프로필에 바인딩된 키만 해석하며, 그 밖의 키는 그대로 통과한다 |
| 해석 | `Interpreter` | `on(HostInput, now) -> Vec<TouchOp>`(순수. 눌린 키 집합, 슬롯 배정, 조이스틱 상태를 내부에 갖는다) | 바인딩 종류(탭과 홀드, 스와이프, 가상 조이스틱, 마우스 버튼, 휠, 통과), 논리 좌표 → 게스트 픽셀 → QMP 축(0~32767) 변환, 기준점 정책으로 화면 비율 차이 흡수, 슬롯 최대 10개 |
| 합성 | `TouchSynth`(예정), `KeySynth`(구현) | `KeySynth::apply(HostKey) -> Option<Value>` | `KeySynth`는 QMP `input-send-event` 키 JSON을 만들고 자동 반복 누름을 없앤다. 알 수 없는 스캔 코드는 세어 버린다. `TouchSynth`의 멀티터치 이벤트와 장치가 없을 때의 단일 포인터 폴백은 아직 없다 |
| 일정 | `Scheduler` | `schedule(at, TouchOp)`, `tick(now)` | 스와이프의 보간 단계, 조이스틱 60 Hz 갱신, 나중의 매크로 시퀀스. 단조 시계 주입으로 결정적 테스트 |

- 프로필: `InputProfile`(JSON, `version`, `id`, `name`, `package: Option`, `reference_aspect`,
  `anchor: Center | Edges`, `bindings: Vec<Binding>`). `ProfileStore`는 동봉 프리셋(`host/presets/`)과
  사용자 프로필(`<home>/profiles/`)을 합쳐 읽고 원자적으로 저장한다. `AutoApply`는
  `ForegroundWatcher`(세대 어댑터의 전경 앱 명령을 1초마다)의 패키지로 프로필을 고른다.
  프로필이 없으면 매핑 없음이며, 마지막으로 수동 선택한 프로필은 그 게스트에 기억된다.
- 논리 좌표: 저장은 0.0~1.0. 프로필의 `reference_aspect`와 실제 게스트 비율이 다르면 `anchor`
  정책으로 옮긴다(가운데 기준은 짧은 축을 맞추고 긴 축은 가운데 정렬, 가장자리 기준은 각 점이
  가까운 가장자리에서의 거리 비율을 지킨다). 무대 크기와 배율은 좌표에 영향이 없다. 좌표는
  언제나 게스트 화면 기준이다.
- 테스트 표면: 문은 모든 판정 규칙을 표로 검사하고, 키 합성은 정확한 QMP JSON과 반복 제거를
  검사한다. 감독자는 실행 전 입력을 버리는지와 한 입력 묶음이 QMP 호출 하나가 되는지를 가짜
  QMP로 검사한다. 해석기는 입력 열 → 터치 동작 열의 표로 전부 테스트한다. 조이스틱은 키 조합
  여덟 방향과 뗌 순서, 홀드는 누름과 뗌의 짝, 슬롯은 동시 터치 열 개, 기준점 정책은 16:9 →
  16:10과 세로 전환. 일정은 가짜 시계로. 합성은 가짜 QMP로. 포착은 플랫폼 스파이크에서만.
- 매크로 자리: `Binding.action`은 `#[non_exhaustive]` 열거형이고 `Sequence { steps, repeat }`
  변형과 일정 단계의 시퀀스 실행이 들어갈 자리를 문서 주석으로 표시한다. v1에는 넣지 않는다(D9).
  들어갈 때는 게임 약관 검토가 먼저다(P5).

- 남은 일: `TouchSynth`와 `Scheduler`를 구현하고 현재 `Mapper`가 만든 터치 동작을 멀티터치
  QMP 이벤트로 잇는다. 마우스 포착, 조이스틱 상태, 스와이프 일정도 이때 연결한다. 지금은
  바인딩된 키가 만든 터치 동작을 로그에 남기고 버린다.

### 3.17 ome-overlay

- 순수 인터페이스는 `overlay_mode(&AppSnapshot) -> OverlayMode`, `overlay_rect(PhysicalRect) ->
  PhysicalRect`, `placement(mode, Option<PhysicalRect>) -> OverlayPlacement`이다. 게스트가 실행 중이고
  선택한 프로필이 실제 목록에 있으며 표지를 켰거나 편집 중일 때만 창을 보인다. 편집 프로토콜은
  그대로다. 오버레이 웹뷰는 편집 중인 바인딩을 초안으로 들고 있다가 `저장`에서
  `input_profile_save { profile }` 하나로 보낸다. Rust가 프로필 전체를 한 번에 검증하므로 두 표지의
  키를 맞바꾸는 편집도 중간 상태 없이 들어간다. 그 밖에 보내는 명령은 `input_editor_toggle`뿐이고
  좌표는 논리 좌표로 온다. `input_binding_upsert`와 `input_binding_remove`는 레일의 입력 화면이 쓴다.
- `host/app/src/overlay.rs`가 `overlay.html`을 여는 두 번째 Tauri 창을 처음 필요할 때 만든다. 이 창은
  투명하고 장식과 그림자와 작업 표시줄 항목이 없으며 `main`이 소유한다. 붙인 SDL 자식의 클라이언트
  사각형을 화면 물리 픽셀로 읽어 같은 자리에 놓고, 읽지 못하면 `main`의 클라이언트 원점과 마지막
  `StageRect`를 사용한다. 편집 밖에서는 클릭을 통과시키고, 편집에 들어갈 때 오버레이로 포커스를
  옮기며 나올 때 `main`으로 돌린다. 같은 배치를 되풀이하지 않는다. `main`이 최소화되거나 숨으면
  소유 창도 숨고, 창 이동·크기·배율·포커스 이벤트와 모든 새 스냅숏에서 배치를 다시 계산한다.
- 표지의 그림은 오버레이 웹뷰(HTML/CSS)가 그린다. 글자는 어떤 게임 화면 위에서도 읽히도록
  어두운 배경 칩 위에 놓는다(DESIGN.md 9절).
- 테스트 표면: 모드 표와 사각형·배치 규칙은 순수 단위 테스트로 검사한다. 실제 창의 좌표, 보임,
  클릭 통과, 중지 때 숨김은 무시된 실제 게스트 테스트에서 잰다.

## 4. 데이터 흐름 두 가지

**게스트 시작.** 웹뷰 `guest_start` → 껍데기 입장 검사 → `AppRuntime::apply(GuestStart)` →
`GuestConfig` 검증 → `QemuInvocation::for_boot` → `Supervisor::start`(platform `ProcessLaunch` +
`JobObject`) → QMP 준비 대기 → `GuestState::Running` → 스냅숏 이벤트 → 웹뷰가 무대 사각형을
`stage_rect_changed`로 보냄 → `GuestWindowHost::attach`와 `place` → adb `wait_for_boot` →
스냅숏의 `bootCompleted = true`.

**게스트 재부팅.** 안드로이드가 재부팅 → QEMU가 `-action reboot=shutdown`으로 종료(코드 0,
QMP `SHUTDOWN` 이벤트에 `reason: guest-reset`) → `Lifecycle`이 `Restarting`으로 전이하고
같은 `QemuInvocation`으로 다시 시작 → 창 담기 다시 수행. 비정상 종료(코드 ≠ 0 또는 재부팅
이벤트 없음)는 `Failed`로 가고 로그 경로를 스냅숏에 싣는다.

## 5. 두 앱에서 가져온 방법론

| 항목 | 출처 | OME 적용 |
|---|---|---|
| 워크스페이스 `unsafe_code = "forbid"`, 윈도우 크레이트만 `deny` + ffi 모듈 `allow` | UAC `Cargo.toml`, `windows-observer` | 3.2절 그대로. 추가로 `Check-UnsafeScope.ps1` 게이트 |
| `#![deny(clippy::undocumented_unsafe_blocks)]`와 `SAFETY:` 주석, 소유 핸들 타입, 경계 검사된 읽기 | MacType `service-runtime/platform` | platform 크레이트의 규칙 |
| 얇은 Tauri 껍데기: 런타임 크레이트 하나만 부름, 입장권(`CommandAdmission`) | UAC `src-tauri/src/{lib,commands,admission}.rs` | 3.12, 3.13절 |
| `create: false` 창 + `on_navigation` 허용 목록, `withGlobalTauri: false` | UAC `lib.rs`, `tauri.conf.json` | 3.13절 |
| 엄격한 CSP, 권한 목록에 앱 명령만(`core:default` 없음), 권한 목록과 핸들러의 동기화 테스트 | UAC `tauri.conf.json`, `tools/tauri-capability.test.mjs` | `adr/0003`, `host/tools/tauri-capability.test.mjs` |
| 고정 동사 승격 대리, 임의 명령줄 없음, 결과는 프로세스 종료 코드 | MacType `open_service/broker`, `fixed helper` | `ome-setup` |
| 잡 오브젝트로 자식 수명 묶기, 상속 핸들 목록 명시 | MacType `platform/{job,launch}.rs` | 감독자의 QEMU 실행 |
| React 19 + 엄격 TS + Vite + 손으로 쓴 토큰 CSS, 컴포넌트 라이브러리 없음, 폰트 번들 | UAC `ui/`, `DESIGN.md` | 3.14절, `docs/DESIGN.md` |
| QA 갤러리와 제품 빌드 분리를 빌드 시점에 강제 | UAC `vite.config.ts`, `ui-build-policy.test.mjs` | 3.14절 |
| `contracts.ts` + `bridge.ts`가 유일한 IPC 표면, 판단은 Rust | UAC `ui/src/{contracts,bridge}.ts` | 3.14절 |
| cargo-deny 라이선스 허용 목록과 advisories, 고정 툴체인, `.npmrc save-exact` | UAC `deny.toml`, `rust-toolchain.toml`, `.npmrc` | `host/` 루트 |
| `quality.mjs` 한 명령으로 fmt, clippy `-D warnings`, 테스트, UI 검사 | UAC `tools/quality.mjs` | `host/tools/quality.mjs`, `ci/Invoke-AllChecks.ps1`에서 호출 |
| 도메인 용어 고정(`CONTEXT.md`), 디자인 계약(`DESIGN.md`), ADR | 두 앱 모두 | 저장소 루트와 `docs/` |
| 한국어 문구는 결과와 다음 행동, 내부 이름 금지, 존댓말 | UAC `UX_CONTRACT.md` | `AppIssue`와 화면 문구 규칙 |

가져오지 않은 것: UAC의 Android와 uniffi 계층, 벤더링한 tauri 패치(안드로이드 전용), Tamarin
증명(암호 프로토콜이 없다), MacType의 커스텀 캡션(OME는 윈도우 기본 캡션을 쓴다).

## 6. 첫 스파이크

M2 구현의 첫 작업은 창 담기다. `ome-platform-win`의 창 함수와 `ome-window-host`를 먼저 채우고,
지금 돌고 있는 QEMU와 같은 인자로 띄운 QEMU 창을 임시 Tauri 창의 무대에 붙여 DPI, 포커스,
크기 변경, 종료 처리를 잰다. 결과는 `docs/evidence/M2/window-hosting.md`에 남긴다. 이
스파이크가 실패하면 2안(별도 창 + 오버레이)으로 가고, 그 결정은 ADR로 남긴다.
결과(2026-09-27 밤, 제품 런타임으로 실제 게스트를 두 번 띄움): 1안이 통과했다. `running` 뒤
122 ms에 붙었고, 게스트 창 DPI는 붙이기 전후 96으로 같았으며, 부모를 960×600으로 바꾸자 게스트
클라이언트 영역도 960×600이 됐고, `GuestWindowToFront` 뒤 게스트 HWND가 키보드 포커스를 가졌다.
정지는 adb 전원 끄기(`reboot -p`)를 거쳐 2.1~2.3 s 만에 `stopped`와 `UserStop`으로 끝났다. 부팅은
33~37 s, 능력 조사는 열 항목 중 일곱이 `available`이었다(이 게스트에는 멀티터치 장치와 앱 루트가
없고, 전경 앱은 아직 읽지 않는다). 측정 중 결함 둘을 고쳤다. 호스트 조사가 돌려준 `\\?\` 경로에
QEMU가 설정 파일 경로를 못 만들어 시작 직후 죽던 것과, 껍데기가 감독자의 전원 끄기 이음새를
비워 두어 ACPI `system_powerdown`만 보내고(Android는 이를 키 입력으로 취급한다) 30 s 뒤 강제
종료하던 것이다.
오버레이 배치(ADR-0005 결과 항목)도 같은 게스트로 쟀다(`docs/evidence/M2/overlay-window.md`):
`overlay` 창은 게스트 자식 창과 같은 화면 사각형(2560×1600)에 놓였고, 편집 모드 진입에서 보이기까지
625 ms(첫 생성 포함), 표시 모드 재적용 44 ms, 표지 끄기 1 ms였으며 게스트가 끝나면 숨었다. 그
측정 중 스냅숏마다 ISO를 다시 해시하던 결함을 찾아 검증 표지로 고쳤다.

두 번째 스파이크는 멀티터치다. `-device virtio-multitouch-pci`를 더한 게스트에서 QMP
멀티터치 이벤트가 안드로이드 터치로 닿는지, 슬롯 두 개(조이스틱 + 탭)가 동시에 동작하는지,
`usb-tablet`의 마우스와 함께 써도 어긋나지 않는지를 잰다. 결과는
`docs/evidence/M2/multitouch.md`에 남기고, 실패하면 단일 포인터 폴백만으로 v1을 낸다.
결과(2026-09-27 저녁): 앞의 둘은 통과했다. 세 번째는 실패했다. `input-send-event`의 `btn`
이벤트가 usb-tablet이 아니라 멀티터치 장치에 BTN_MOUSE로 들어가, 잡고 있던 터치가 풀린다. 그래서
합성 단계는 마우스 버튼도 터치 접촉으로 합성하고(usb-tablet은 절대 좌표 이동에만 쓴다), 터치
접촉을 잡고 있는 동안 `btn` 이벤트를 보내지 않는다. `mtt`의 begin/update는 슬롯과 tracking-id만
정하고 좌표는 별도 `data` 이벤트로 보내며, end/cancel은 tracking-id -1이다. QMP 왕복 중앙값은
2.2 ms였다.

## 7. 확장 지점

- **새 안드로이드 버전**: `manifests/images/<id>.json`을 더하고(산출물은 `artifacts.json`에),
  세대 어댑터가 모르는 방언이 있으면 어댑터를 하나 더한 뒤, 게스트를 만들어 능력 조사를
  돌리고 결과를 `docs/evidence/M2/images/<id>.md`에 남긴다. 호환성 항목에 그 이미지의 검증
  기록을 적으면 끝이다. 코드 변경은 어댑터에 국한된다.
- **새 바인딩 종류(매크로 포함)**: `Binding.action`에 변형을 더하고 해석기에 그 변형의 터치
  동작 열을, 필요하면 일정 단계에 시퀀스를 더한다. 오버레이 편집기에 표지 종류 하나가 늘고
  프로필 JSON `version`이 오른다. 새 이음새는 없다.
- **새 입력 장치**(게임패드): 포착 단계에 `GamepadSource`를 더하고 `HostInput` 열거형에 변형을
  더한다. 해석기의 바인딩은 입력 종류에 무관하다.
- **다른 변환기**(Digitalis): 이미지 프로필의 `translator` 값 하나와 `translator/` 계약이다.

## 8. 계약 두 번째 판 (2026-09-27 확정)

뼈대 첫 판의 `contract.rs`는 `KeymapView`와 `keymap_*` 명령, 여덟 단계 마법사, 고정 범위의 설정을
갖는다. 두 번째 판은 3.15~3.17절과 사용자 결정(2026-09-27, `M2-SCREENS.md` 10절)을 반영한다.
웹뷰(`contracts.ts`), Rust(`contract.rs`), 픽스처(`tests/fixtures/contract/`), 권한 목록
(`host/app/capabilities/main.json`), 명령 목록(`TAURI_COMMANDS`, `build.rs`, `generate_handler!`)을
한 커밋에서 같이 고친다. `CONTRACT_VERSION`은 2가 된다. 이 절이 두 번째 판의 정본이고 필드 이름은
그대로 옮긴다(와이어는 camelCase).

### 8.1 마법사

- `WizardStep`은 일곱 단계다. `hostCheck`, `whpxConsent`, `rebootPending`, `artifactDownload`,
  `guestInstall`, `firstBoot`, `appInstall`, `done`. `googleRegistration`은 없다(사용자 결정
  2026-09-27: 구글 등록은 첫 실행에서 요구하지 않고 설정의 운영체제 절에서 필요할 때만 보인다).
- `WizardView`에서 `gsf_id`가 빠지고 `image_id: Option<String>`(S1.4에서 고른 이미지)과
  `install_guide: Vec<String>`(고른 이미지 프로필의 설치 안내 문장, S1.5)이 들어온다.
- 명령 `wizard_restart`는 없어진다(설정의 `처음부터 다시 설정` 삭제). `guest_disk_create`는
  `guest_create { image_id, size_gib }`로 바뀐다.

### 8.2 게스트 이미지와 게스트

```
ImagesView { profiles: Vec<GuestImageSummary>, guests: Vec<GuestSummary>, active_guest: Option<String> }
GuestImageSummary {
  id, display_name, android_version: String, api_level: u32,
  distribution: Bliss | AndroidX86 | SelfBuilt,
  translator: Houdini | NdkTranslation | Digitalis | None,
  size_bytes: Option<u64>, status: Verified | Candidate | Deprecated,
  released_at: Option<String>, verified_games: u32,
  recommended: bool
}
GuestSummary { name, image_id, android_version, disk_size_gib: u32, last_started_at: Option<String>,
               capabilities: CapabilityReport }
CapabilityReport { probed_at: Option<String>, items: Vec<CapabilityItem> }
CapabilityItem { id: CapabilityId, state: Available | Unavailable | Unknown }
CapabilityId = bootMarker | appList | displaySize | mediaVolume | deviceId | screenshot |
               foregroundApp | multitouch | nativeBridge | root
```

- 프로필은 `manifests/images/<id>.json`에서 새 크레이트 `ome-guest-image`가 읽는다(3.15절의 필드.
  첫 파일은 Bliss 16.9.7, 안드로이드 13, `released_at` 2024-10-11, `status: verified`). 세대 어댑터와
  능력 조사의 구현은 뒤 단계이고 두 번째 판에는 타입과 `recommended` 규칙, 로더만 있다.
- `profiles`는 Rust가 `android_version` 내림차순으로 정렬해 준다. `recommended`는 Rust가 계산한다.
  규칙(사용자 결정 2026-09-27): `status == Verified`이고 `released_at`이 28일보다 오래된 프로필 중
  가장 새 것 하나만 `true`. 하나도 없으면 `Verified` 중 가장 새 것, 그것도 없으면 전부 `false`.
  웹뷰는 이 값을 계산하지 않는다.
- 명령: `guest_image_select { id }`, `guest_create { image_id, size_gib }`, `guest_select { name }`,
  `guest_delete { name }`(디스크까지), `guest_reinstall { name }`(디스크를 지우고 같은 이미지로
  설치 절차를 다시 시작. 설정의 `다시 설치`).
- `GuestView`에 더해지는 필드: `image_id: Option<String>`, `android_version: Option<String>`,
  `api_level: Option<u32>`, `capabilities: CapabilityReport`, `device_id: Option<String>`(GSF
  안드로이드 ID, 부팅 뒤 읽었을 때), `adb_address: Option<String>`(예 `127.0.0.1:5555`),
  `root_enabled: Option<bool>`(조사 전이면 None).
- 명령 `guest_root_set { enabled }`: 세대 어댑터가 정한 방법으로 앱 루트 권한을 켜고 끈다.
  `open_registration_page`는 남고, 실행 시 `device_id`를 클립보드에 복사한 뒤 기본 브라우저를 연다.

### 8.3 입력

`ome-keymap`은 `ome-input`으로 이름이 바뀌고 프로필 형식은 `version: 2`가 된다.

```
InputView { profiles: Vec<InputProfile>, active_id: Option<String>, suspended: bool, editing: bool,
            auto_apply: bool, foreground_package: Option<String>, multitouch: Capability,
            suspend_hotkey: String }
Capability = Available | Unavailable | Unknown
InputProfile { id, name, bundled: bool, target_package: Option<String>,
               reference_aspect: Size, anchor: Center | Edges, bindings: Vec<Binding> }
Binding { id: String, trigger: Trigger, action: BindingAction }
Trigger = { kind: key, code }                        // W3C KeyboardEvent.code
        | { kind: mouseButton, button: left|right|middle }
        | { kind: wheel, direction: up|down }
        | { kind: keySet, up, down, left, right }     // 가상 조이스틱의 네 키
BindingAction = { kind: tap, at: LogicalPoint, hold: bool }
              | { kind: swipe, from: LogicalPoint, to: LogicalPoint, duration_ms: u32 }
              | { kind: joystick, center: LogicalPoint, radius: f64 }   // 반지름은 짧은 축 기준 0..1
              | { kind: mouseTap, at: Option<LogicalPoint> }            // None = 누른 자리
              | { kind: wheelSwipe, at: LogicalPoint, distance: f64 }
              | { kind: passThrough }
LogicalPoint { x: f64, y: f64 }   // 0.0..=1.0
```

- 명령: `input_profile_select { id: Option<String> }`, `input_suspend_toggle`,
  `input_profile_delete { id }`, `input_profile_save { profile: InputProfile }`(새 프로필과 이름
  바꾸기, 대상 앱 지정, 복제가 모두 이 하나로 간다. 동봉 프리셋은 저장을 거부한다),
  `input_binding_upsert { profile_id, binding }`, `input_binding_remove { profile_id, id }`,
  `input_editor_toggle`, `input_auto_apply_set { enabled }`, `input_suspend_hotkey_set { code }`.
- 검증은 Rust가 한다. 좌표 범위, 트리거 중복(같은 프로필 안에서 같은 키 두 번), 조이스틱의 네
  키가 서로 다름, `duration_ms > 0`, `radius`와 `distance`가 0 초과. 실패는 `AppIssue`다.
- `Binding.action`은 `#[non_exhaustive]`이고 시퀀스(매크로)의 자리는 문서 주석으로만 표시한다(D9).

### 8.4 표시

```
DisplayView { presets: Vec<DisplayPreset>, active_id: Option<String>, custom: Option<CustomDisplay>,
              fit: StageFit, refresh_rate_hz: Option<u32>, refresh_rates: Vec<u32>, refresh_supported: bool,
              vsync: VsyncMode, vsync_supported: bool }
CustomDisplay { size: Size, density_dpi: u32 }
VsyncMode = Off | On | Adaptive
```

- 명령: `display_preset_apply { id }`, `display_custom_apply { size, density_dpi }`(폭과 높이는
  640..=7680, 8의 배수, DPI 120..=640), `display_refresh_set { hz: Option<u32> }`(None은 운영체제 기본인 60 Hz, Some은 30..=240,
  목록은 60, 75, 90, 120, 144에 사용자 지정 하나), `display_vsync_set { mode }`, `stage_fit_set { fit }`.
- 주사율은 QEMU 11.1의 virtio-gpu에서 장치 속성이 아니다. `refresh_rate`는 `DEFINE_EDID_PROPERTIES`를
  쓰는 장치(`VGA`, `bochs-display`)에만 있고, virtio-gpu는 디스플레이 백엔드가 보내는 `QemuUIInfo.refresh_rate`
  (`hw/display/virtio-gpu-base.c`)를 EDID에 넣는다. GTK 백엔드는 호스트 모니터의 주사율을 보내고 SDL
  백엔드는 아무것도 보내지 않아 생성기 기본값 75 Hz가 된다(그래서 `edid=off`였다). 첫 시도(SDL 옵션으로
  UI 정보를 보내는 패치)는 SDL 창의 시작 크기 640x480이 장치의 `xres`, `yres`를 덮어써 폐기했다
  (`docs/evidence/M2/sizing-20260927/bootC-guest.txt`). 그래서 자체 패치 0002는 virtio-gpu에 장치 속성
  `refresh_rate`(mHz, EDID 도우미와 같은 이름과 단위)를 더하고, UI 정보가 주사율 없이 오면 그 값을
  유지한다. `ome-guest-config`는 주사율이 None이면 지금처럼 `virtio-vga-gl,edid=off`를 내고(런처 동등성
  픽스처 그대로), Some(hz)면 `virtio-vga-gl,edid=on[,xres=<w>,yres=<h>],refresh_rate=<hz*1000>`을 낸다.
  다시 시작해야 적용된다. `DisplayView.refresh_supported: bool`은 QEMU 번들의 `ome-patches.txt`에 0002가
  있을 때만 참이고 그때만 주사율 행이 화면에 있다. EDID의 선호 모드가 주사율을 실으므로 안드로이드는
  설정 변경 없이 그 모드로 돈다(부팅 D: 1280x800 @ 119.997 Hz, VSYNC 8.33 ms). 세대 어댑터의
  `peak_refresh_rate` 설정은 필요할 때만 쓰는 예비 수단이다. 수직 동기화도 같다. QEMU SDL 디스플레이의
  `SDL_GL_SetSwapInterval(0)` 고정값(`ui/sdl2.c`)을 `-display sdl,swap-interval=<-1|0|1>` 옵션으로 여는
  자체 패치 0003이 있어야 하고, `vsync_supported = true`일 때만 화면에 행이 있다.

### 8.5 설정

```
SettingsView { memory_mib, memory_mib_min: u32, memory_mib_max: u32, vcpus, vcpus_max: u32,
               gpu_mode, close_action, show_fps, auto_update_check, home_dir, disk_usage_bytes,
               adb_access: AdbAccess, binding_overlay_default: bool }
SettingsInput { memory_mib, vcpus, gpu_mode, close_action, show_fps, auto_update_check,
                adb_access, binding_overlay_default }
AdbAccess = Localhost | Network
CloseAction = StopGuest | MinimizeToTray      // 기본값 StopGuest (사용자 결정 2026-09-27)
```

- 한도는 호스트에서 온다. `HostProbe`에 `total_memory_bytes()`와 `logical_processors()`가 더해진다.
  `memory_mib_max` = 호스트 물리 메모리 − 4096 MiB를 1024 단위로 내림(최소 4096),
  `memory_mib_min` = 4096, `vcpus_max` = 호스트 논리 프로세서 수(최소 2). 기본값은 8192와 4 그대로.
  `Settings::validate`와 `GuestConfig::validate`는 고정 상한(16384, 8) 대신 이 한도를 받는다.
  QEMU와 WHPX에는 자체 상한이 없다(`target/i386/whpx/whpx-all.c`는 `smp.cpus`를 파티션 프로세서 수로
  그대로 넘기고 q35의 `max_cpus`는 4096). 근거는 `docs/evidence/M2/sizing-20260927/`.
- `adb_access`: `Localhost`면 지금처럼 `hostfwd=tcp:127.0.0.1:<port>-:5555`, `Network`면
  `hostfwd=tcp:0.0.0.0:<port>-:5555`. 기본값 `Localhost`. 화면은 어느 쪽이든 경고를 보인다.
- 트레이는 그대로 있다. 창을 닫을 때 게스트를 끄는 것이 기본이고 트레이로 내리는 것은 설정이다.

### 8.7 화면 A가 드러낸 빈자리 (2026-09-27, 세 번째 판에서 구현함)

화면 워커가 마법사와 막힘 화면을 그리며 계약에 없는 동작을 셋 찾았다. 없는 동작은 없는 버튼으로
두었고, 다음 명령이 들어오면 화면이 버튼을 되살린다.

- `wizard_defer`: 마법사를 지금 단계에 저장한 채 닫고 무대로 간다(`나중에 하기`, S1.2의 `지금은
  건너뛰기`와 Esc). `phase`는 `main`이 되고 마법사 상태는 그대로 남아 다음 실행에서 이어진다.
  하이퍼바이저가 꺼진 채 미루면 무대는 `blocker: hypervisorPlatformOff`를 받는다. `wizard_skip`은
  지금처럼 건너뛸 수 있는 단계(앱 설치)에서만 다음 단계로 간다.
- `open_help { topic }`: `topic`은 열거형(`virtualizationBios`, `hypervisorPlatform`, `googleAccount`,
  `adbSecurity`)이고 Rust가 `docs/` 안의 정해진 문서 또는 GitHub의 정해진 페이지(R10 목록)를 기본
  브라우저로 연다. 웹뷰는 URL을 모른다. S8의 `켜는 방법`, 설정 고급 절의 도움말이 쓴다.
- `app_quit`: 창을 닫고 앱을 끝낸다(S1.3의 `닫기`). 창 닫기 정책(운영체제 끄기)과 같은 경로를 탄다.
- `guest_volume_set { index }`(0~15): 무대 도구 막대의 `볼륨`. 능력 조사 `mediaVolume`이 있을 때만 버튼이 있다.
  `GuestView.media_volume: Option<u32>`가 현재 값이다.
- `input_overlay_toggle`과 `InputView.overlay_visible: bool`: 무대 도구 막대의 `매핑 표시 켬/끔`. 지금은
  `OverlayState.mode`가 Rust 안에만 있어 화면이 표지가 보이는지 알 수 없다.
- `app_install_cancel`: 앱 설치 진행 줄의 `취소`.
- `open_help`의 `topic`에 `qemuSource`(R4 소스 제공 링크), `thirdPartyNotices`, `releaseNotes`(업데이트의
  릴리스 노트, Rust가 `UpdateState::Available.notes_url`을 연다)를 더한다. 정보 절의 `소스 코드 받기`,
  `제3자 고지`, 업데이트의 릴리스 노트 링크가 쓴다.
- `open_home_folder`: 저장 위치의 `폴더 열기`(`open_logs_folder`, `open_screenshots_folder`와 같은 꼴).
- `copy_to_clipboard { item }`: `item`은 열거형 `deviceId | adbAddress`. 웹뷰는 자유 문자열을 보내지
  않고 Rust가 스냅숏의 값을 복사한다. 설정 고급 절과 Google 계정 절의 `복사`.
- `display_presets()`의 `needs_reboot`는 현재 방향과 카드의 방향이 다를 때만 참이다(지금은 셋 다 참).
- `input_profile_save`로 새 프로필(없던 id)을 저장하면 그 프로필이 `active_id`가 된다. 이름 바꾸기나
  대상 앱 지정처럼 있는 id를 저장하면 선택은 그대로다.
- `blocker`는 마법사 밖(`phase == main`)에서 `HypervisorPlatform`이 꺼져 있으면 `hypervisorPlatformOff`다.
  호스트 점검 행 자체는 마법사가 켤 수 있으므로 `Attention`으로 남는다.
- 런타임의 고정값(`wizard_view`의 `download`, `disk_free_bytes`, `inspected_at`, `wizard_facts`의
  `guest_installed`, `guest_booted`)은 산출물 저장소와 감독자, adb 어댑터가 붙을 때 실제 값이 된다.
  파일 놓기(S1.7, 앱 화면)는 Tauri 껍데기의 `DragDrop` 이벤트가 검증된 경로를 런타임에 넘기는 일이고
  웹뷰는 놓기 영역만 그린다.

### 8.6 막힘 화면과 문구

- `AppSnapshot.blocker: Option<Blocker>`, `Blocker { kind: VirtualizationOff | QemuMissing |
  HypervisorPlatformOff }`. `Some`이면 웹뷰는 S8을 그린다. 마법사와 무관하게 호스트 점검 결과에서
  Rust가 정한다.
- `GuestView.last_exit.kind`는 `userStop | guestReset | bootTimeout | crash | startFailed`가 되어
  S2의 실패 문장을 원인별로 나눈다(`unexpected`는 `crash`로 바뀐다).
- `AppIssue`의 문장은 `DESIGN.md` 9절의 어휘를 따른다. 화면에 `게스트`라는 말은 없고 `운영체제`와
  `가상 머신`만 있다.
