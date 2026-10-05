# OME 입력기 스파이크: 안드로이드 13 게스트 (2026-10-05)

ADR-0013의 게스트 쪽(입력기 `org.openmobileemulator.ime`, 소켓 `ome-ime`, 줄 단위 JSON)을 실제 게스트에서 시험한 기록이다.

## 환경

- 게스트: Bliss 16.9.7(API 33), 개발 PC의 `default` 게스트, 화면 2560x1440.
- 빌드: `guest/ime/build.ps1`, SDK `C:\Program Files (x86)\Android\android-sdk`, build-tools 36.0.0, `android-36` 플랫폼(35가 없어 대체),
  minSdk 33, targetSdk 35, APK 16,849바이트, v3 서명(실행마다 만든 임시 RSA 3072 키),
  SHA-256 `ee2393681fce1ac2fbac36505bd561e8b4b5da42658e38ae95202912e3185262`.
- 설치: `adb install -r`, `ime enable`, `ime set` 성공.

## 받은 줄 (설정 앱 검색 칸에 포커스)

```json
{"op":"hello","version":1,"package":"org.openmobileemulator.ime","versionCode":1}
{"op":"blur"}
{"op":"blur"}
{"op":"blur"}
{"op":"focus","inputType":589825,"imeAction":3,"package":"com.android.settings.intelligence"}
```

## 명령과 결과

`compose "ㅎ"` → `compose "하"` → `compose "한"` → `commit "한"` → `commit "글 입력"` → `key backspace` → `key enter`

- 조합 중 글자는 밑줄과 함께 바뀌었다(`02-compose-*.png`).
- 확정 뒤 칸의 글자는 `한글 입력`(`03-committed.png`), Backspace 뒤 `한글 입`(`04-backspace.png`).
- Enter는 imeAction SEARCH(3)라 `performEditorAction(3)`을 불렀고 줄바꿈은 없었다(`05-enter.png`). 검색 앱이 원래 자동 검색이라 액션 호출 자체를 화면으로 입증하지는 못했다.

## 재연결과 포커스 해제

- 이전 연결을 둔 채 새로 붙으면 이전 연결은 EOF를 받고 새 연결은 `hello`와 현재 `focus`를 받았다. 첫 구현은 `LocalSocket.close()`만 불러 이전 연결의 읽기가 남았고, `shutdownInput()`/`shutdownOutput()`을 더해 고쳤다.
- 홈으로 가면 `{"op":"blur"}`.
- LatinIME로 바꾸면 소켓 EOF, 다시 OME 입력기로 바꾸면 `hello`/`focus`(서비스 종료와 재생성).
- 잘못된 JSON과 모르는 `op`은 무시, 65,537자 프레임은 연결을 끊고 다음 연결을 받았다.

## 결함: 글자 명령과 키 명령을 쉬지 않고 섞으면 순서가 어긋난다

빈 검색 칸에 쉬지 않고 `compose "지울 글"`, `compose ""`, `commit "AB"`, `key left`, `key delete`, `compose "한"`, `key left`, `key right`, 잘못된 JSON, 모르는 op, `commit "글"`을 보내면
기대값 `A한글` 대신 `AB한`이 되었다. 명령마다 화면이 안정되기를 기다리면 `A한글`이다. `commitText`/`setComposingText`는 입력 연결로 바로 편집하고
`sendDownUpKeyEvents`는 앱의 입력 이벤트 대기열을 거치므로 둘의 순서가 보장되지 않는다고 본다(내부 추적은 하지 않았다).

### 수정 (같은 날)

편집 키를 입력 연결의 편집 연산으로 옮겼다. Backspace와 Delete는 선택 영역 삭제나 `deleteSurroundingTextInCodePoints`(조합 중이면 조합 글자에서
한 코드 포인트를 빼 `setComposingText`), 방향키와 Home/End는 조합 확정 뒤 `getExtractedText`와 `setSelection`(서로게이트 쌍을 가르지 않음),
여러 줄 칸의 Enter는 `commitText("\n")`, 명령마다 `beginBatchEdit`/`endBatchEdit`로 묶는다. 편집 상태를 못 읽는 칸과 Up/Down/Tab/Escape만 키 이벤트로 남으며
이들은 빠른 글자 명령과 섞일 때 순서를 보장하지 않는다(`guest/ime/README.md`).

APK SHA-256 `80b6dfb04c89d1c0087858fe24f889070d83daf60e359302b0809ae31b7f8f91`. 위 재현열을 한 번의 쓰기로 다섯 번 보내 다섯 번 모두
`A한글`(UIAutomator `android:id/search_src_text`, `07-order-1..5.png`). 기본 시험 재실행도 `ㅎ`, `하`, `한`, `한글 입력`, `한글 입`, `한글 입`으로 통과(`06-*.png`).
`A + U+10400 + B`에 Left 두 번, Delete, Home, `commit X`, End, Backspace를 쉬지 않고 보내 `XA`, `compose 한 + U+10400` 뒤 Backspace, Right, `commit 글`은 `한글`(`08-composition-backspace.png`).

## 시험하지 않은 것

안드로이드 15, 유니티 입력 칸(트릭컬), 여러 줄 칸의 Enter, 일반 앱 UID의 연결 거절.

시험 뒤 기본 입력기를 `com.android.inputmethod.latin/.LatinIME`로 되돌리고 게스트를 정상 종료했다.
