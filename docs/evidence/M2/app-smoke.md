<!-- SPDX-License-Identifier: GPL-2.0-or-later -->
<!-- Copyright (C) 2026 Open Mobile Emulator contributors -->

# M2 제품 앱 스모크: 실제 UI를 CDP로 몰아 본 결과

측정일은 2026-09-28 02:41 KST다. 7451359의 작업 트리에서 `cargo build -p ome-app --features
custom-protocol`로 만든 디버그 앱(임베디드 UI)을 개발 호스트의 `default` 게스트(Bliss 16.9.7, Android
13)로 돌렸다. 앱은 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` 환경에서
띄우고, Playwright가 `chromium.connectOverCDP`로 주 페이지(`http://tauri.localhost/`)와 오버레이
페이지(`overlay.html`)에 붙어 모든 클릭을 보냈다. 창 크기 변경과 합성 캡처, 트레이의 `종료`만 Win32
보조 코드가 맡았다(트레이 메뉴를 실제로 열어 항목을 골랐다). 게스트 안은 아무것도 바꾸지 않았고
앱 상태 파일(`state.json`)은 실행 전후 SHA-256이 같다. 네 번 실행했고(전경 전환 실패로 사진을
다시 찍음) 최종 실행의 값을 적는다. 원자료는 `app-smoke/driver-output.txt`(UTC), 앞선 실행은
`attempt-1..3-*`, 드라이버 소스는 `driver-source.txt`와 `native-source.txt`다.

## 단계 표

| 단계 | 측정 | 판정 |
|---|---|---|
| 앱 실행 → 첫 화면 | 630 ms | 그려짐. 호스트 점검 내용이 비어 있음(결함 2) |
| `나중에 하기` → 셸 | 1.7 s(사진 포함) | 통과 |
| `시작` → `부팅 중` 표시 | 70 ms | 통과 |
| → `실행 중` 표시 | 1.0 s | 통과(창 담김, 아직 `bootCompleted=false`) |
| → 부팅 완료와 능력 조사 | 27.1 s | 통과 |
| 창 1600×1000으로 바꾸고 되돌림 | 자식 창이 따라옴 | 통과 |
| 레일 `앱` | 목록 7개(DOM) | 화면은 게스트 창에 가려짐(결함 1) |
| 레일 `입력` → 무대의 `매핑 편집` → `편집 끝` | 오버레이 창과 도구 띠 표시 | 동봉 프리셋이라 편집은 막힘(설계대로) |
| 레일 `설정` → Google 계정 절 | 계정 1개(DOM) | 화면은 게스트 창에 가려짐(결함 1) |
| `끄기` → `시작 가능` | 1.9 s | 통과(adb 전원 끄기) |
| 트레이 `종료` → 프로세스 종료 | 1.1 s, exit 0 | 통과. QEMU와 `ome.exe` 없음 |

창 사각형(물리 픽셀, 배율 2): 주 창 (147,147)–(2733,1818), 게스트 자식 창 (312,318)–(2696,1726) =
2384×1408, 무대 CSS 1192×704. 1600×1000으로 바꾼 뒤 자식 창 (246,230)–(1644,968) = 1398×738, 무대
CSS 699×369. 되돌린 뒤 원래 값으로 돌아왔다.

사진: `01-first-screen`(DOM), `02-shell`(3차 실행의 것. 최종 실행 사진에는 다른 앱이 겹쳤다),
`03-stage-running`, `04-stage-resized`, `05-apps`, `06-input`, `06-overlay-editing`, `07-overlay-showing`,
`08-settings-top`, `09-settings-google`, `10-stage-stopped`. `*-dom.png`는 같은 시점의 웹뷰 DOM 사진이다.

## 찾은 결함

1. **다른 화면으로 가도 게스트 창과 오버레이가 본문을 가린다.** `앱`, `입력`, `설정`을 골라도 SDL
   자식 창은 보이는 채로 무대의 마지막 사각형에 남고 오버레이 표지도 그 위에 남는다(`05-apps.png`,
   `08-settings-top.png`). `Shell.tsx`는 화면 선택을 로컬 상태로만 두고, `StageFrame`은 사라질 때
   관찰자만 해제하며 Rust에 알리지 않는다. 무대가 사라지면 자식 창과 오버레이를 숨겨야 한다.
2. **첫 실행의 호스트 점검 화면이 비어 있다.** 스냅숏의 `host`가 `rows: [], ready: false,
   inspectedAt: null`이라 판정도 항목도 없다(`01-first-screen.png`). 조사는 `다시 확인`을 눌러야
   돈다. 앱이 시작할 때 한 번 조사해야 한다.
3. **자동 업데이트 확인이 실패로 끝난다.** `update.state.kind = failed`, `update_check_failed`,
   문구는 네트워크 연결을 확인하라고 한다. 저장소가 비공개이고 릴리스가 없어 GitHub API가 404를
   돌려주는 상황인데, 이것은 "업데이트 없음"으로 보여야 한다.
4. 앱 목록의 버전과 설치일이 비어 있다(라벨은 패키지 이름). 세대 어댑터가 아직 그 값을 읽지 않는다.
5. 동봉 프리셋에는 `무대에서 편집`이 없고 무대의 `매핑 편집`은 `동봉 프리셋은 복제한 뒤 편집할 수
   있습니다.`를 보인다. 설계대로이며 결함이 아니다.
6. 도구 막대의 프리셋은 `1280×720`, 상태 줄은 `1192×704 · 창에 맞춤`이다. 창에 맞춤 모드가 무대
   크기로 운영체제 해상도를 맞춘 결과라 설계대로다.

앱 stderr에는 종료 때의 `Failed to unregister class Chrome_WidgetWin_0. Error = 1412` 한 줄만 있었고
앱은 exit 0으로 끝났다.

## 다시 돌리는 방법

드라이버는 `app-smoke/driver-source.txt`(실행 당시 코드)와 `rerun-driver-source.txt`(출력 폴더를
인자로 받는 판)다. 앱과 QEMU가 이미 떠 있으면 중단하고, 매 실행 뒤 앱 자체의 `끄기`와 트레이
`종료`를 쓰며 상태 파일을 되돌린다. 스크립트의 통과 표시는 예외가 없었다는 뜻일 뿐이므로 합성
사진을 직접 봐야 한다.
