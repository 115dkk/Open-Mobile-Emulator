<!-- SPDX-License-Identifier: GPL-2.0-or-later -->
<!-- Copyright (C) 2026 Open Mobile Emulator contributors -->

# M2 실제 게스트 창 담기와 정상 종료 측정

측정일은 2026-09-27이다. `ome-runtime`의 실제 의존성으로 개발 호스트의 `default` 게스트를 두 번 시작했다. 두 실행 모두 부팅과 첫 능력 조사를 마쳤고, `GuestStop`은 adb 전원 끄기를 거쳐 `stopped`와 `UserStop`으로 끝났다. 테스트 전, 각 실행의 정지 뒤, 테스트 종료 뒤에 QEMU 프로세스가 없음을 확인했다.

원자료는 `window-hosting/measurements.txt`, `window-hosting/test-output.txt`, `run-*-qemu-command.txt`, `run-*-qemu-stderr.txt`, `run-*-adb.png`에 있다.

## 환경

| 항목 | 측정값 |
|---|---|
| QEMU | `QEMU emulator version 11.1.1 (v11.1.1-dirty)` |
| QEMU 출처 | `custom-build` |
| QEMU 실행 파일 | `C:\Users\USER\AppData\Local\OpenMobileEmulator\qemu-build\out\bin\qemu-system-x86_64.exe` |
| 게스트 ID | `default` |
| 이미지 | `bliss-16.9.7-android-13`, Android 13, API 33 |
| 디스크 | `%LOCALAPPDATA%\OpenMobileEmulator\vm\default\disk.qcow2` |
| adb | Android Debug Bridge 1.0.41, `127.0.0.1:5555` |
| 테스트 부모 창 | 클라이언트 영역 1280×800 |
| 호스트 DPI | 96 |
| 배율 | 1.0000 |
| 실행 전 QEMU 수 | 0 |

호스트 부모 영역을 캡처하는 API는 `ome-platform-win`에 없어서 호스트 캡처는 건너뛰었다. `unsafe` 코드를 새로 넣지 않았다.

## 첫 실행

### 상태와 시간

| 시점 | 상태 |
|---|---|
| 0 ms | `stopped` |
| 30 ms | `starting` |
| 1,171 ms | `running` |
| 정지 요청 뒤 176 ms | `stopping` |
| 정지 요청 뒤 2,114 ms | `stopped` |

시작부터 `bootCompleted=true`까지 33,428 ms가 걸렸다. `running`부터 부팅 완료까지는 32,256 ms였다.

### 능력 조사

조사 시각은 `2026-09-27T23:32:07+09:00`이다. `adbConnected=true`였고 기기 ID는 hex `39335c0a…(가려짐)`, decimal `4121739…(가려짐)`이었다. Google 계정은 1개, 미디어 볼륨은 15, 앱 루트 권한은 꺼짐이었다. 해상도는 960×600이었다.

| 항목 | 상태 | 읽은 값 |
|---|---|---|
| `bootMarker` | `available` | `bootCompleted=true` |
| `appList` | `available` | 제3자 패키지 7개 |
| `displaySize` | `available` | 960×600, 130 dpi |
| `mediaVolume` | `available` | 15 |
| `deviceId` | `available` | `39335c0a…(가려짐)` |
| `screenshot` | `available` | 임시 게스트 screencap 명령 성공 |
| `foregroundApp` | `unknown` | 값 없음 |
| `multitouch` | `unavailable` | 입력 장치 조사에서 없음 |
| `nativeBridge` | `available` | `libndk_translation.so` |
| `root` | `unavailable` | `false` |

런타임의 `ScreenshotSave`로 `%LOCALAPPDATA%\OpenMobileEmulator\screenshots\20260927-233207.png`를 만들고 `run-1-adb.png`로 복사했다.

### 창 담기

QEMU 창 클래스는 `SDL_app`이었다. `running`부터 `hosting=embedded`까지 122 ms가 걸렸고 `StageRectChanged` 호출은 121 ms가 걸렸다. 붙이기 전 게스트 창 DPI, 붙인 뒤 게스트 창 DPI, 부모 창 DPI는 모두 96이었다.

처음에는 부모와 게스트 클라이언트 영역이 모두 1280×800이었다. 부모를 960×600으로 바꾸고 새 사각형을 보낸 뒤 둘 다 960×600이었다. `GuestWindowToFront`는 성공했고, 이어서 조회한 게스트 HWND의 키보드 포커스는 `true`였다.

### 정지

`GuestStop` 뒤 2,114 ms 만에 `stopped`를 확인했다. `lastExit.kind`는 `UserStop`이고 시각은 `2026-09-27T23:32:09+09:00`이었다. 로그 경로는 `qemu-default-20260927-143133443.stderr.log`였다. 종료 확인 때 QEMU 프로세스는 0개였다.

## 두 번째 실행

### 상태와 시간

| 시점 | 상태 |
|---|---|
| 0 ms | `stopped` |
| 152 ms | `starting` |
| 991 ms | `running` |
| 정지 요청 뒤 197 ms | `stopping` |
| 정지 요청 뒤 2,295 ms | `stopped` |

시작부터 부팅 완료까지 37,369 ms, `running`부터 부팅 완료까지 36,378 ms가 걸렸다.

### 능력 조사

조사 시각은 `2026-09-27T23:32:48+09:00`이다. 첫 실행과 같은 값을 얻었다. `adbConnected=true`, 기기 ID hex `39335c0a…(가려짐)`, decimal `4121739…(가려짐)`, Google 계정 1개, 미디어 볼륨 15, 루트 꺼짐, 해상도 960×600이었다.

