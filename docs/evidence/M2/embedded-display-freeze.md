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

## 패치 0004 뒤의 재검증 (워커 Q1, 2026-09-29)

제품 밖의 WinForms 소유자 위에 QEMU SDL 창을 소유 팝업으로 놓고 `SendInput`으로 클릭했다. 패치 옵션을 켠 두 번의 실행과 옵션을 뺀 대조 실행에서 다음 값을 얻었다.

```text
patched-1 hover=owner afterDown=owner afterUp=owner sent=1/1
patched-2 hover=owner afterDown=owner afterUp=owner sent=1/1
control hover=owner afterDown=GUEST afterUp=GUEST sent=1/1
```

실제 `default` 게스트로 `keyboard_passthrough_real` 검사를 돌려 다음 값을 얻었다. 클릭 직후에도 전경은 게스트가 아니었고, 버튼 누름과 KeyA 누름·뗌이 게스트에 도착했다.

```text
foreground_immediately_after_click_is_guest=false
pointer_position_events=2 pointer_down_events=1
native_pointer_button_down_verified=true
foreground_after_click_correction_is_guest=false
focus_after_click_correction_is_guest=false
key_a_down_events=1 key_a_up_events=1
```

새 QEMU 실행 파일은 `C:\Open Mobile Emulator\qemu-build\out\bin\qemu-system-x86_64.exe`다. 수정 시각은 `2026-09-29 23:12:13.840527000 +0900`, SHA-256은 `38ded7952400ad9163993beb4206ea809204fca5eabe212558a18f4b3ce98f96`이다. 외부 QEMU 체크아웃의 기준 커밋은 다음과 같다.

```text
c3d48b7 Update version for 11.1.1 release
```

## 표시 멈춤의 원인 (2026-09-30 새벽)

패치 0004를 넣은 뒤의 완료 실행 1회차는 GRUB이 선명하게 보였지만 3·4회차는 GRUB이 떠 있는 30초 내내 캡처가
검었고 커널이 virtio-gpu 스캔아웃을 잡은 뒤에야 살아났다. 같은 코드에서 결과가 갈렸으므로 제품 밖에서 조건을
하나씩 재었다. 스크립트는 같은 이름의 폴더의 `probe-freeze.ps1`이다. 1회차가 남긴 설치 ISO 명령줄을 새 EFI 변수
저장소와 `-snapshot`으로 띄우고(GRUB 메뉴가 4초부터 약 30초 머문다), 소유자로 쓸 WinForms 창을 팝업과 겹치지 않는
곳에 두고, 8·14·20초에 팝업 영역을 화면에서 복사해 검지 않은 픽셀의 비율을 쟀다. GRUB 메뉴(사진 배경)가 살아
있으면 98~99 %이고, 멈춘 창은 마지막 프레임에 따라 0~39 %의 일정한 값이 나온다. 처음 두 묶음은 1회차의 EFI 변수
저장소를 그대로 써서 펌웨어가 빈 ESP를 먼저 시도하느라 20초 넘게 걸렸고, 소유자 창을 검게 칠하고 팝업을 그 위에
두어 소유되지 않은 창이 가려졌으므로 표에서 뺐다.

| 바깥에서 한 일 | 시점 | 살아 있음 / 시도 |
|---|---|---|
| 아무것도 하지 않음 | | 4 / 4 |
| 위치만 옮김(`SetWindowPos`, `SWP_NOSIZE`), 6초에 한 번 더 | 0.7초 | 3 / 3 |
| z-순서만 바꿈(맨 아래로, 다시 맨 위로) | 6초 | 3 / 3 |
| 크기를 바꿈(스타일·소유자 그대로), QEMU가 크기를 바꿀 때마다 되돌림 | 0.7초 | 3 / 6 |
| 크기를 한 번만 바꿈(QEMU의 모드 전환이 끝난 뒤) | 6초 | 1 / 3 |
| 스타일만 바꿈(WS_POPUP, NOACTIVATE, TOOLWINDOW, `SWP_FRAMECHANGED`) | 0.7초 | 0 / 2 |
| 소유자만 지정(`GWLP_HWNDPARENT`) | 0.7초 | 1 / 4 |
| 제품과 같은 전환(스타일, 소유자, 크기) | 0.7초 | 0 / 4 |
| 같은 전환, `SWP_NOCOPYBITS` 추가 | 0.7초 | 0 / 3 |
| 같은 전환, `SWP_FRAMECHANGED` 없이 | 0.7초 | 0 / 2 |
| 발견 즉시 숨기고 크기가 1.5초간 안정된 뒤 전환 | 약 4초 | 2 / 4 |
| 전환 뒤 6초에 크기를 2픽셀 늘렸다 되돌림 | | 1 / 2 |
| SDL의 DPI 인식을 없음 또는 시스템으로 바꾸고 전환 | 0.7초 | 1 / 4 |

