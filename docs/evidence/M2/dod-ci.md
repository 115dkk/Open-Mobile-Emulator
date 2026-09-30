<!-- SPDX-License-Identifier: GPL-2.0-or-later -->
<!-- Copyright (C) 2026 Open Mobile Emulator contributors -->

# M2 완료 실행을 GitHub 호스트 윈도우 러너에서 돌린 기록

개발 PC가 완료 실행 지역 10회차 도중 블루스크린을 반복해(2026-09-30) 실기 실행이 불가능해졌다.
`.github/workflows/m2-dod.yml`(windows-2025 러너, WHPX 가능은 `ci-whpx-probe.md`에서 확인)이 그 자리를
대신한다. 이 문서는 실행마다 러너가 무엇이었고 드라이버가 어디까지 갔는지를 적는다. 원문 파일은
`dod-ci/` 폴더에 실행 번호를 붙여 둔다. 러너 이미지가 바뀌면 사실도 바뀌므로 날짜를 함께 적는다.

CI의 완료 실행은 16회 전까지 한 번도 드라이버에 닿지 못했다. 6~14회는 워크플로 파일이 파싱에서
거부됐고(runner 컨텍스트를 잡 env에 둔 것), 15회(2026-09-29, main ecfdc3a)는 ome2 zip의 SHA-256
불일치로 QEMU 받기에서 끝났다. 16회부터 이 브랜치의 워크플로(f885e67)로 돈다.

## 러너 사실 (16회, 2026-09-30 01:47 UTC, `dod-ci/run16-runner-facts.txt`)

| 항목 | 값 |
|---|---|
| OS | Windows Server 2025 Datacenter, 10.0.26100 |
| CPU | AMD EPYC 7763, 2코어 4스레드 |
| 메모리 | 16.0 GiB |
| 디스크 | C: 30.9 GiB 여유, D: 145 GiB 여유(OME 홈은 `D:\ome-home`) |
| 세션 | 대화형(`UserInteractive: True`), 세션 2, 사용자 `runneradmin` |
| 화면 | 1024x768 하나. 준비 단계의 `ChangeDisplaySettings`가 1920x1080으로 바꿨다(반환 0) |
| 비디오 | Microsoft Hyper-V Video 10.0.26100.1150. GPU가 없다 |
| 하이퍼바이저 | `HypervisorPresent` True, `HypervisorPlatform` Enabled(재시작 불필요) |
| WebView2 | Evergreen 런타임 153.0.4234.48이 이미 있다(`EdgeWebView\Application`) |
| Windows OCR | `Language.OCR~~~en-US` Installed, 인식기 1개. 다른 언어는 없다 |
| 도구 | pwsh 7.6.6, Windows PowerShell 5.1(10.0.26100), gh, git 2.55, OpenSSH 9.5, node 24.1.0(setup-node), `ANDROID_HOME=C:\Android\android-sdk`(platform-tools 37.0.1) |

WMI 조회가 느리다. `Get-CimInstance Win32_Processor`와 `Win32_VideoController`에 각각 약 8초,
`Get-WindowsCapability`에 약 19초가 걸렸다. 제품 코드의 호스트 점검은 Win32 호출(`IsProcessorFeaturePresent`,
CPUID, `WinHvPlatform.dll`)이라 이 느림과는 무관하다.

## 실행 기록

