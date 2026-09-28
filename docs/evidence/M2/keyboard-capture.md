# 키보드 캡처 경로 측정 (2026-09-29)

## 배경

M2 완료 실행(`ci/dod/run-dod.mjs`)은 설치기 GRUB 메뉴에서 멈췄다. 드라이버가 앱 창을 전경으로 올리고 스캔 코드로
Home과 Down을 보내도 선택이 움직이지 않았다(`docs/evidence/M2/dod-local/`, 3라운드). 워커 W5가 키 전달 경로의
고리마다 `[input]` 추적을 넣은 디버그 빌드로 같은 실행을 되풀이했는데, 앱 stderr에 `[input]` 줄이 하나도 없었다.
그래서 이 세션은 경로의 첫 고리(저수준 키보드 관찰)가 키를 받는 조건을 직접 쟀다.

## 환경

- 호스트: 개발 PC(CLAUDE.md 9절), Windows 11 22621.
- 제품: `host/target/debug/ome.exe`, 2026-09-28 07:22 빌드(커밋 1a86b28 + W5의 `[input]` 추적, 훅 설치 줄은 빌드 뒤에
  추가되어 없음). 게스트 없이 빈 `OME_HOME`으로 띄웠다.
- 키 보내기: `SendInput` 스캔 코드 쌍(누름, 120 ms, 뗌). 방향키는 확장 플래그.
- 관찰: 앱 stderr의 `[input] hook` 줄(첫 고리), 별도 pwsh 프로세스의 자체 `WH_KEYBOARD_LL` 관찰자, CDP
  (`--remote-debugging-port`)로 웹뷰 `window`에 단 `keydown`/`keyup` 리스너.
- 스크립트는 `docs/evidence/M2/keyboard-capture/`에 있다.

## 실험과 결과

| 실험 | 앱 띄운 곳 | 키 보낸 곳 | 앱이 전경인가 | 저수준 경로 | 웹뷰 DOM |
|---|---|---|---|---|---|
| probe3 | PowerShell 도구 | 같은 프로세스 | 아니오 | F24, Down 도착 | 안 잼 |
| probe4 | bash 도구 | 자식 pwsh(`native.ps1`, 전경 전환 포함) | 예 | 없음 | 안 잼 |
| A2 | bash 도구 | PowerShell 도구 프로세스 | 아니오 | 도착 | 안 잼 |
| B | bash 도구 | 자식 pwsh 셋(`native.ps1` HOME이 먼저 전경 전환) | 예 | 없음 | 안 잼 |
| C | PowerShell 도구 | 도구 프로세스와 자식 pwsh를 번갈아 | 4단계(`native.ps1`)부터 예 | 1~3단계 도착, 4단계부터 없음 | 안 잼 |
| D | PowerShell 도구 | 같은 프로세스 | `ShowWindow`+`SetForegroundWindow` 뒤 예 | 기준선(F13)만 도착 | 안 잼 |
| E | PowerShell 도구 | 같은 프로세스 | 다른 창 → 앱 → 앱 → 앱 → 다른 창 | 다른 창일 때만 도착 | 안 잼 |
| F | PowerShell 도구 | 같은 프로세스, 외부 관찰자 병행 | 다른 창 → 앱 → 앱 → 다른 창 | 앱과 관찰자 모두 다른 창일 때만 도착 | 안 잼 |
| G2 | PowerShell 도구 | 자식 pwsh(제목 표시줄 클릭으로 전경) | 예 | F12 뗌만(개발자 도구가 전경을 가져간 뒤) | F13, ArrowDown, Home, Enter, F12 누름 도착 |

실험 F의 시각표(UTC). 앱과 외부 관찰자가 같은 순간에 같은 키를 보고, 같은 순간에 같은 키를 놓친다.

```
19:11:08.959 A other foreground F13 0x64      앱 hook: 도착   관찰자: 19:11:08.988 scan=0x64 도착
19:11:10.672 B app foreground   F14 0x65      앱 hook: 없음   관찰자: 없음
19:11:11.804 B2 app foreground  DOWN 0x50 ext  앱 hook: 없음   관찰자: 없음
19:11:13.437 C other foreground F15 0x66      앱 hook: 도착   관찰자: 19:11:13.452 scan=0x66 도착
```

실험 G2의 웹뷰 리스너 기록.

```
down code=F13 key=F13 repeat=false target=BODY
up code=F13 key=F13 repeat=false target=BODY
down code=ArrowDown key=ArrowDown repeat=false target=BODY
up code=ArrowDown key=ArrowDown repeat=false target=BODY
down code=Home key=Home repeat=false target=BODY
up code=Home key=Home repeat=false target=BODY
down code=Enter key=Enter repeat=false target=BODY
up code=Enter key=Enter repeat=false target=BODY
down code=F12 key=F12 repeat=false target=BODY
```

두 도구의 프로세스 문맥은 같았다(세션 1, `WinSta0\Default`, 잡 오브젝트 UI 제한 없음, 중간 무결성).

## 판정