멈춘 창은 `PrintWindow(PW_RENDERFULLCONTENT)`로 떠도 같은 프레임이 나오므로 캡처 방법의 문제가 아니라 DWM에 새
프레임이 닿지 않는 것이다. 결론은 세 가지다. 호스트가 SDL 창의 스타일이나 소유자를 바꾸면 거의 항상 굳고, 크기를
바꾸면 절반쯤 굳으며, 옮기거나 z-순서를 바꾸는 것은 안전하다. QEMU 자신의 `SDL_SetWindowSize`는 수십 번의 부팅에서
한 번도 굳지 않았다. 그러므로 제품은 창의 스타일·소유자·크기를 바깥에서 건드리지 않아야 한다. 스타일과 소유자는
QEMU가 GL 컨텍스트를 만들기 전에 정하고, 크기와 표시 여부는 QEMU가 QMP 명령을 받아 SDL로 스스로 바꾼다. 이것이
QEMU 패치 0005(`-display sdl,owner-window=<HWND>`, QMP `x-ome-display-window`)다. 제품에는 위치 이동과 z-순서만
남는다.


## 패치 0005 검증 (워커 Q2a, 2026-09-30)

(이 절의 실패는 모니터 절전 중에 잰 것이라 무효다. 다음 두 절을 보라. 기록은 그대로 둔다.)

QEMU v11.1.1 트리의 `qapi/ui.json`과 `ui/sdl2.c`에 패치 0005를 적용했다. `owner-window`를 주면 SDL 창을
숨김 상태의 테두리 없는 소유 팝업으로 만들고, GL 문맥을 만들기 전에 소유자와 `WS_EX_NOACTIVATE`,
`WS_EX_TOOLWINDOW`, `WM_MOUSEACTIVATE` 처리를 설정한다. 게스트 해상도가 바뀌어도 창 크기를 바꾸지 않으며,
`x-ome-display-window` QMP 명령이 SDL을 통해 위치, 크기, 표시 상태를 바꾼다.

새 실행 파일의 SHA-256은
`7aef44c57e9c5fe7091637c94aba772ddaa3b0cbcd6af0537a3994f4624b076b`다. 외부 QEMU 체크아웃의 기준
커밋은 다음과 같다.

```text
c3d48b7 Update version for 11.1.1 release
```

빌드는 fingerprint 파일을 따로 보관한 뒤 configure 83초, build 53초에 성공했다. 첫 dist 실행은 마지막 디렉터리
교체에서 `Permission denied`로 실패했고, 같은 명령을 다시 실행하자 57초에 성공했다.

### 독립 프로브 결과

프로브는 완료 실행 1라운드의 설치 ISO 명령줄과 실행마다 새로 복사한 EFI 변수 저장소를 썼다. 호스트는 Win32로
창 속성, 표시 상태, 사각형만 읽었다. 창의 위치, 크기, 표시 상태는 QMP 명령으로만 바꿨다.

처음 만든 프로브는 QEMU 프로세스의 숨겨진 `SDL_app` 가운데 임시 창을 골랐다. 첫 QMP 요청은 성공했지만 그 임시
창이 사라져 `GetWindowRect failed`로 끝났다. 화면 표시 상태를 재기 전의 프로브 오류였으며, 소유자가 WinForms 폼과
일치하는 `SDL_app`을 고르도록 바로잡았다.

