<!-- SPDX-License-Identifier: GPL-2.0-or-later -->
<!-- Copyright (C) 2026 Open Mobile Emulator contributors -->

# M2 오버레이 창 실제 검사

측정일은 2026-09-28 02:18이다. 디버그 빌드의 제품 앱(`ome.exe --real-overlay-window-check`,
`host/app/src/overlay_check.rs`, 릴리스 빌드에는 없다)을 무시 테스트
`measures_real_overlay_window_through_product_app`(`host/crates/ome-runtime/tests/real_guest_spike.rs`)이
띄워 개발 호스트의 `default` 게스트로 잰 값이다. 검사 앱은 게스트를 시작해 `running`이 되면 주 창의
클라이언트 영역을 무대 사각형으로 보내 게스트 창을 담고, 편집 모드에 들어갔다가 나오고, 표지를 껐다
켠 뒤 런타임으로 게스트를 끈다. 원자료는 `overlay-window/check-output.txt`다.

실행 방법은 다음과 같다.

```
cd "C:/Open Mobile Emulator/host"
cargo build -p ome-app --features custom-protocol
cargo test -p ome-runtime --test real_guest_spike measures_real_overlay_window_through_product_app -- --ignored --nocapture
```

## 환경

| 항목 | 값 |
|---|---|
| 게스트 | `default`(Bliss 16.9.7, Android 13), `%LOCALAPPDATA%\OpenMobileEmulator\vm\default\disk.qcow2` |
| QEMU | 자체 빌드 11.1.1, WHPX |
| 앱 | `host/target/debug/ome.exe`(임베디드 UI, 5edcf82 이후 작업 트리) |
| 검사 전 QEMU 프로세스 | 0 |
| 검사 뒤 QEMU 프로세스 | 0 |

## 측정

| 항목 | 값 |
|---|---|
| 게스트 자식 창의 클라이언트 사각형(화면 물리 픽셀) | x 160, y 206, 2560×1600 |
| 편집 모드 진입에서 오버레이 창이 보이기까지 | 625 ms(창을 처음 만드는 시간 포함) |
| 편집 모드의 오버레이 사각형 | x 160, y 206, 2560×1600(게스트와 같음) |
| 편집 모드의 클릭 통과 | 끔 요청 성공 |
| 표시 모드로 돌아온 뒤 다시 적용 | 44 ms, 같은 사각형, 보임 |
| 표시 모드의 클릭 통과 | 켬 요청 성공 |
| `InputOverlayToggle`로 숨김 | 1 ms, 보임 false |
| 다시 켠 뒤 `GuestStop`에서 `stopped`까지 | 30,700 ms |
| 정지 뒤 오버레이 | 보임 false |

무대 사각형은 주 창의 클라이언트 영역 전체였다. 물리 픽셀 값은 이 호스트의 주 창 배율에서 나온
것이고, 오버레이는 자기 창의 좌표를 읽지 않고 게스트 자식 창의 사각형을 그대로 쓴다.

## 판정

- 오버레이 창은 게스트 자식 창과 같은 화면 사각형에 놓인다.
- 편집 모드에서만 보이고 클릭을 받으며, 표시 모드에서는 클릭을 통과시킨다. 표지를 끄면 숨고
  게스트가 끝나면 숨는다.
- 정지가 30.7 s 걸린 까닭은 이 검사가 부팅 완료를 기다리지 않고 `running` 직후에 끄기 때문이다.
  adb 전원 끄기 이음새는 부팅이 끝난 게스트에만 답하므로 ACPI `system_powerdown`으로 넘어갔고,
  부팅 중인 Android는 이를 무시해 감독자의 30 s 시한 뒤 강제 종료로 끝났다. 부팅이 끝난 게스트의
  정지는 `window-hosting.md`대로 2.1~2.3 s다. 부팅 중인 게스트를 끌 때 시한을 줄일지는 열린
  항목이다.

## 재지 못한 것

- 클릭 통과의 원시 창 스타일(`WS_EX_TRANSPARENT`)은 조회하지 않았다. `set_ignore_cursor_events`
  호출이 성공한 사실만 기록했다.
- 소유 창의 Z 순서는 육안으로 확인하지 않았다.
- 오버레이 웹뷰가 실제로 표지를 그리는지는 이 검사가 보지 않는다(페이지 쪽은 갤러리 스크린샷
  `docs/evidence/M2/screens/overlay-*.png`).

## 첫 시도에서 찾은 결함

첫 검사 시도는 앱이 시작 직후 주 스레드로 한 코어를 계속 태우며 응답하지 않았고 검사 스레드는 첫
스냅숏에서 멈췄다. 껍데기와 런타임에 표식을 넣어 추적하니 `wizard_facts`가 스냅숏마다
`ArtifactStore::verify`를 불러 선택한 이미지의 ISO 전체(2.3 GB)를 SHA-256으로 다시 읽고 있었다.
런타임은 명령과 이벤트마다 스냅숏을 만들고 그동안 잠금을 쥐므로 앱 전체가 해시 뒤에 줄을 섰다.
한 번 맞춘 검증을 파일 옆 표지로 기억하도록 고친 뒤(`ome-artifacts`, 같은 날 커밋) 이 측정을 얻었다.
이 검사의 첫 스냅숏이 표지를 썼고(`...iso.verified`), 그 뒤의 스냅숏은 해시하지 않았다.
