# 게스트 포인터와 호스트 커서의 어긋남 (2026-10-05)

사용자가 0.1.3 설치본에서 보고했다. 안드로이드가 그리는 검은 포인터가 실제 마우스 커서와 다른 자리에 있고, 터치는 검은
포인터 자리에서 일어난다. 스크린숏에서 커서는 화면의 (628, 416) 근처에, 검은 포인터는 (430, 300) 근처에 있었다.

## 1. 환경

- 제품 0.1.3, `%LOCALAPPDATA%\Open Mobile Emulator\qemu`의 QEMU는 릴리스 `qemu-v11.1.1-ome5`(패치 0001~0006).
- 게스트 `bliss-16.9.7-android-13`, 직접 커널 부팅. 표시 설정 1920x1080, 240 dpi, 주사율 설정 없음.
- QEMU 인자(발췌): `-device virtio-vga-gl,edid=off -display sdl,show-cursor=on,gl=on,owner-window=<HWND>`,
  `-usb -device usb-tablet`.

## 2. 측정

`pointer-alignment/probe.ps1`이 SDL 창 안의 정해진 비율 자리로 `SetCursorPos`를 하고(PMv2 스레드, 물리 픽셀), 그때마다
`adb shell getevent -pl /dev/input/event2`로 USB 태블릿의 현재 `ABS_X`, `ABS_Y`를 읽는다. 태블릿의 범위는 0~32767이다.
창은 2384x1320 물리 픽셀이었다.

| 창 안 비율 | 창 좌표 | ABS_X | ABS_Y | 기대값(비례) |
|---|---|---|---|---|
| 0.02, 0.02 | 48, 26 | 0 | 368 | 655, 655 |
| 0.1, 0.1 | 238, 132 | 0 | 1966 | 3277, 3277 |
| 0.5, 0.5 | 1192, 660 | 7807 | 9912 | 16384, 16384 |
| 0.9, 0.9 | 2146, 1188 | 15743 | 17858 | 29490, 29490 |
| 0.98, 0.98 | 2336, 1294 | 17330 | 19455 | 32112, 32112 |
| 0.25, 0.75 | 596, 990 | 2841 | 14868 | 8192, 24575 |
| 0.75, 0.25 | 1788, 330 | 12774 | 4956 | 24575, 8192 |

게스트가 받은 값은 커서 자리의 약 0.6배다. 모든 점이 다음 식과 맞는다(표면 1280x800, 창 2384x1320).

- 상류 `handle_mousemotion`이 먼저 `x1 = mx * 1280 / 2384`, `y1 = my * 800 / 1320`으로 표면 좌표를 만든다.
- 패치 0006 첫 판의 `sdl2_owner_window_map_point`가 이 값을 창 좌표로 여기고, 창 안에 1280x800을 띠를 두고 넣은 자리
  (`vw = 1320 * 1.6 = 2112`, `vx = 136`)로 다시 변환한다. 비율은 `(x1 - 136) / 2112`, `y1 / 1320`이다.
- 가운데 점: `(640 - 136) / 2112 = 0.2386`, 측정 `7807 / 32767 = 0.2383`. 세로 `400 / 1320 = 0.303`, 측정 `0.3025`.

그리고 virgl 스캔아웃은 `sdl2_gl_scanout_flush`가 창 전체에 늘려 그리므로(`egl_fb_setup_default(ww, wh)` 뒤
`egl_fb_blit`), 띠를 빼는 변환은 처음부터 이 모드와 맞지 않았다. 띠는 스캔아웃 밖의 GL 표면
(`surface_gl_setup_viewport`)에만 있다.

## 3. 게스트 쪽의 두 번째 원인

`adb shell wm size`는 `Physical size: 1280x800`, `Override size: 1920x1080`이었다. 물리 화면은 virtio-gpu의 기본값
1280x800에 머물렀다. 패치 0006이 창 크기를 게스트에 알리지 않게 했고, 주사율 설정이 없으면 제품이 `xres`, `yres`를 넘기지
않았기 때문이다. `dumpsys input`의 뷰포트는 다음과 같았다.

```
Viewport INTERNAL: displayId=0, uniqueId=local:0, port=0, orientation=0, logicalFrame=[0, 0, 1920, 1080], physicalFrame=[0, 40, 1280, 760], deviceSize=[1280, 800], isActive=[1]
```

태블릿(`Classes: CURSOR | EXTERNAL`, `Mode: POINTER`)의 범위는 논리 화면 0~1919, 0~1079에 대응하고, 논리 화면은 물리 화면의
y 40~760에 그려진다. 그래서 QEMU의 변환이 맞아도 세로로 최대 화면 높이의 5 %가 어긋나고, 16:10 물리 화면을 16:9 창에 늘려
그리므로 그림도 가로로 늘어난다.

## 4. 수정

- QEMU 패치 0006을 다시 썼다. `handle_mousemotion`과 `handle_mousebutton`이 원래 SDL 창 좌표를 `sdl2_window_to_surface`
  하나로 변환한다. 스캔아웃과 2D 렌더러는 비례로, 소유자 창이 있고 스캔아웃 밖의 GL 표면일 때만 띠를 빼고 변환한다.
  `sdl_send_mouse_event`는 더 변환하지 않는다. 릴리스는 `qemu-v11.1.1-ome6`.
- `ome-guest-config`는 주사율 설정이 없어도 표시 설정의 해상도가 있으면 `virtio-vga-gl,edid=off,xres=W,yres=H`로
  기동한다. `xres`, `yres`는 EDID가 아니라 virtio-gpu의 표시 정보이므로(`virtio-gpu-base.c`의
  `req_state[0].width = conf.xres`, EDID와 무관) EDID를 끈 채로도 게스트의 선호 모드가 된다.

## 5. 수정 뒤 측정

러너가 빌드한 `qemu-v11.1.1-ome6`(실행 37294107725, 커밋 a2ab1f2, SHA-256과 출처 증명 확인)을 설치본의 `qemu`에, 같은 커밋에서
로컬로 빌드한 `ome.exe`를 설치본에 바꿔 넣고(이전 파일은 `qemu.ome5.bak`, `ome.exe.v0.1.3.bak`) 게스트를 다시 시작했다.
그 사이 사용자가 표시 설정을 2560x1440으로 바꿔 두었다.

- QEMU 인자: `-device virtio-vga-gl,edid=off,xres=2560,yres=1440`.
- `wm size`: `Physical size: 2560x1440`, 덮어쓰기 없음.
- `dumpsys input`: `logicalFrame=[0, 0, 2560, 1440], physicalFrame=[0, 0, 2560, 1440], deviceSize=[2560, 1440]`.
  물리 화면과 논리 화면이 같아 띠가 없다.
- 사용자가 같은 설치본을 직접 써 보고 포인터가 커서 자리에 온다고 확인했다(2026-10-05, "문제 해결되었습니다").

`probe.ps1`을 다시 돌린 결과는 일곱 점 모두 `32741,4141`로 움직이지 않았다. 그때 무대 가운데의 창을
`WindowFromPoint`로 보니 Claude 데스크톱 앱(`Claude.exe`)의 `Chrome_RenderWidgetHostHWND`가 무대 위에 있어,
`SetCursorPos`의 마우스 이동이 SDL 창에 가지 않았다. 사용자가 원격으로 이 PC를 보던 창이다. 그래서 수정 뒤의 좌표표는
사용자의 확인으로 대신한다.