첫 유효 실행은 창 생성과 QMP 처리까지 통과했지만 화면 표시 검증에서 실패했다.

```text
window_at=1669ms hidden_initial=True hwnd=10750800 owner_hwnd=10291966
qmp_greeting={"QMP":{"version":{"qemu":{"micro":1,"minor":1,"major":11},"package":"v11.1.1-dirty"},"capabilities":["oob"]}}
qmp_request={"execute":"qmp_capabilities","id":1}
qmp_reply={"return":{},"id":1}
qmp_request={"execute":"x-ome-display-window","id":2,"arguments":{"visible":true,"y":500,"x":900,"width":1280,"height":720}}
qmp_reply={"return":{},"id":2}
first_state rect=900,500 1280x720 expected=900,500 1280x720 rect_ok=True top_level=True owner=10291966 owner_ok=True popup=True no_caption=True noactivate=True toolwindow=True visible=True
qmp_request={"execute":"x-ome-display-window","id":3,"arguments":{"visible":true,"y":550,"x":950,"width":1440,"height":810}}
qmp_reply={"return":{},"id":3}
second_state rect=950,550 1440x810 expected=950,550 1440x810 rect_ok=True top_level=True owner=10291966 owner_ok=True popup=True no_caption=True noactivate=True toolwindow=True visible=True
capture=C:\Users\32170336\AppData\Local\Temp\claude\C--Open-Mobile-Emulator\7758e147-b2c0-5152-846b-245148981fae\scratchpad\probe-click\owner-run-1-14s-screen.png
RESULT tag=run-1 hide_until_6s=False lit=8s=21.48% 14s=21.48% 20s=21.48%
Exception: non-black share was below 98 percent
```

창은 요청한 두 사각형과 소유 관계, 스타일, 표시 비트를 모두 갖췄다. 하지만 비검정 비율은 세 번 모두 21.48%로
같았고 필수 기준인 98%에 미치지 못했다. 14초 캡처에는 SDL 화면 대신 호스트 바탕 화면이 있었다. 작업 지시서의
중단 조건에 따라 나머지 일반 실행 세 번과 처음에는 숨겼다가 6초에 표시하는 대조 실행은 하지 않았다. 따라서 패치
0005의 화면 표시 문제는 해결되지 않았고, 독립 프로브 검증은 실패했다.

## 측정을 오염시킨 조건: 모니터 절전 (2026-09-30 02:20~02:50)

위 표를 만든 뒤 순정 QEMU 창(아무것도 하지 않음)마저 8·14·20초 모두 검게 재는 시간대가 생겼다. 같은 순간 QMP
`screendump`는 GRUB 메뉴를 그대로 돌려줬고(스크립트 `probe-screendump.ps1`), 창은 그 자리의 맨 위에 있었으며
(`WindowFromPoint`), DWM은 창을 숨김(cloaked)으로 보고하지 않았다. GRUB에 방향키를 QMP로 보내 게스트가 화면을
바꾸게 해도 창은 그대로였다. 시스템 이벤트 로그의 Kernel-Power 566은 이 시간대에 `InputHid` 깨움이 제 클릭
실험과 같은 시각에 찍히고 01:14 `SessionUnlock` 뒤로는 아무 입력도 없다고 적었다. 모니터가 절전에 들어가면 DWM이
합성을 멈추고 GDI 화면 복사는 마지막으로 합성된 내용을 돌려주므로, 창이 살아 있어도 검게 재진다.

확인 실험은 QEMU를 띄우기 전에 `SendInput`으로 마우스를 1픽셀 움직여 모니터를 깨우고
`SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED)`로 붙잡은 채 같은 측정을 하는 것이다.

| 조건 | 모니터 절전 중 | 모니터 깨운 뒤 |
|---|---|---|
| 아무것도 하지 않음 | 12~14 % (2/2 검음) | 98 %, 98 % (2/2 살아 있음) |
| 제품과 같은 전환(스타일, 소유자, 크기) | 25 % | 25 %, 26 % (2/2 굳음) |