1. 앱 창이 전경이면 제품의 저수준 경로도, 별도 프로세스의 저수준 관찰자도 키를 받지 못한다. 다른 창이 전경이면
   둘 다 받는다. 전경이 바뀌는 즉시 되돌아오므로 영구 손상이 아니다.
2. 같은 순간 웹뷰 DOM은 그 키를 받는다.
3. F12는 디버그 빌드에서 WebView2 개발자 도구를 열고 전경을 가져간다. F12는 제품의 기본 일시 중지 단축키다.
4. 원인은 이 호스트의 입력 처리에 있고 제품 코드 안에 없다. 더 추적하지 않는다(사용자 지시 2026-09-29).
5. 완료 실행이 GRUB에서 멈춘 이유가 이것이다. 드라이버는 키를 보내기 전에 앱을 전경으로 올리므로 제품의 저수준
   경로는 그 키를 한 번도 보지 못했고, W5 실행의 `[input]` 줄 0개가 그 증거다.

## 결정

- 캡처 단계를 메인 웹뷰의 키 이벤트로 바꾼다. 웹뷰가 `input_host_key(code, pressed)`로 넘기고, 문과 합성과 감독자는
  그대로다. 전역 저수준 훅과 입력 펌프는 제품에서 뺀다(ADR-0005 개정, `docs/ARCHITECTURE.md` 3절과 8절).
- 두 웹뷰 창의 개발자 도구를 끈다. 디버그 빌드에서도 `OME_DEVTOOLS`를 줘야 켜지므로 F12는 더는 개발자 도구를
  열지 않는다. 브라우저 가속 키 전체(F5, Ctrl+F, Ctrl+P)를 끄는 스위치는 tauri 2.11.6이 wry의
  `with_browser_accelerator_keys`를 노출하지 않아 넣지 못했다. 무대가 활성일 때는 웹뷰가 모든 키의 기본 동작을
  막으므로 그때는 닿지 않고, 다른 화면에서는 남는다.
- 담긴 게스트 창은 키보드 포커스를 갖지 않는다. 포커스는 웹뷰에 남고, 마우스는 포커스 없이도 게스트 창에 닿는다.
  실제 게스트로 잰 결과는 아래 절에 있다.

## 실제 게스트 검증 (워커 X1)

2026-09-29에 `default` 게스트(Bliss OS 16.9.7, 제품 런타임의 기본 QEMU 설정)를 대상으로
`browser_keyboard_and_mouse_reach_unfocused_real_guest` 무시 테스트를 실행했다. 부팅 뒤 테스트 창을 잠시 최상위 창으로
두어 다른 앱이 검증 좌표를 가리지 않게 했으며, 테스트를 마치고 제품 런타임으로 게스트를 종료했다.

| 항목 | 결과 | 측정 |
|---|---|---|
| (a) `to_front` 뒤 포커스 | 통과. 부모 GUI 스레드의 포커스 창은 SDL 자식이 아니었다. | `focus_after_to_front_is_guest=false` |
| (b) 포커스 없는 SDL 자식 클릭 | 포인터 전달은 통과했다. 화면 좌표 `(748, 491)`에서 `SDL_app`을 확인한 뒤 실제 `SetCursorPos`와 `SendInput` 누름/뗌을 보냈다. 게스트는 371 ms 안에 절대 좌표와 버튼 누름을 받았다. 다만 제품 기본 장치는 `ABS_X`와 `BTN_MOUSE`로 보고해, 요구한 `BTN_TOUCH` 또는 `ABS_MT_POSITION_X`는 확인하지 못했다. | 아래 `getevent` 3줄 |
| (c) 클릭 뒤 포커스 | 통과. 별도 보정 없이 SDL 자식은 포커스를 얻지 않았다. | `focus_immediately_after_click_is_guest=false`, `focus_after_click_is_guest=false` |
| (d) 웹뷰 코드 입력 | 통과. `ingest_browser_key("KeyA", true, true)`와 뗌 호출 뒤 26 ms 안에 게스트가 누름과 뗌을 받았다. | 아래 `KEY_A` 2줄 |

클릭의 `getevent -lq` 출력은 다음과 같다. 현재 제품의 기본 장치는 QEMU USB Tablet이어서 SDL 호스트 클릭은
`BTN_TOUCH`가 아니라 `BTN_MOUSE`로 보고된다. 좌표와 버튼 누름을 함께 확인했다.

```text
+55ms /dev/input/event2: EV_ABS       ABS_X                00003e65
+169ms /dev/input/event2: EV_ABS       ABS_X                00003fff
+279ms /dev/input/event2: EV_KEY       BTN_MOUSE            DOWN
```

웹뷰 키 입력의 `getevent -lq` 출력은 다음과 같다.

```text
+10ms /dev/input/event3: EV_KEY       KEY_A                DOWN
+26ms /dev/input/event3: EV_KEY       KEY_A                UP
```

테스트 결과는 `1 passed; 0 failed`, 총 29.06초였다. 종료 뒤 `tasklist`에서 `qemu-system`과 `ome.exe`가
모두 없음을 확인했다.
