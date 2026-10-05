# ADR-0013: 한국어 입력은 호스트 입력기가 조합하고 OME 입력기가 게스트 앱에 넣는다

상태: 채택, 2026-10-05.

## 배경

사용자 요구(2026-10-05): 한국어 입력은 사용자에게 가장 중요한 기능 가운데 하나다.

지금의 키보드 경로(ADR-0005 결과)는 웹뷰의 `keydown`/`keyup`이 `input_host_key(code, pressed)`로 물리 키 코드를 넘기고,
Rust가 QMP `input-send-event`로 `usb-kbd`에 누름과 뗌을 합성한다. 이 경로로는 한글이 들어가지 않는다.

- QMP는 키 코드만 나른다. 글자를 나르는 명령이 없다.
- 무대에는 편집 가능한 요소가 없어 윈도우 입력기가 조합을 시작하지 않는다. 한/영이 한글이어도 웹뷰는 라틴 키 코드를 받는다.
- 게스트 쪽 입력기는 AOSP `LatinIME` 하나이고 한국어 자판이 없다(2026-10-05, 안드로이드 13 게스트 `ime list -a`).
  구글 키보드는 GApps 이미지에도 없고, 블롭 없는 안드로이드 15 이미지에는 GApps가 없다.

## 결정

1. **조합은 호스트가 한다.** 게스트 앱의 글 입력 칸에 포커스가 있을 때 웹뷰는 무대 위에 투명한 `textarea`를 두고 포커스를 준다.
   윈도우 입력기(한/영 키, 두벌식, 한자 변환까지 사용자가 쓰던 그대로)가 그 칸에서 조합하고, 웹뷰는 `compositionupdate`,
   `compositionend`, `input` 이벤트를 Rust 명령으로 넘긴다. 게스트에 한국어 자판을 따로 둘 필요가 없고, 호스트 한/영 상태와 게스트 상태가
   어긋나는 일도 없다.
2. **게스트에는 OME 입력기를 둔다.** OME가 소스로 만드는 작은 `InputMethodService`(패키지 `org.openmobileemulator.ime`, 라이선스
   GPL-2.0-or-later, 소스 `guest/ime/`)다. 화면 자판을 그리지 않는다. 앱의 입력 칸에 `setComposingText`로 조합 중인 글자를 밑줄과 함께
   보이고 `commitText`로 확정한다. 조합 미리 보기는 게스트 앱 안에 그려지므로 호스트에 따로 그리지 않는다.
3. **통로는 adb 하나다.** 입력기는 추상 유닉스 소켓 `ome-ime`에서 기다리고, 호스트는 `adb forward tcp:0 localabstract:ome-ime`로 받은
   포트에 TCP로 붙는다. 제품이 이미 쓰는 adb 연결 위에 있으므로 새 네트워크 엔드포인트가 없다(R10). 소켓은 같은 기기 안의 셸 사용자와
   adb만 닿는다.
4. **글 입력 중에는 모든 키가 이 통로로 간다.** 입력기가 입력 칸 포커스를 알리면(`focus`) Rust의 문 단계가 키보드를 글 모드로 바꾸고,
   그동안 키는 QMP 대신 입력기로 간다. 확정 글자와 Enter, Backspace, 방향키가 서로 다른 통로로 가면 순서가 뒤바뀌기 때문이다.
   입력 프로필의 바인딩은 글 모드에서 쉰다(채팅 칸에 WASD를 치면 캐릭터가 움직이지 않는다). 일시 중지 단축키(F12)는 글 모드에서도
   문 단계가 먼저 처리한다. 포커스가 풀리면(`blur`) 원래 키보드 경로로 돌아간다.