그러므로 위 표에서 같은 묶음 안에 '아무것도 하지 않음' 대조군이 살아 있던 묶음(옮기기, z-순서, 크기 한 번, 스타일만,
소유자만, 제품과 같은 전환의 첫 측정)은 유효하고, 대조군 없이 잰 묶음(NOCOPYBITS, FRAMECHANGED 없이, DPI 인식 변경)은
절전 여부를 알 수 없어 참고만 한다. 워커 Q2a가 패치 0005의 생성 시점 변경을 하나씩 끄며 잰 첫 행렬은 순정 대조군까지
검었으므로 무효이고, 모니터를 깨운 채 다시 잰다. 완료 실행 3·4회차가 GRUB 30초 동안 검게 찍힌 것도 같은 원인일
가능성이 크다. 그 두 회차는 GRUB 인식 전까지 HID 입력을 보내지 않는다.

대책은 두 가지다. 완료 실행 드라이버는 `ci/dod/keep-awake.ps1`을 띄워 실행 내내 모니터를 붙잡고, 제품은 게스트가
도는 동안 `ES_DISPLAY_REQUIRED`를 잡는다(게임을 보는 사용자의 화면이 꺼지지 않게 하는, 동영상 재생기와 같은 동작).

## 패치 0005 검증, 모니터를 깨운 채 (워커 Q2a 2차, 2026-09-30 03:00~03:30)

워커 Q2a의 첫 검증(위 "패치 0005 검증" 절)과 생성 시점 변경을 하나씩 끈 첫 행렬은 모니터가 절전 중이라 무효였다.
같은 프로브(`probe-owner.ps1`)에 QEMU를 띄우기 전 마우스 1픽셀 이동과 `ES_DISPLAY_REQUIRED`를 넣고 다시 쟀다.
창은 WinForms 소유자 위에 `-display sdl,...,owner-window=<HWND>`로 만들고, QMP `x-ome-display-window`로
(900,500) 1280×720에 보인 뒤 6초에 (950,550) 1440×810으로 다시 놓았다. 값은 8·14·20초의 검지 않은 픽셀 비율이다.
생성 시점 변경을 끄는 임시 환경 변수는 진단 빌드에만 있었고 패치 파일과 최종 빌드에는 없다.

| 실행 | 끈 변경 | 8초 | 14초 | 20초 | 두 사각형 일치 |
|---|---|---|---|---|---|
| a1, a2 | 다섯 가지 전부(순정 창에 가까움) | 99.0 % | 99.3 % | 99.5 % | 테두리 있음(예상대로 불일치) |
| b1r2, b2 | 숨김 | 99.1 % | 99.4 % | 99.5 % | 일치 |
| c1, c2 | 테두리 없음 | 99.0 % | 99.3 % | 99.5 % | 테두리 있음 |
| d1, d2 | 소유자 | 99.0 % | 99.3 % | 99.5 % | 일치(소유자 불일치) |
| e1, e2 | 확장 스타일 | 99.0 % | 99.3 % | 99.5 % | 일치 |
| f1, f2 | 프레임 재계산 | 99.0 % | 99.3 % | 99.5 % | 일치 |
| g1r2, g2 | 숨김과 프레임 재계산 | 99.1 % | 99.4 % | 99.5 % | 일치 |
| full1, full2r2 | 없음(패치 그대로) | 99.1 % | 99.3 % | 99.5 % | 일치, 최상위, 소유자 일치, WS_POPUP, 캡션 없음, NOACTIVATE, TOOLWINDOW |
| hide | 없음, 첫 명령은 visible=false, 6초에 true | 99.1 % | 99.4 % | 99.5 % | 6초 전 숨김, 뒤 표시 |

