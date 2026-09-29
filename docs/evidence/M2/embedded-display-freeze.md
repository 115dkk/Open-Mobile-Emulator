# 담긴 게스트 창의 표시 정지 (2026-09-29)

## 배경

키 캡처를 웹뷰로 옮긴 뒤(`keyboard-capture.md`) 완료 실행을 다시 돌렸더니 키는 QMP까지 갔는데 설치기 GRUB 메뉴가
60초 동안 검은 화면으로 찍혔다(`dod-evidence-x1`). 키를 전혀 보내지 않아도 같았다. 담긴 창은 펌웨어 단계와 GRUB 단계
(VGA 호환 모드의 2D 표면)를 검은색이나 펌웨어 로고 한 장으로 멈춘 채 보이다가, 커널의 virtio-gpu 스캔아웃이 시작되면
(부팅 로고부터) 다시 살아났다. 3라운드(2026-09-28)에서는 같은 자리에서 GRUB이 찍혔었다.

## 환경

- 호스트: 개발 PC(CLAUDE.md 9절). 모니터 한 대, 배율 200 %(`GetDpiForWindow` = 192).
- 제품: `host/target/debug/ome.exe`(커밋 4d8a269), 그리고 비교용으로 3라운드 커밋 1a86b28을 따로 빌드한 것.
- QEMU: `host/target/debug/qemu`(sha256 82f25980…, 3라운드와 동일). 인자도 경로만 빼면 동일.
- 캡처: `ci/dod/native.ps1 -Shot`(DWM 창 경계 또는 `-QemuPid`로 SDL 창 사각형). 두 방식이 같은 순간 같은 결과를 냈다.
- 스크립트는 `docs/evidence/M2/embedded-display-freeze/`에 있다.

## 실험과 결과

### 제품에 담긴 창

| 실행 | 조건 | 결과 |
|---|---|---|
| x1 완료 실행 | 드라이버(창이 뜨자마자 전경 전환과 HOME을 100 ms마다) | 60초 내내 검은색 |
| 키 없음 1, 2 | 마법사만 걸어감, 캡처만 | 검은색 또는 펌웨어 로고로 멈춤, 부팅 로고(virtio-gpu)부터 정상 |
| GRUB 키 | 9초에 첫 캡처, 그때 HOME·DOWN×4·ENTER | 9초에 GRUB, 설치기 커널 로그와 파티션 대화상자까지 정상 |
| 촘촘한 시간표 | 1.5초 간격 메인 창 캡처 | 0~29초 GRUB, 31초에 부팅(시간 초과 30초) |
| 두 캡처 비교 | 메인 창과 SDL 사각형을 번갈아 | 둘 다 30초 내내 펌웨어 로고 한 장 |
| 3초 전경 전환 | SetForegroundWindow 하나만 | 멈춤 |
| 아무것도 안 함 | 캡처도 9초부터 | 멈춤 |
| 1a86b28 빌드 | 위와 같음 | 멈춤 |

열한 번 중 두 번만 살아 있었다. 코드 차이도, 키도, 전경 전환도, 캡처 방식도 원인이 아니었다. 멈춘 창에 숨김·표시,
1픽셀 크기 변경, `RedrawWindow`, 크기 변경과 다시 그리기의 조합을 주어도 풀리지 않았다.

### 제품 밖에서

| 실행 | 조건 | 결과 |
|---|---|---|
| 독립 QEMU | 같은 인자, 최상위 창 | 6초에 GRUB, 26초까지 시간 초과 막대 진행 |
| 독립 QEMU + x1의 efivars | 키 폭주를 겪은 변수 저장소 | 20초 넘게 펌웨어 로고. 새 템플릿으로 바꾸면 정상 |
| WinForms 부모, DPI 인식 없음 | 3초 뒤 SetParent | 정상 |
| WinForms 부모, PMv2(192) | 3초 뒤 SetParent, SDL 창은 96 | 한 번은 멈춤, 한 번은 정상(무작위) |
| WinForms 부모, PMv2 + SDL 창도 PMv2 | `SDL_WINDOWS_DPI_AWARENESS=permonitorv2`로 QEMU 실행 | 세 번 모두 정상, 캡처마다 시간 초과 막대가 진행 |