5. **프레임은 줄 단위 UTF-8 JSON이다.** 한 줄에 객체 하나, 필드 이름은 camelCase다.
   - 호스트 → 입력기: `{"op":"compose","text":"한"}`(조합 중 글자 교체, 빈 문자열이면 조합 지우기),
     `{"op":"commit","text":"한글"}`(조합을 확정하고 덧붙임), `{"op":"key","key":"enter"|"backspace"|"delete"|"tab"|"escape"|"left"|"right"|"up"|"down"|"home"|"end"}`,
     `{"op":"hello","version":1}`.
   - 입력기 → 호스트: `{"op":"hello","version":1,"package":"org.openmobileemulator.ime","versionCode":N}`,
     `{"op":"focus","inputType":N,"imeAction":N,"package":"<앱 패키지>"}`, `{"op":"blur"}`.
   - 모르는 `op`은 무시한다. 판(version)이 다르면 호스트는 입력기를 다시 설치한다.
6. **설치와 선택은 제품이 한다.** 게스트가 부팅을 마치면 제품은 동봉한 입력기 APK의 `versionCode`와 게스트의 것을 비교해 다르거나 없으면
   `adb install -r`로 넣고(서명이 다르면 지우고 다시 넣는다), `ime enable`, `ime set`으로 기본 입력기로 고른다. 사용자 조작은 없다.
   능력 조사에 `textInput` 항목을 더한다.
7. **서명 키는 릴리스마다 새로 만든다.** 입력기는 저장된 데이터가 없으므로 서명이 바뀌어 다시 설치해도 잃는 것이 없다. 키를 저장소에 두지
   않는다(입력기는 사용자가 치는 모든 글자를 본다).
8. **APK는 저장소에 넣지 않는다(R1).** `guest/ime/build.ps1`이 안드로이드 SDK의 `aapt2`, `d8`, `apksigner`와 `android.jar`(API 33)로
   Gradle 없이 빌드하고, 릴리스 워크플로가 러너에서 빌드해 설치기에 동봉한다(`ci/release/tauri.release.conf.json`의 리소스, ADR-0007의
   출처 증명 대상). 개발 빌드는 개발 PC의 SDK로 같은 스크립트를 돌린다.

## 계약 (판 10)

- `AppSnapshot.text_input: TextInputView { state: unavailable | idle | active, input_type: Option<u32>, package: Option<String> }`.
  `unavailable`은 입력기가 없거나 연결이 끊긴 상태, `idle`은 연결은 되었고 입력 칸 포커스가 없는 상태, `active`는 글 모드다.
- 명령 `text_compose { text }`, `text_commit { text }`, `text_key { key }`. `key`는 5번의 열거형이다. 글 모드가 아니면 `not_active` 문제로 거절한다.
- 상태가 바뀌면 껍데기의 이벤트 펌프가 스냅숏을 곧바로 내보낸다. 웹뷰는 `active`일 때만 `textarea`에 포커스를 주고, 그동안
  `useGuestKeyboard`의 물리 키 전달을 멈춘다.

## 확인할 것 (스파이크)

- WebView2에서 투명 `textarea`가 윈도우 한글 입력기의 조합 이벤트를 받는지, 그리고 조합 중 Backspace와 Enter가 어떤 이벤트 순서로 오는지.
- 유니티 게임(트릭컬)의 이름 입력과 채팅 칸에서 입력기의 `onStartInput`이 불리는지.
- 하드웨어 키보드(`usb-kbd`)가 붙은 상태에서 화면 자판을 그리지 않는 입력기가 기본 입력기로 유지되는지.
- 안드로이드 13(Bliss 16.9.7)과 15(OME 자체 이미지) 둘 다.

## 결과

- 새 크레이트 `ome-guest-ime`(소켓 틀, 프레임, 재연결)와 `guest/ime/`(입력기 소스와 빌드 스크립트)가 생긴다.
- 문 단계에 글 모드가 생기고 `docs/ARCHITECTURE.md` 3.16절과 8절에 판 10을 적는다.
- `docs/KNOWN_LIMITATIONS.md`: 글 입력은 OME 입력기가 기본 입력기일 때만 된다. 사용자가 게스트 설정에서 다른 입력기를 고르면 한글이
  들어가지 않는다.