모든 실행이 살아 있었고 QEMU 안에서 한 두 번째 크기 변경 뒤에도 그대로였다. 그러므로 패치 0005의 방식(창을 만들 때
소유자와 스타일을 정하고, 크기와 표시 여부는 QEMU가 SDL로 바꿈)은 표시를 굳히지 않는다. 같은 시간대에 제품 방식의
사후 전환은 2/2 굳었으므로(앞 절), 차이는 호스트가 창을 바깥에서 바꾸느냐에 있다. 최종 빌드의 실행 파일 SHA-256은
`7aef44c57e9c5fe7091637c94aba772ddaa3b0cbcd6af0537a3994f4624b076b`, QEMU 기준 커밋은 `c3d48b7`, 외부 트리의
인덱스는 HEAD에 0001~0005만 더한 상태다(임시 토글은 걷어냈다).

## 패치 0005를 쓰는 제품 검증 (워커 Q2b, 2026-09-30 03:28~03:37)

제품을 패치 0005에 맞춰 고쳤다. 게스트 설정(`ome-guest-config`)은 `owner_window`가 있으면 `-display sdl,...,owner-window=<HWND>`를
붙이고 이때 `activate-on-click=off`는 붙이지 않는다(패치 0005가 그 동작을 포함한다). 창 담기(`ome-window-host`)는 숨은 창을
포함해 QEMU pid의 `SDL_app` 창을 찾아 최상위이고 소유자가 메인 창인지 확인한 뒤, 무대의 화면 좌표를 계산해
`DisplayWindowGeometry`를 돌려주고 z-순서만 바꾼다. 런타임은 그 기하를 감독자(`Supervisor::set_display_window`)를 거쳐 QMP
`x-ome-display-window`로 보내고, 1초 틱마다 `resync`가 실제 창 사각형이 기대와 다르면 다시 보낸다. 무대가 가려지면
`visible=false`로, 다시 보이면 `visible=true`로 같은 명령을 보낸다. 호스트는 창의 스타일, 소유자, 크기를 건드리지 않는다.
게스트가 Running인 동안 런타임은 `DisplayKeepAwake`(전용 스레드의 `SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED)`)를
쥐고, Running을 벗어나면 놓는다.

검증은 `ome-runtime/tests/keyboard_passthrough_real.rs`(개발 게스트 `default`)로 두 번 했고 둘 다 통과했다. 실행 파일은
Q2a 2차 검증과 같은 `7aef44c5…`다. 각 실행에서 (a) 부팅 뒤 창 속성과 사각형, (b) `GuestStart` 뒤 15초 동안 0.5초마다 팝업
영역의 밝은 픽셀 비율(`screen_region_lit_share`, 12픽셀 간격 표본, R+G+B > 60), (c) 실제 클릭, (d) 웹뷰 키를 쟀다.

| 항목 | 1회 | 2회 |
|---|---|---|
| 첫 기하 전송 | 시작 뒤 910 ms, (113,158) 1280×720, visible | 903 ms, 같음 |
| 부팅 뒤 창 | 최상위, 소유자 일치, `WS_EX_NOACTIVATE`, 실제 사각형 (113,158) 1280×720 = 보낸 값 | 같음 |
| 시작 40초 동안의 틱 | 194회, 보이는 사각형 불일치 0, 최대 편차 0 px, 가장 긴 틱 간격 4477 ms | 196회, 0, 0 px, 3985 ms |
| 보낸 기하 수 | 2 (첫 배치 1, 틱의 재전송 1) | 2 |
| 밝은 비율 최대 / 연속 변화 | 0.998 / 12회 | 0.998 / 11회 |
| `to_front` 뒤 포그라운드 | 메인 창(게스트 아님) | 같음 |
| 클릭 (753,518), `WindowFromPoint` = `SDL_app` | ABS_X, ABS_Y, BTN_MOUSE DOWN 도달(433 ms), 클릭 직후와 뒤 모두 포그라운드는 메인 창 | 같음(432 ms) |
| KeyA (`ingest_browser_key`) | DOWN 1, UP 1, 8 ms | DOWN 1, UP 1, 44 ms |
| 종료 | Stopped | Stopped, `UserStop` |

밝은 비율의 흐름은 두 실행이 같다. 1.1초 0.103(펌웨어 로고), 2.6초 0.990(GRUB 메뉴의 사진 배경), 8초 0.633과 9초 0.26(GRUB이
넘어가며 커널 메시지), 11초부터 0.004(커널 부팅 중의 검은 콘솔), 14.6초 0.000. 값이 계속 바뀌므로 창은 살아 있고, 뒤쪽의
낮은 값은 굳은 것이 아니라 게스트 화면이 검은 것이다. 1회 실행의 원문 일부다.