### PMv2 SDL 창을 제품에 담았을 때 (2026-09-29 오후)

| 실행 | 조건 | 결과 |
|---|---|---|
| x2 완료 실행 | 환경 변수 적용된 제품 | 60초 내내 무대 자리표시자만 보임(SDL 창은 최상위 자식이고 보이는 상태) |
| PrintWindow | SDL 자식 자체를 그리게 함 | 검은색. 창 영역은 1744×1216, DPI 192 |
| 웹뷰 창 영역에서 무대를 뺌 | `SetWindowRgn(WRY_WEBVIEW, 전체 − 무대)` | SDL 자식의 펌웨어 화면이 살아서 드러남. 다만 1024×768 원본 크기로 왼쪽 아래에 그려짐 |
| 자식 크기 1픽셀 변경 뒤 복원 | 외부 크기 변경 | 뷰포트가 무대 크기로 늘어남(좌우 레터박스) |
| WinForms PMv2 부모, 혼합 호스팅 켬 | SDL PMv2 | 정상. 혼합 호스팅은 관계없음 |

SDL 2.32.10 소스로 확인한 원인: QEMU는 게스트 해상도가 바뀔 때 `SDL_SetWindowSize`를 부르고, SDL은 내부 크기
기록을 먼저 바꾼 뒤 `SetWindowPos(hwnd, HWND_NOTOPMOST, …)`를 호출하며 실패를 무시한다. 자식 창에서는 이 호출이
실패하므로 기록(1024×768)과 실제 창(1744×1216)이 어긋나 뷰포트가 원본 크기로 남는다. 그리고 WebView2의
DirectComposition 시각은 최상위 창의 일반 그리기 위에 합성되므로 형제인 SDL 자식을 덮는다. DPI 인식이 없는 자식은
DPI 가상화 표면으로 그 위에 보였지만 그 표면이 무작위로 멈춘 것이다.

### 2안: 메인 창이 소유한 최상위 팝업

| 실행 | 조건 | 결과 |
|---|---|---|
| WinForms 소유자(PMv2) 위에 SDL PMv2 창을 WS_POPUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW 소유 팝업으로 배치 | 클릭 포함 30초 | 계속 살아 있고 선명함. QEMU가 창을 1024×768로 스스로 바꿈(창 사각형으로 드러남). 클릭해도 팝업이 전경을 가져가지 않음 |

## 판정

1. DPI 인식이 없는 SDL 창(96)을 PMv2 부모(192) 아래에 담으면 표시가 무작위로 멈춘다. 재부모화 전에 그린 마지막
   프레임(검은색 또는 펌웨어 로고)이 남고, 게스트가 virtio-gpu 스캔아웃으로 바꿀 때까지 새 프레임이 화면에 닿지 않는다.
   제품 코드와 무관하며 3라운드 커밋 빌드도 같다. 3라운드가 살아 있던 것은 무작위의 한쪽이었다.
2. QEMU 프로세스에 `SDL_WINDOWS_DPI_AWARENESS=permonitorv2`를 주면 SDL이 창을 PMv2로 만들어 부모와 같아지고
   멈춤이 사라진다(3/3). SDL 2.24 이상이 비디오 초기화 때 이 환경 변수를 읽는다.
3. 펌웨어 단계에 닿은 키 폭주는 EFI 변수 저장소를 바꿔 다음 부팅의 펌웨어 단계를 20초 넘게 늘렸다. 키는 GRUB 메뉴를
   인식한 뒤에만 보내야 한다.
4. GRUB 메뉴의 시간 초과는 약 30초다. 드라이버가 서둘러 끊을 필요가 없다.

## 결정

- 게스트 창은 재부모화하지 않고 메인 창이 소유한 최상위 팝업(WS_POPUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW)으로
  무대 위에 놓는다(ADR-0005의 2안, `docs/adr/0009-guest-window-is-an-owned-popup.md`). QEMU가 스스로 크기를 바꾸면
  런타임의 1초 틱이 다시 배치한다. 오버레이 창이 팝업 위, 팝업이 메인 창 위다.
