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
│ guest-    │ super-   │ qmp       │ artifacts │ host-     │ adb           │
│ config    │ visor    │           │           │ check     │               │
├───────────┼──────────┼───────────┼───────────┼───────────┼───────────────┤
│ window-   │ keymap   │ wizard    │ diagnos-  │ (핵심 크레이트 전부       │
│ host      │          │           │ tics      │  #![forbid(unsafe_code)]) │
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
    ome-guest-config/     GuestConfig, QemuInvocation
    ome-qmp/              QmpChannel
    ome-supervisor/       Supervisor 상태 기계와 프로세스 어댑터
    ome-artifacts/        Manifest, AllowedHosts, ArtifactStore
    ome-host-check/       HostReadiness
    ome-adb/              AdbSession, AppPackage
    ome-window-host/      Stage 기하와 GuestWindowHost
    ome-keymap/           KeymapProfile, Mapper
    ome-wizard/           FirstRunWizard 상태 기계
    ome-diagnostics/      DiagnosticBundle
    ome-runtime/          AppRuntime, AppSnapshot, Command, AppIssue, 설정 저장
    ome-setup/            승격 도우미 실행 파일(고정 동사)
  app/                    Tauri 껍데기: Cargo.toml, tauri.conf.json, capabilities/, permissions/,
                          src/{main,lib,commands,admission,events,tray,window}.rs, icons/
  ui/                     Vite + React: index.html, src/{main.tsx, App.tsx, bridge.ts,
                          contracts.ts, screens/, components/, styles/}, public/fonts/
  presets/                키 매핑 프리셋 JSON (게임 이름은 파일 이름에만, R6)
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
- 이음새: `CommandRunner` 트레이트(실제 프로세스, 테스트 녹음).
- 테스트 표면: 출력 파싱과 패키지 열기는 픽스처로, 설치 순서는 녹음으로 테스트한다.

### 3.8 ome-window-host

- 인터페이스: `StageGeometry::physical(css_rect, scale) -> PhysicalRect`(순수),
  `GuestWindowHost::attach(parent_window, qemu_pid) -> Hosted`, `Hosted::place(rect)`,
  `Hosted::detach()`. 실패는 `HostingIssue`이고 호출자는 2안(별도 창)으로 간다.
- 뒤에 숨는 것: SDL 창 찾기(pid + 클래스), `WS_POPUP` 제거와 `WS_CHILD` 부여, `SetParent`,
  `SWP_FRAMECHANGED`, DPI 혼합 호스팅, 포커스 넘기기, QEMU 종료 시 분리.
- 테스트 표면: 기하 계산은 순수 테스트. 실제 담기는 M2 첫 스파이크에서 SDL 픽스처로 잰다.

### 3.9 ome-keymap

- 인터페이스: `KeymapProfile`(JSON, 버전 필드), `Mapper::translate(key_event, stage_size)
  -> Vec<InputEvent>`(순수), 프리셋 로더.
- 뒤에 숨는 것: 키 코드 표, 좌표 정규화(0~32767), 스와이프의 단계 분해, 눌림과 뗌.
- 테스트 표면: 프로필과 키 입력에 대한 이벤트 열을 표로 테스트한다.

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

### 3.14 host/ui (웹뷰)

- `contracts.ts`는 `AppSnapshot`, `Command`, `AppIssue`의 TypeScript 형이고, Rust 형과 같은
  내용이다. `bridge.ts`는 `invoke`와 `listen`을 감싼 유일한 IPC 표면이다. 화면은
  `(snapshot, viewState) => JSX`이며 훅 안에서도 판단을 만들지 않는다.
- 스타일은 `styles/tokens.css`(디자인 시스템과 같은 값)와 화면별 CSS다. 컴포넌트 라이브러리는
  쓰지 않는다. 폰트는 저장소에 담아 오프라인으로 싣는다(R10).
- QA 갤러리(`ui/qa.html`, 합성 스냅숏)는 제품 빌드에 절대 들어가지 않는다. Vite 플러그인이
  제품 번들에 QA 모듈이 섞이면 빌드를 실패시킨다.

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
