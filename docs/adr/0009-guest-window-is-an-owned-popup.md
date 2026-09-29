# ADR-0009: 게스트 창은 메인 창이 소유한 최상위 팝업이다

상태: 채택, 2026-09-29. ADR-0005의 창 담기 2안을 택한다.

## 배경

M2 첫 스파이크는 QEMU의 SDL 창을 메인 창의 자식으로 재부모화하는 1안을 통과시켰다(2026-09-27,
`docs/evidence/M2/window-hosting.md`). 그런데 2026-09-29의 완료 실행에서 설치기 GRUB이 검은 화면으로 찍혔고, 재어 보니
1안은 200 % 모니터에서 서로 다른 두 가지 이유로 깨진다(`docs/evidence/M2/embedded-display-freeze.md`).

- DPI 인식이 없는 SDL 창(96)을 PMv2 메인 창(192) 아래에 담으면 시스템의 DPI 가상화 표면으로 보이는데, 그 표면이
  무작위로 멈춰 재부모화 전의 마지막 프레임이 virtio-gpu 스캔아웃까지 남는다(열한 번 중 아홉 번). 어떤 외부 조치로도
  풀리지 않는다.
- SDL 창을 PMv2로 만들면 멈춤은 사라지지만, WebView2의 DirectComposition 시각이 최상위 창의 일반 그리기 위에 합성되어
  형제인 SDL 자식을 덮는다. 그리고 QEMU가 게스트 해상도에 맞춰 `SDL_SetWindowSize`를 부를 때 SDL 2.32는 내부 크기 기록을
  먼저 바꾸고 `SetWindowPos(hwnd, HWND_NOTOPMOST, …)`의 실패를 무시하므로, 자식 창에서는 기록과 실제 크기가 어긋나
  게스트가 원본 크기로 왼쪽 아래에 그려진다.

메인 창이 소유한 최상위 팝업(WS_POPUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW)은 세 문제를 모두 피한다. 재부모화가 없으니
가상화도 없고, 다른 최상위 창이므로 WebView2가 덮지 않으며, QEMU의 크기 변경은 실제 창 사각형에 드러나 호스트가 다시
배치할 수 있다. 소유 관계 덕에 팝업은 메인 창 위에 머물고 메인 창과 함께 최소화·숨김되며, WS_EX_NOACTIVATE 덕에
클릭해도 전경을 가져가지 않아 웹뷰가 키보드를 계속 받는다(ADR-0005 개정판의 캡처 단계).

## 결정

1. `ome-window-host`는 발견한 SDL 창을 메인 창이 소유한 팝업으로 바꾸고 무대의 물리 화면 좌표에 놓는다. 스타일은
   WS_POPUP, 확장 스타일은 WS_EX_NOACTIVATE와 WS_EX_TOOLWINDOW를 더하고 WS_EX_APPWINDOW를 뺀다. 포커스는 어떤 경우에도
   팝업에 주지 않는다.
2. QEMU가 스스로 창 크기를 바꾸면(게스트 해상도 전환) 런타임의 1초 틱이 사각형 차이를 보고 다시 배치한다. 메인 창의
   이동·크기 변경·배율 변경 때도 다시 배치한다.
3. z-순서는 오버레이 창, 게스트 팝업, 메인 창 순이다. 오버레이가 보일 때 팝업은 오버레이 바로 뒤에 놓는다.
4. QEMU는 `SDL_WINDOWS_DPI_AWARENESS=permonitorv2` 환경 변수로 띄워 팝업이 물리 픽셀로 선명하게 그려지게 한다.
5. 스냅숏의 `hosting`은 이 방식도 `embedded`로 보고한다. 사용자에게는 무대 안의 게스트다. `separateWindow`는 이것마저
   실패했을 때의 대비책으로 남는다.

## 결과

- `docs/ARCHITECTURE.md` 6절과 창 담기 서술은 이 결정을 따른다. 1안의 측정은 역사로 남긴다.
- 게스트 해상도가 바뀌는 순간 팝업이 최대 1초 동안 게스트 해상도 크기로 보일 수 있다. KNOWN_LIMITATIONS에 적는다.
- 재부모화 코드(`make_child_of_at`, `focus_child`)는 스파이크 테스트용으로만 남고 제품 경로는 부르지 않는다.