```text
liveness_sample t=+1071ms lit=0.103
liveness_sample t=+2572ms lit=0.990
liveness_sample t=+7575ms lit=0.998
liveness_sample t=+8075ms lit=0.633
liveness_sample t=+9076ms lit=0.260
liveness_sample t=+11077ms lit=0.004
liveness_samples_shown=28 liveness_samples_not_shown=2 liveness_max=0.998 liveness_consecutive_changes=12
bootCompleted=true
owned_popup_top_level=true
owned_popup_owner_matches_host=true
owned_popup_no_activate=true
startup_ticks=194 startup_visible_rect_mismatch_count=0 startup_largest_deviation_px=0 startup_largest_tick_interval_ms=4477 startup_elapsed_ms=40170
geometries_sent=2 first_sent_after_start_ms=910 first_sent=113,158 1280x720 visible=true
popup_rect_after_boot_actual=113,158 1280x720 sent=113,158 1280x720 visible=true
foreground_after_front=0x1880936
focus_after_to_front_is_guest=false
click_screen=753,518
window_at_click_class=SDL_app
foreground_immediately_after_click_is_guest=false
getevent +102ms /dev/input/event2: EV_ABS       ABS_X                00003e65
getevent +338ms /dev/input/event2: EV_KEY       BTN_MOUSE            DOWN
pointer_position_events=2 pointer_down_events=1
native_pointer_button_down_verified=true
getevent +2ms /dev/input/event3: EV_KEY       KEY_A                DOWN
getevent +8ms /dev/input/event3: EV_KEY       KEY_A                UP
key_a_down_events=1 key_a_up_events=1
```

기하가 흐르는 경로의 디버그 추적(`[stage]`)이다. 첫 명령은 `place`가 돌려준 기하이고, 두 번째는 첫 틱의 `resync`가 창이 아직
그 사각형에 놓이기 전에 잰 차이를 다시 보낸 것이다.

```text
[stage] supervisor enqueue display-window state=Running epoch=1 geometry=DisplayWindowGeometry { x: 113, y: 158, width: 1280, height: 720, visible: true }
[stage] supervisor dequeue display-window state=Running worker_epoch=1 message_epoch=1
[stage] qmp command=x-ome-display-window x=113 y=158 width=1280 height=720 visible=true
[stage] send guest window geometry=DisplayWindowGeometry { x: 113, y: 158, width: 1280, height: 720, visible: true } result=ok
[stage] resent guest window geometry
[stage] qmp reply=x-ome-display-window ok
```

### 사전 릴리스 ome3의 실행 파일로 다시 검증 (2026-09-30 07:22)

`qemu-v11.1.1-ome3`의 실행 파일(`7139f05b…`, `qemu-build/out/bin`의 것과 같다)을 디버그 번들에 넣고 같은 테스트를 돌렸다.
처음 두 번은 실패했다. 밝은 비율 표본이 한 번은 28개 전부 0.000, 한 번은 전부 1.000이었고 변화가 없었다. 시스템 로그의
Kernel-Power 566은 07:14:19 `SessionUnlock` 뒤로 입력이 없다가 07:22:13 `InputHid`로 세션이 바뀌었다고 적었는데, 그 시각은
모니터 깨우기 도우미(`ci/dod/keep-awake.ps1`)가 마우스를 1픽셀 움직인 순간이다. 그러므로 두 실패는 앞 절과 같은 교란, 곧
모니터가 절전 중일 때 잰 값이다. 이 PC는 원격 브리지로 조작하므로 무인 실행 중에는 모니터가 꺼진다.

도우미로 화면을 깨워 붙잡은 채 이전 실행 파일(`7aef44c5…`)과 ome3를 잇달아 돌리자 둘 다 통과했다.