- 제품은 QEMU를 `SDL_WINDOWS_DPI_AWARENESS=permonitorv2`로 띄운다(`ome-guest-config`의 호출 환경, `ome-platform-win`의
  환경 블록, `ome-supervisor`의 어댑터. `.cmd` 로그의 둘째 줄에 환경을 적는다). 팝업이 물리 픽셀로 선명하게 그려진다.
- 완료 실행 드라이버는 창이 뜬 직후의 HOME 반복을 없애고, OCR로 메뉴를 인식한 뒤 HOME 한 번으로 카운트다운을 멈춘 다음
  항목을 고른다.

## 실제 게스트 검증 (워커 Z2, 2026-09-29)

테스트는 `browser_keyboard_and_mouse_reach_unfocused_real_guest`다. 실행 명령은
`cargo test -p ome-runtime --test keyboard_passthrough_real -- --ignored --nocapture`였다.

| 항목 | 측정값 | 판정 |
|---|---|---|
| (a) 소유 팝업 속성 | `owned_popup_top_level=true`, `owned_popup_owner_matches_host=true`, `owned_popup_no_activate=true` | 통과. 실제 `default` 게스트 창은 최상위 창이고 테스트 호스트가 소유하며 `WS_EX_NOACTIVATE`가 있었다. |
| (b) 부팅 중 배치 복원 | 40,001 ms 동안 틱 178회, 틱 사이 최댓값 4,349 ms, 틱 직전 사각형 불일치 0회, 최대 편차 0 px, 복원 감지 0회. 부팅 뒤 실제·예상 사각형은 모두 `(108, 131, 1280×720)`이었다. 별도로 `[stage] restored guest popup placement after a guest resize`가 2회 출력됐다. | 사각형은 부팅 뒤 일치했다. 하지만 adb 부팅 확인과 능력 조사가 틱 스레드를 막아 250 ms 주기를 지키지 못했다. 부팅 확인만 작업 스레드로 옮긴 뒤에도 능력 조사 때문에 최댓값은 4,349 ms였다. |
| (c) 실제 클릭 | 화면 좌표 `(748, 491)`에서 `window_at_click_class=SDL_app`. `ABS_X` 2회는 290 ms와 390 ms에 도착했지만 `pointer_down_events=0`이었다. 클릭 직후 `foreground_immediately_after_click_is_guest=true`였고, `GuestWindowToFront` 보정 뒤에는 `foreground_after_click_correction_is_guest=false`, `focus_after_click_correction_is_guest=false`였다. | 실패. 실제 클릭은 좌표만 전달했고 버튼 DOWN은 전달하지 않았다. `WS_EX_NOACTIVATE`도 SDL 창의 활성화를 막지 못했다. 호스트 보정은 전경과 포커스를 되돌렸다. |
| (d) 웹뷰 키 입력 | `key_a_down_events=1`, `key_a_up_events=1`, 16 ms | 통과. `ingest_browser_key("KeyA", ...)`의 누름과 뗌이 모두 게스트에 도착했다. |

검증 과정에서 제품 결함 세 가지를 확인해 최소한으로 고쳤다. `is_top_level`이 소유자를 자식 부모로 잘못
판정하던 문제는 `GA_ROOT`로 검사하도록 바꿨다. `GuestWindowToFront`가 팝업만 다시 배치하고 전경과 포커스를
호스트로 돌려주지 않던 문제는 소유자 스레드에서 호스트를 활성화하도록 고쳤다. 그리고 동기 adb 부팅 확인이 틱을
최대 15초 막던 문제는 부팅 확인을 작업 스레드로 옮겼다. 능력 조사는 아직 동기 실행이라 (b)의 250 ms 조건을
만족하지 못한다. (c)의 클릭 활성화와 버튼 DOWN 누락도 남아 있으므로 이 검증은 전체 통과가 아니다.

## 클릭 활성화의 원인 (2026-09-29 밤)

Z2의 (c)에서 팝업이 `WS_EX_NOACTIVATE`인데도 클릭에 전경을 가져갔고 버튼 DOWN이 게스트에 닿지 않았다. 두 현상은 하나의
원인에서 나온다. SDL은 창이 전경이 되는 순간 눌려 있던 버튼을 활성화용 클릭으로 기록하고(`focus_click_pending`,
`src/video/windows/SDL_windowsevents.c`의 `WIN_UpdateFocus`와 `WIN_CheckWParamMouseButton`), 힌트
`SDL_MOUSE_FOCUS_CLICKTHROUGH`가 없으면 그 누름을 버린다. 그러므로 활성화만 막으면 버튼도 전달된다.

