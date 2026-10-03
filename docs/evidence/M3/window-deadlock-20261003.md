# 설치된 제품의 창이 응답하지 않던 교착 (2026-10-03)

사용자가 개발 PC에 설치한 제품에서 창 제목이 `Open Mobile Emulator (응답 없음)`이 되고 무대가 회색으로 굳었다. 게스트는
정상이었다. QEMU 프로세스는 살아 있었고, `adb devices`는 `device`, `sys.boot_completed`는 `1`이었다.

## 1. 설치된 실행 파일

`%LOCALAPPDATA%\Open Mobile Emulator\ome.exe`는 버전 0.1.0이고 2026-10-01 23:19에 쓰인 파일이다(SHA-256 `9D079F1A…`). 옆에
`ome.exe.v0.1.0.bak`(21:59, `3CA6D0E6…`)이 있다. 커밋 8df5197(직접 커널 부팅 이전, 23:12) 무렵의 로컬 빌드를 바꿔 끼운 것으로,
정식 v0.1.1이 아니다. 오늘 시작은 `-kernel` 직접 부팅이었다(`logs/qemu-default-20261003-032030498.cmd.log`). 이 시작에서
옛 디스크 부팅으로부터 옮겨졌다(`migratedFromDisk: true`, `directBootVerified: false`).

## 2. 스레드 스택

WinDbg의 `cdb -pv`(비침습, 프로세스를 멈추지 않음)로 떴다. 이 빌드의 PDB는 없어서 제품 프레임은 `ome+오프셋`으로만 나온다.

UI 스레드(`main`)는 창 위치 변경 메시지 안에서 Rust 잠금을 기다린다.

```
ntdll!NtWaitForAlertByThreadId
KERNELBASE!WaitOnAddress
ome+0x9b7924            (std 잠금 대기)
...
ome+0x1feb41            (창 이벤트 처리기)
comctl32!DefSubclassProc ...
USER32!SendMessageWorker
USER32!RealDefWindowProcW        (WM_WINDOWPOSCHANGED -> WM_MOVE/WM_SIZE)
...
USER32!_fnINLPWINDOWPOS
```

토키오 작업 스레드 하나는 `SetWindowPos`에서 돌아오지 못한다.

```
win32u!NtUserSetWindowPos
ome+0x633a8a
ome+0x6f20a3
ome+0x6b60a1
ome+0x6c37e7
ome+0x228bc4
```

## 3. 원인

창 이벤트 처리기(`host/app/src/window.rs`)는 `Moved`, `Resized`, `ScaleFactorChanged`, `Focused`마다 UI 스레드에서
`state.runtime.lock()`을 잡고 `host_window_moved`를 불렀다. 반대편의 명령 처리(`commands::apply` 등)는 작업 스레드에서
같은 잠금을 쥔 채 창 호스트로 게스트 팝업과 오버레이에 `SetWindowPos`를 부른다. 이 두 창의 소유자는 UI 스레드다. Windows는
다른 스레드가 소유한 창의 위치를 바꾸면 그 스레드가 메시지를 처리할 때까지 호출을 붙잡아 두므로, 두 스레드가 서로를 기다린다.
같은 모양의 대기는 창 닫기(`CloseRequested`와 거기서 부르는 `request_app_exit`)와 트레이 메뉴(`guest-power`, `quit`)에도 있었다.

## 4. 고친 것

UI 스레드는 런타임 잠금을 기다리지 않는다.

- 창 이동 계열 이벤트는 `spawn_blocking` 작업 하나로 합쳐 보낸다(`MOVE_PENDING`). 작업이 시작되면 표식을 먼저 지우므로,
  처리 중에 온 이벤트는 다음 작업을 하나 더 예약한다.
- 창 닫기는 `prevent_close` 뒤 트레이로 숨길지 끝낼지를 작업 스레드에서 정한다.
- 트레이의 `guest-power`와 `quit`도 잠금을 작업 스레드에서 잡는다.

사용자의 인스턴스는 스택을 뜬 뒤 `adb shell svc power shutdown`으로 게스트를 정상 종료하고(QEMU가 스스로 끝남),
응답하지 않는 `ome.exe`를 닫았다.