| 실행 파일 | 시작 | 밝은 비율 최대 / 연속 변화 | 부팅 뒤 창 | 클릭 | KeyA | 결과 |
|---|---|---|---|---|---|---|
| `7aef44c5…` (Q2b가 쓴 것) | 07:22:16 | 0.998 / 12회 | 최상위, 소유자 일치, NOACTIVATE, (113,158) 1280×720 | BTN DOWN 도달 | DOWN 1, UP 1 | 통과, 46.0 s |
| `7139f05b…` (ome3) | 07:23:03 | 0.998 / 12회 | 같음 | 같음 | 같음 | 통과, 45.6 s |

밝은 비율의 흐름도 두 실행이 같다(1.1초 0.103, 2.6초 0.990, 8초 0.633, 11초 0.004). 교훈은 하나다. 이 테스트를 비롯한
화면 캡처 기반 측정은 모니터가 켜져 있어야 뜻이 있고, 제품의 `DisplayKeepAwake`는 절전을 막을 뿐 이미 꺼진 모니터를 켜지는
않는다. 무인 실행 전에는 도우미로 깨워야 하며, 완료 실행 드라이버는 시작할 때 그렇게 한다.

## 제품 경로의 완료 실행 7~11회차와 표시 시점 프로브 (2026-09-30 07:29~08:23, 21:09)

패치 0005와 ome3 실행 파일(`7139f05b…`)을 쓰는 제품으로 지역 완료 실행을 네 번 돌렸다. 네 번 모두 게스트 시작 0.9초 뒤에
기하 하나(`visible=true`)만 보냈고, 그 뒤 60초 동안 기하를 다시 보내지 않았다(앱 출력의 `[stage] send guest window geometry`).
호스트는 창의 스타일, 소유자, 크기를 건드리지 않았다. 세션은 08:23에 호스트가 재시작되면서 끊겼고, 아래 기록은 그 세션의
스크래치패드에서 다음 세션이 정리한 것이다.

| 회차 | 시작 | GRUB 30초 동안 팝업에 보인 것 | 결과 |
|---|---|---|---|
| 7 | 07:29 | 펌웨어의 마지막 텍스트 프레임(`BdsDxe: loading Boot0001 …`)이 07:29:22부터 07:29:50까지 그대로 | GRUB 인식 실패. 캡처 전의 `Focus`(메인 창 SW_RESTORE, 전경, TOPMOST 토글)를 원인으로 보고 eac2c2a로 없앴다 |
| 8 | 07:47 | 검은 프레임. OCR 문자열이 07:47:26부터 07:47:57까지 비어 있음 | GRUB 인식 실패. `Focus`가 없어도 굳는다 |
| 9 | 07:55 | GRUB 메뉴가 선명하게 보임 | GRUB 선택, cfdisk, ESP 선택까지 지나가고 파일 시스템 선택 화면의 OCR 분류에서 멈춤(393e958로 고침) |
| 10 | 08:00 | 검은 프레임. OCR 문자열이 08:00:28부터 08:00:58까지 비어 있음 | GRUB 인식 실패 |