| 실행 | 커밋 | 결과 | 드라이버가 간 곳 |
|---|---|---|---|
| 16 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36656117032) | f885e67 | 취소(114분 뒤 손으로) | `launch`: 제품(pid 4996)은 떴지만 CDP 9333이 30초 안에 답하지 않음 |
| 17 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36665957774) | 8fc8e5d | 실패(드라이버가 스스로 종료) | `launch`: 제품 창은 그려졌고 호스트 점검 여섯 줄이 모두 통과했지만 CDP는 120초 뒤에도 답하지 않음 |
| 18 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36667090492) | 6b691bb | 실패(도우미 오류) | `launch`: `CreateProcessAsUser`로 제품이 떴지만(pid 4028, WebView2 자식 6116) 도우미가 `$Error`라는 읽기 전용 변수에 대입하다 죽어 pid를 알리지 못함. 증거 아티팩트 없음(고아 제품이 출력 파일을 쥐고 있어 게시 단계도 실패) |
| 19 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36667569730) | 6ea7669 | 실패 | `launch`: 제한 토큰으로 뜬 제품(pid 5496, 중간 무결성)이 Tauri 설정 훅에서 패닉("the underlying handle is not available")으로 곧 종료. 이 길은 여기서 접는다 |
| 20 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36674327649) | e87b262 | 실패(래퍼 오류) | 표준 사용자 `omeuser` 생성(관리자 아님), 권한 부여 1.4초, `Start-Process -Credential`로 그 계정의 pwsh 시작까지 됨. 래퍼가 셸 폴더 조회로 관리자의 AppData를 받아 그 아래 Temp를 만들다 접근 거부 |
| 21 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36674900503) | e3b3c99 | 실패(`S1.5-installer`) | 표준 사용자로 CDP 접속 성공. 호스트 점검 8행 통과, WHPX 동의 건너뜀, ISO 2.4 GB 64초에 내려받아 검증, 디스크 만들기까지. QEMU가 `Could not initialize DirectSound: No sound driver is available`로 곧 종료(러너에 오디오 장치 없음) |
| 22 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36675968945) | 1e8ebde | 실패(`S1.5-installer`) | 오디오는 `none`으로 넘어감(명령줄에 `-audiodev` 없음). 내려받기 38.8초(캐시 없음, 두 번째 받기). QEMU가 `No provider of glCreateShader found. Requires one of: Desktop OpenGL 2.0 / OpenGL ES 2.0 / GL_ARB_shader_objects`로 종료. 러너의 WGL 기본 컨텍스트는 GDI Generic 1.1 |

### 16회 (2026-09-30 01:39~03:42 UTC)

준비 단계는 모두 통과했다. 빌드 12분(캐시 없음, 이후 실행부터 `actions/cache/save`가 실패한 잡에서도
저장한다), QEMU ome3 zip 해시 일치, 비공개 픽스처 복제 성공(배포 키 비밀이 저장소에 있다).

드라이버(`dod-ci/run16-driver-output.txt`)는 preflight를 0.8초에 지났고(QEMU 실행 파일 SHA-256
`7139f05b…`, 개발 PC에서 검증한 ome3와 같다), 제품을 띄우고 화면 깨우기 도우미를 붙인 뒤
`http://127.0.0.1:9333/json/version`을 30초 기다리다 `Timeout: CDP (fetch failed)`로 끝났다. 제품의
표준 출력과 오류 출력(`app-output.txt`)은 0바이트였고, OME 홈에 로그 폴더가 생기지 않았다. 정리
단계는 제품을 강제 종료했다(종료 코드 -1). 그러니 제품은 살아 있었지만 30초 안에 웹뷰를 만들지
못했거나, 웹뷰가 원격 디버깅 포트를 열지 못한 것이다. 어느 쪽인지는 이 실행의 증거로 가릴 수 없다.

드라이버는 01:48:25에 결과를 쓰고도 종료하지 않았다. 도우미 `keep-awake.ps1`을 stdout 파이프로 띄워
둔 채였고 그 파이프가 노드의 이벤트 루프를 붙들었다. 잡은 03:41에 손으로 취소했고, `if: always()`의
증거 단계는 취소 뒤에도 돌아 아티팩트(2.3 KB)를 올렸다. 커밋 8fc8e5d가 세 가지를 고쳤다. 제품을
기다리는 모든 await에 상한(스냅숏 20초, 설치기 감시자 90초, 브라우저 닫기 10초), 실행 전체 감시
(기본 100분, 넘기면 결과를 쓰고 소유 프로세스를 끝낸 뒤 종료), 끝에서 도우미를 죽이고 `process.exit`.
그리고 CDP 대기를 120초로 늘리고, 그래도 답이 없으면 `native.ps1 -Desktop`으로 앱 프로세스와 자식의
최상위 창 목록, 프로세스 목록, 가상 화면 전체 스크린샷을 남긴다.