활성화의 원인은 QEMU나 SDL이 아니라 창의 소유 관계다. 이 세션의 실험 스크립트는 이 문서와 같은 이름의 폴더에 있다
(`probe-noactivate.ps1`, `probe-noactivate-x.ps1`, `probe-noactivate-child.ps1`, `probe-click3.ps1`). 모두 PMv2 프로세스에서
`SendInput`으로 왼쪽 버튼을 누르고 400~600 ms 뒤 `GetForegroundWindow`를 읽는다. 처음 돌린 대조 실험은 `INPUT` 구조체 크기를
잘못 잡아 `SendInput`이 아무것도 보내지 않았고(반환값 0), 아래 표는 그것을 고쳐 반환값 1을 확인한 뒤의 결과다.

| 창 | 만든 곳 | 소유자 | 창 프로시저 | 클릭 뒤 전경 |
|---|---|---|---|---|
| WinForms 폼, 생성 때 `WS_EX_NOACTIVATE` | 같은 프로세스 | 있음 | WinForms 기본 | 팝업 |
| Win32 창, 생성 때 `WS_EX_NOACTIVATE` | 같은 프로세스 | 있음 | `DefWindowProc` | 팝업 |
| Win32 창, 생성 뒤 제품과 같은 방식으로 전환 | 같은 프로세스 | 있음 | `DefWindowProc` | 팝업 |
| 위와 같음 | 다른 프로세스 | 있음 | `DefWindowProc` | 팝업 |
| 위와 같음, `WM_LBUTTONDOWN`에서 `SetCapture` | 다른 프로세스 | 있음 | `DefWindowProc` | 팝업 |
| Win32 창, 생성 때 `WS_EX_NOACTIVATE` | 같은 프로세스 | 없음 | `DefWindowProc` | 소유자(활성화 안 됨) |
| Win32 창, 생성 뒤 전환 | 같은 프로세스 | 있음 | `WM_MOUSEACTIVATE`에 `MA_NOACTIVATE`를 답함 | 소유자(활성화 안 됨) |
| QEMU SDL 창(펌웨어만, 게스트 없음) | QEMU | 있음 | SDL | 팝업 |

소유자가 없으면 `WS_EX_NOACTIVATE`가 문서대로 동작하고, 소유자가 있으면 스타일과 무관하게 클릭이 팝업을 활성화한다. 이때
소유자 창은 `WM_MOUSEACTIVATE`를 받지 않으므로(소유자 프로시저에서 세어 0회) 소유자 쪽에서 막을 수 없다. 막는 방법은 팝업
자신의 창 프로시저가 `WM_MOUSEACTIVATE`에 `MA_NOACTIVATE`를 답하는 것뿐이다. 클릭은 그대로 전달된다(`MA_NOACTIVATE`는
메시지를 버리지 않는다).

QEMU의 SDL 창 프로시저는 SDL2.dll 안에 있으므로 제품 쪽에서 바꿀 수 없다. 그래서 QEMU 패치 0004로 `-display
sdl,activate-on-click=off`를 더한다. 윈도우에서 이 옵션이 꺼져 있으면 `ui/sdl2.c`가 창을 만든 직후 HWND를 얻어 창
프로시저를 덧씌우고 `WM_MOUSEACTIVATE`에만 `MA_NOACTIVATE`를 답하며 나머지는 SDL의 프로시저에 넘긴다. 제품은 담긴
게스트에만 이 옵션을 붙이고(`GuestConfig.hosted_window`), 런처와 인자 대조 픽스처는 그대로다. 소유 관계를 버리는 대안
(소유자 없는 팝업)은 활성화는 막지만 메인 창을 클릭할 때마다 팝업이 한 프레임 뒤로 갔다 오고 최소화도 따로 처리해야 해서
택하지 않았다. `SDL_MOUSE_FOCUS_CLICKTHROUGH=1`로 버튼만 살리는 대안은 활성화 자체를 두므로 웹뷰가 키보드를 잃는다.