다음 세션이 병합된 main(ffc6a8c, PR #4)으로 돌린 11회차(21:09)는 GRUB이 살아 있었다. 메뉴를 둘째 프레임에서 읽고, 방향키
넷과 Enter로 `Installation`에 들어가 설치기의 안내 상자와 파티션 목록까지 읽었다. 드라이버는 목록의 파란 항목
(`Create/Modify partitions`)이 OCR에서 빠져 `modify`를 180초 기다리다 끝났고(분류기를 힌트 문구로도 알아보게 고쳤다), 그
사이 다른 창이 무대를 덮어 뒤쪽 캡처는 무효다. 제품 경로의 GRUB 생존은 지금까지 다섯 번 중 두 번(9, 11회차)이다.

7, 8, 10회차 모두 GRUB이 끝나고 커널이 문자를 찍는 순간(7회차 07:29:54, 10회차 08:01:00의 `SELinux: … is deprecated`)부터 팝업이
다시 살아났고, 그 뒤 부팅 애니메이션까지 정상이었다. 그러므로 굳는 구간은 정확히 GRUB 메뉴가 떠 있는 동안이다. 펌웨어에서
GRUB으로 넘어가는 표면 전환에서 굳고, GRUB에서 커널로 넘어가는 표면 전환에서 풀린다. 두 전환에서 QEMU가 창에 하는 일은
없다. `sdl2_gl_switch`는 표면 텍스처를 지우고 다시 만들 뿐이고, `sdl2_window_resize`는 owner-window가 있으면 바로 돌아간다
(`ui/sdl2.c` 287~296행).

### 표시 시점 프로브 (`probe-reveal.ps1`, 08:05~08:23)

제품 밖에서 같은 실행 파일을 WinForms 소유자 위에 owner-window로 띄우고, 창이 보이자마자 `visible=false`로 (900,500)
1280×720에 놓은 뒤 정해진 시점에 `visible=true`를 보냈다. 값은 8~20초 사이 표본 가운데 검지 않은 비율이 90 % 이상인 수다.
모니터는 프로브가 깨워 붙잡았다.

| 실행 | 표시 시점 | 결과 |
|---|---|---|
| r2 | 2.02초 | 살아 있음 43/43 |
| r3a | 3.05초 | 살아 있음 44/44 |
| r3b | 3.04초 | 굳음 0/44. 3.9초의 프레임(펌웨어 로고, 25 %)에서 멈춰 GRUB 메뉴(98.8 %)가 끝내 보이지 않음 |
| r6 | 6.04초 | 살아 있음 44/44 (표시했을 때 GRUB이 이미 그려져 있었다) |
| c1 | 3.03초, 10.4초에 QMP 숨김·표시 한 번 | 표시 뒤와 순환 뒤 모두 살아 있음. 두 번째 실행 도중 호스트가 재시작됐다 |

같은 세션이 창 담기에 "첫 배치는 숨긴 채 보내고 2초 뒤 틱에서 드러낸다"는 변경(`REVEAL_DELAY`)을 넣고
`keyboard_passthrough_real`을 한 번 돌렸다. 3.06초에 드러난 뒤 3.1~9.6초의 표본은 팝업 가운데의 창이 팝업이 아니어서
(테스트 호스트가 노출 시점에 자기 최상위 띠를 다시 올린다) 재지 못했고, 10.1초 0.273, 11.1초 0.004(커널 부팅의 검은
콘솔)만 남아 최대 밝기 조건(0.6)에 걸려 실패했다. GRUB 구간을 보지 못했으므로 이 결과로는 판정할 수 없다. 이 변경은
검증 전이라 main에 넣지 않고 `wip/reveal-delay` 브랜치(ebec903)에만 두었다.

### 판정

1. 굳는 데 호스트 쪽 동작은 필요하지 않다. r3b는 QMP 표시 한 번 뒤에 굳었고, 8회차와 10회차의 드라이버는 표시 뒤 첫
   캡처까지 창을 읽기만 했다(`-WaitInstaller`는 창을 찾아 보고만 한다).
2. 표시 시점은 믿을 만한 조절 수단이 아니다. 3초 표시는 4번 중 3번 살았고, 0.9초 표시는 테스트(`keyboard_passthrough_real`,
   Q2b 2회와 ome3 A/B 2회)에서는 4/4 살았는데 완료 실행에서는 4번 중 1번만 살았다.
3. 같은 0.9초 표시인데 테스트와 완료 실행의 결과가 다른 까닭은 아직 모른다. 둘의 차이는 소유자 창(제품의 Tauri·WebView2
   창과 테스트의 맨 Win32 창), 팝업 크기(완료 실행 1744×1216, 테스트 1280×720), 그리고 무대가 있는 마법사 화면이다.
4. 굳은 창은 다음 표면 전환에서 풀린다. 그러므로 표면 전환 때 QEMU가 하는 일(텍스처를 지우고 다시 만들기, 그 다음
   `sdl2_gl_render_surface`의 뷰포트 설정과 그리기와 스왑) 가운데 무엇인가가 DWM에 새 프레임이 닿게 한다. 어느 것인지는
   QEMU 안에서 따로 재야 한다.