### 17회 (2026-09-30 03:47~03:53 UTC)

빌드는 16회가 저장한 캐시로 끝났고(`cache-hit`), 준비 단계는 OCR 인식기 1개, WebView2 있음, 화면
1920x1080으로 지나갔다. 드라이버는 launch에서 CDP를 120초 기다렸고, 답이 없자 `native.ps1 -Desktop`이
남긴 창 목록과 화면(`dod-ci/run17-launch-window.png`)이 처음으로 제품의 상태를 보여 주었다.

- 제품 창 `Open Mobile Emulator`(`Tauri Window`, 1296x839)가 떠 있고, 마법사 1/7 호스트 점검이
  "이 PC에서는 Open Mobile Emulator를 사용할 수 있습니다"와 함께 여섯 줄을 모두 통과로 보였다(CPU
  가상화, Windows 하이퍼바이저 플랫폼 켜짐, 다시 시작 대기 없음, 가상 머신 구성 요소, 앱 설치 도구,
  디스크 여유 공간). 곧 러너에서 제품이 QEMU ome3와 펌웨어, adb를 찾고 WHPX가 준비됐다고 판정한다.
- WebView2 브라우저 프로세스(`msedgewebview2.exe` 153.0.4234.48)의 명령줄에 `--remote-debugging-port`가
  없었다. 드라이버가 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`로 넘긴 값이 적용되지 않은 것이다. 개발
  PC에서는 같은 방법이 통했다(`app-smoke.md`).
- 차이는 권한이다. 러너는 모든 잡을 UAC가 꺼진 관리자 `runneradmin`으로 돌리므로 제품이 높은 무결성
  수준으로 뜬다. WebView2 150부터 높은 무결성으로 뜬 호스트에서는 DevTools 원격 디버깅 끝점이 열리지
  않는다(MicrosoftEdge/WebView2Feedback#5640, 149에서는 되던 것이 150.0.4078.48부터 안 됨). 개발 PC의
  드라이버는 보통 사용자 권한으로 돈다.

그래서 드라이버는 제품을 `ci/dod/start-unelevated.ps1`로 띄운다. 이 도우미는 자기 토큰에서
`CreateRestrictedToken(LUA_TOKEN)`으로 UAC가 걸러 낸 것과 같은 토큰을 만들고 무결성 수준을 중간
(`S-1-16-8192`)으로 내린 뒤 `CreateProcessAsUser`로 제품을 시작한다. 새 사용자를 만들지 않으므로 파일
권한과 바탕 화면은 그대로다. 제품의 표준 출력과 오류 출력은 도우미가 파일로 받고, 도우미는 pid를 한 줄로
알린 뒤 종료 코드를 기다린다. 사용자 PC에서 제품이 관리자 권한으로 뜨는 일은 없으므로 이것은 러너 쪽
조정이지 제품의 동작 변경이 아니다.

### 18·19회 (2026-09-30 04:02~04:17 UTC): 제한 토큰으로 띄우는 길은 접는다

18회는 도우미가 PowerShell의 읽기 전용 `$Error`에 대입하다 죽어 pid를 알리지 못했다(제품은 떴다).
19회는 도우미가 제대로 돌아 제품이 중간 무결성으로 떴지만(`childElevated: false`), 제품은 Tauri 설정
훅에서 "Failed to setup app: error encountered during setup hook: the underlying handle is not
available"로 패닉해 곧 끝났다(`dod-ci/run19-app-output.txt`). 120초 뒤의 바탕 화면 진단에는 창도
프로세스도 없었다. 인위적으로 깎은 토큰 아래에서는 제품의 창이나 트레이 만들기가 실패한 것이다.

이 길은 여기서 접는다. 관리자 세션의 토큰을 깎아 원격 디버깅 끝점을 여는 것은 WebView2가 둔 제한을
돌아가는 일이고, 제품을 그 조건에 맞추는 것도 제품의 몫이 아니다. 드라이버는 6b691bb 이전처럼 제품을
그대로 띄우고(CDP 대기 120초와 바탕 화면 진단은 남긴다), 도우미 스크립트는 지웠다. 남은 선택은
사용자가 정한다. (가) 러너에 표준 사용자 계정을 만들어 드라이버 전체를 그 계정으로 돌린다(제품이
설계된 실행 조건이지만, 작업 폴더와 픽스처의 권한을 그 계정에 열어야 한다). (나) 드라이버의 CDP를
UI 자동화(UIA)로 바꾸고, 스냅숏은 제품이 진단 목적으로 파일에 쓰는 기능을 더해 읽는다(큰 작업).
(다) 러너는 빌드와 호스트 점검까지만 쓰고, 마법사 완주는 실기로 돌린다.

### 결정: 표준 사용자 계정으로 돌린다 (사용자, 2026-09-30)

위 세 선택 가운데 (가)를 골랐다. 워크플로의 마법사 단계는 한 단계 안에서 표준 로컬 사용자
`omeuser`를 만들고(암호는 그 단계의 메모리에만 있다), 체크아웃·노드·비공개 픽스처에 읽기, OME 홈과
증거 폴더에 수정 권한을 주고, `Start-Process -Credential -LoadUserProfile`로 같은 바탕 화면에서 그
계정으로 `ci/dod/run-as-user.ps1`을 띄운다. 래퍼는 프로필 경로(USERPROFILE, LOCALAPPDATA, APPDATA,
TEMP)를 그 계정의 셸 폴더에서 다시 읽고, 드라이버에 필요한 값은 인자로 받아 환경에 넣은 뒤 드라이버를
실행한다. 제품은 로그인한 표준 사용자 아래에서 뜨는 설계 조건 그대로 돌게 된다.

### 20회 (2026-09-30 05:38~05:44 UTC): 표준 사용자로 시작은 된다

`net user`로 만든 `omeuser`는 Administrators에 없었고, 체크아웃·노드·픽스처 읽기와 홈·증거 폴더 수정
권한 부여는 1.4초에 끝났으며, 보조 로그온 서비스와 `Start-Process -Credential -LoadUserProfile`로 그
계정의 pwsh가 러너의 바탕 화면 세션에서 시작됐다. 래퍼는 `[Environment]::GetFolderPath('LocalApplicationData')`
가 `C:\Users\runneradmin\AppData\Local`을 돌려주어 그 아래 Temp를 만들다 접근 거부로 끝났다. 다른 사용자의
자격 증명으로 시작한 프로세스는 호출자의 환경을 물려받고, 셸 폴더 조회는 `%USERPROFILE%`을 그 환경에서
펼치기 때문이다. e3b3c99가 프로필 경로를 HKLM ProfileList의 SID 항목에서 읽고, 없으면 워크플로가 만든
`D:\ome-user`를 쓰도록 고쳤다.

### 21회 (2026-09-30 05:45~05:54 UTC): 표준 사용자로 마법사가 돈다. 막힌 곳은 소리

래퍼는 `omeuser`로 떴고 CDP가 붙었다. 드라이버 단계와 시간은 preflight 0.8초, launch 7.1초,
호스트 점검 1.9초(8행 모두 준비. 새 문구 "필요하지 않습니다.", "설치에 충분한 여유가 있습니다."가
그대로 보인다), WHPX 동의 1.8초(이미 켜져 있어 건너뜀), 내려받기 63.9초(2,429,550,592바이트, 약
44 MB/s, 검증 완료), 설치기 31초 실패다. 원문은 `dod-ci/run21/`이다.

`디스크 만들기`를 누르자 제품이 32 GiB qcow2를 만들고 QEMU를 띄웠지만 QEMU는 곧 끝났다
(`run21/qemu-…stderr.txt`): "Could not initialize DirectSound: No sound driver is available for use,
or the given GUID is not a valid DirectSound device ID". 러너에는 오디오 장치가 없고, QEMU는
`-audiodev dsound` 초기화 실패를 치명적으로 다룬다. 감독자는 `startFailed`로 기록했고 드라이버의
설치기 감시자는 창이 없어 30초 뒤 끝났다(드라이버의 GL 재시도 규칙은 이 문구와 맞지 않는다).

오디오 장치가 없는 PC는 러너만이 아니므로 제품이 고친다. `ome-platform-win`이 `waveOutGetNumDevs`로
출력 장치 수를 읽고(`audio_output_devices`), `ome-host-check`의 `HostProbe`에 그 관찰이 더해지며,
런타임은 게스트 설정을 만들 때 장치가 0이라고 확실히 보고될 때만 오디오 백엔드를 `none`으로 두고
그 밖에는 `dsound`를 유지한다(결정적 테스트 둘). 부수 관찰: 정리 단계의 트레이 `종료`가
"Tray popup did not appear"로 실패해 강제 종료로 끝났다. 완주가 되면 마지막 `quit` 단계에서 같은
일이 날 수 있으므로 지켜본다. 업데이트 확인은 러너에서 `update_check_failed`였다(네트워크).

### 22회 (2026-09-30 05:59~06:07 UTC): 소리는 지났고, 이번엔 OpenGL

오디오 조사가 동작해 QEMU 명령줄에 `-audiodev`가 없었고(`dod-ci/run22/`), QEMU는 DirectSound를 지나
SDL GL 창을 만들었다. 그다음 stderr는 `sdl: failed to set swap interval 0: That operation is not
supported`(GDI 일반 OpenGL 1.1의 전형적인 답) 두 줄 뒤에 libepoxy의 "No provider of glCreateShader
found. Requires one of: Desktop OpenGL 2.0, OpenGL ES 2.0, GL_ARB_shader_objects"로 끝났다. 러너에는
디스플레이 드라이버가 없어(Microsoft Hyper-V Video) WGL이 Microsoft의 소프트웨어 OpenGL 1.1을 주고,
virglrenderer는 셰이더 없이는 돌지 못한다. ANGLE(libEGL, libGLESv2)은 번들에 있지만 SDL의 기본은
WGL이라 쓰이지 않는다.

드라이버에는 GL 실패 때 설정의 소프트웨어 렌더링을 켜고 이어가는 경로가 있었지만, 마법사를 `나중에
하기`로 닫은 뒤 되돌아올 단추가 제품에 없어 그 경로는 성립하지 않는다(그 판정 문구도 이 메시지와
맞지 않았다). 그래서 오디오와 같은 방식으로 제품이 고른다. `ome-platform-win`이 숨은 창에 기본 WGL
컨텍스트를 한 번 만들어 `GL_VERSION`과 `GL_RENDERER`를 읽고(`opengl_capability`), `ome-host-check`의
`OpenGlCapability::supports_virgl`(주 버전 2 이상이고 렌더러가 `GDI Generic`이 아님)이 판정하며,
런타임은 시작할 때 설정이 virgl이고 조사가 확실히 '안 됨'이면 소프트웨어 렌더링으로 바꿔 저장하고
알림을 남긴다. 드라이버는 첫 부팅에서 스냅숏의 `settings.gpuMode`가 `software`면 ISO 저자들의
"No HW Acceleration" GRUB 항목을 고른다(표준 VGA에서는 기본 항목이 초기 사용자 공간에서 멈춘다,
`docs/evidence/M0/guest-install.md`).