| 항목 | 상태 | 읽은 값 |
|---|---|---|
| `bootMarker` | `available` | `bootCompleted=true` |
| `appList` | `available` | 제3자 패키지 7개 |
| `displaySize` | `available` | 960×600, 130 dpi |
| `mediaVolume` | `available` | 15 |
| `deviceId` | `available` | `39335c0a…(가려짐)` |
| `screenshot` | `available` | 임시 게스트 screencap 명령 성공 |
| `foregroundApp` | `unknown` | 값 없음 |
| `multitouch` | `unavailable` | 입력 장치 조사에서 없음 |
| `nativeBridge` | `available` | `libndk_translation.so` |
| `root` | `unavailable` | `false` |

`ScreenshotSave`로 `%LOCALAPPDATA%\OpenMobileEmulator\screenshots\20260927-233248.png`를 만들고 `run-2-adb.png`로 복사했다.

### 창 담기

런타임 스냅숏의 `hosting`은 첫 배치 뒤와 960×600 변경 뒤 모두 `embedded`였다. `running`부터 측정 코드가 `embedded`를 기록하기까지 5,099 ms가 걸렸다.

하지만 두 번째 시작에서는 이전 실행의 무대 사각형이 런타임에 남아 있었다. `running` 이벤트를 처리하는 동안 런타임이 창을 곧바로 담았고, 측정 코드가 뒤이어 수행한 최상위 창 열거는 이미 자식 창이 된 QEMU HWND를 찾지 못했다. 원문은 다음과 같다.

```text
run2.failure.window_discovery=no visible QEMU window within 5s
run2.guest_window_class=unknown
run2.guest_dpi_before_attach=unknown
run2.guest_dpi_after_attach=unknown
run2.guest_client_attached=unknown
run2.guest_client_resized=unknown
run2.failure.focus_query=guest HWND was unavailable
```

따라서 두 번째 실행의 DPI, 실제 자식 창 크기, 포커스는 별도로 확인하지 못했다. 이 항목의 판정에는 첫 실행의 측정값을 쓴다. 제품이 보고한 `embedded` 값과 두 번째 부팅·스크린샷·정지는 모두 기록했다.

### 정지

`GuestStop` 뒤 2,295 ms 만에 `stopped`를 확인했다. `lastExit.kind`는 `UserStop`, 시각은 `2026-09-27T23:32:50+09:00`이었다. 로그 경로는 `qemu-default-20260927-143210586.stderr.log`였다. 종료 확인 때 QEMU 프로세스는 0개였다.

두 실행은 서로 다른 command/stdout/stderr 로그 묶음을 만들었다. 두 번째 실행으로 재시작 경로와 실행별 로그 분리를 확인했다.

## ARCHITECTURE 6절 판정

| 질문 | 판정 | 측정 근거 |
|---|---|---|
| DPI | 통과 | 첫 실행에서 부모 96 DPI, 게스트 붙이기 전 96 DPI, 붙인 뒤 96 DPI였다. |
| 포커스 | 통과 | 첫 실행에서 `GuestWindowToFront` 뒤 게스트 HWND의 키보드 포커스가 `true`였다. |
| 크기 변경 | 통과 | 첫 실행에서 부모를 960×600으로 바꾼 뒤 게스트 클라이언트 영역도 960×600이었다. |
| 종료 처리 | 통과 | 첫 실행 2,114 ms, 두 번째 실행 2,295 ms 만에 `stopped`; 둘 다 `lastExit=UserStop`이고 QEMU가 남지 않았다. |

## 남은 측정 공백과 원문

두 번째 실행의 창 HWND 재조회 항목은 앞 절에 적은 이유로 측정하지 못했다. 그리고 플랫폼 크레이트에 부모 영역 캡처 API가 없어 두 실행 모두 호스트 캡처를 건너뛰었다.

두 번째 QEMU stderr에는 알려진 WHPX 메시지가 세 번 있었다. 게스트는 부팅과 렌더링을 계속했고 adb 전원 끄기로 정상 종료했다.

```text
C:\Users\USER\AppData\Local\OpenMobileEmulator\qemu-build\out\bin\qemu-system-x86_64.exe: failed to get xsave state: No error
```

## 이전 측정

### `\\?\` 실행 파일 경로 실패

첫 측정에서는 호스트 조사가 `Path::canonicalize`의 verbatim 경로를 그대로 넘겨 QEMU가 시작 전에 종료했다. 52b3b7c에서 QEMU와 adb 경로의 verbatim 접두사를 떼도록 고쳤다. 당시 stderr는 다음과 같았다.

```text
\\?\C:\Users\USER\AppData\Local\OpenMobileEmulator\qemu-build\out\bin\qemu-system-x86_64.exe: -rtc base=utc: Could not open '\\?\C:\Users\USER\AppData\Local\OpenMobileEmulator\qemu-build\out\bin/../etc//qemu.conf': Invalid argument
```

### adb 전원 끄기 연결 전의 `stopping` 정체

경로 수정 뒤 측정에서는 부팅과 창 담기를 마쳤지만 `SupervisorPolicy::default()`의 전원 끄기 훅이 `false`를 돌려 QMP `system_powerdown`만 보냈다. Android는 이를 전원 키로 처리해 40,488 ms가 지나도 `stopping`에 머물렀고 QEMU가 실행 중이었다.

```text
run1.stop_duration_ms=40488
run1.stop_final_state=stopping
run1.last_exit=none
run1.qemu_processes_after_stop=qemu-system-x86_64.exe       10652 Console                    1  4,963,280 K
```

이번 작업에서 감독자에 독립적인 adb 세션을 가진 전원 끄기 훅을 연결했다. 최종 측정에서는 두 실행 모두 약 2초 만에 `stopped`와 `UserStop`을 확인했다.
