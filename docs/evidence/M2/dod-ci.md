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
| 23 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36677213522) | e6433d0 | 실패(`S1.4-download`) | OpenGL 조사가 동작해 시작 알림 "하드웨어 가속 그래픽이 없어 소프트웨어 렌더링으로 설정" 남고 `gpuMode: software`. 이번 SourceForge 미러는 3 MB/s였고 953 MB에서 전송이 끊겨 `artifact_download_failed`. 코드 밖의 일 |
| 24 (https://github.com/115dkk/Open-Mobile-Emulator/actions/runs/36678351622) | 3f9f7ce | 취소(117분 뒤 손으로) | 드라이버는 06:33에 시작해 스스로 끝났지만(끝난 시각 미상) 워크플로의 `Start-Process -Wait`가 제품이 띄운 adb 서버(자손)를 기다리며 멈췄고, 그 adb가 출력 파일 핸들을 쥐고 있어 증거 게시도 실패. 잡 정리에서 고아 프로세스로 `adb`(pid 2120)만 종료됨. 드라이버 출력은 남지 않음 |

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

### 23회 (2026-09-30 06:14~06:26 UTC): 그래픽 조사는 동작, 미러가 끊김

시작 알림 "이 PC에는 하드웨어 가속 그래픽(OpenGL 2.0 이상)이 없어 소프트웨어 렌더링으로 설정했습니다."가
스냅숏에 있었고 `settings.gpuMode`는 `software`였다(`dod-ci/run23/dod-result.json`). 내려받기는 이번
미러에서 약 3 MB/s로 흘렀고(21·22회는 44 MB/s) 953 MB에서 끊겨 제품이 `artifact_download_failed`
("운영체제 이미지를 다운로드하거나 검증하지 못했습니다. 네트워크 연결을 확인한 뒤 다시 시도하십시오.")를
보였다. 드라이버는 사람이 하듯 `다운로드`를 한 번 더 누르고 그 사실을 기록하게 했다(두 번째 실패는
그대로 실패). 정리 단계의 트레이 `종료`는 이번에도 메뉴가 열리지 않았다. 러너에서는 제품이 바탕 화면의
explorer와 다른 사용자로 돌아 알림 아이콘이 등록되지 않는 것으로 보이므로, 드라이버는 그때 창 닫기
(WM_CLOSE, 제품의 닫기 동작)로 끝내고 빈자리로 적는다.

### 24회 (2026-09-30 06:28~08:30 UTC): 드라이버는 끝났는데 워크플로가 adb 서버를 기다렸다

이 회차의 드라이버 출력은 남지 않았다. 워크플로가 드라이버의 표준 출력을 `Start-Process -Wait`가 돌아온 뒤
잡 로그에 싣는데, `-Wait`는 지정한 프로세스의 모든 자손이 끝날 때까지 기다린다. 잡 정리가 종료한 고아
프로세스는 `adb`(pid 2120) 하나뿐이었고 ome.exe, QEMU, node는 없었다. 즉 드라이버는 스스로 끝났고(결과는
미상), 제품이 띄운 adb 서버만 남아 `-Wait`를 붙들었으며, 그 adb가 물려받은 출력 파일 핸들 때문에
증거 게시 단계도 "being used by another process"로 실패했다. 117분 뒤 손으로 취소했다.

고침: `Start-Process -PassThru` 뒤 `WaitForExit(115분)`으로 드라이버 프로세스 하나만 기다리고, 끝나면
`omeuser` 소유의 adb·제품·QEMU·WebView2·node·powershell 프로세스를 종료한 뒤 출력을 싣는다. 증거 단계도
읽기 전에 같은 정리를 한다.

### 25회 (2026-09-30 08:32~08:41 UTC): 드라이버 출력이 처음 실렸고, OCR이 GRUB 메뉴를 못 읽었다

24회의 고침이 맞았다. 드라이버는 스스로 끝났고 그 출력이 잡 로그에 실렸으며 증거 게시와 업로드도
통과했다(`dod-ci/run25/`). 단계별로 preflight 0.8초, launch 7.4초, 호스트 점검 2.0초(8행 준비),
WHPX 1.7초(건너뜀), 내려받기 43.5초(2,429,550,592바이트, 약 60 MB/s, 검증 완료), 설치기 65.6초 실패다.

`디스크 만들기` 뒤 제품은 qcow2를 만들고 QEMU를 표준 VGA, 오디오 없음으로 띄웠다(`run25/qemu-installer.cmd.txt`.
stderr는 Skylake-Client-v4의 CPUID 경고 세 가지뿐). 설치기 창은 872x608 픽셀이었다. 드라이버는 60초 동안
2.5초마다 창을 찍어 OCR에 넣었는데, 08:39:42부터 30초 동안 읽힌 글은 GRUB 바닥줄 "Enter: Boot Selected
E: Edit Selected"의 깨진 판독("EnLev: BooL SelecL*'d Edit Selected Tet")뿐이었고 메뉴 항목은 한 줄도
없었다. 그 사이 GRUB의 30초 카운트다운이 기본 항목(Live)을 부팅해 08:40:20에는 Bliss 부팅 배너("Have a
Truly Blissful Experience", Virtual A/B 경고)가 읽혔고, 드라이버는 `installation`을 끝내 보지 못해 끝났다.
실패 화면 캡처는 "Foreground changed; refusing capture"로 막혔다(설치기 창이 전경).

원인은 배율이다. 제품은 설치기 창을 무대의 논리 크기 872x608에 맞추는데, 개발 PC는 200% 배율이라
같은 창이 1744x1216 물리 픽셀이고(`dod-local/shots/installer-00-grub-menu.png`, 그 회차의 OCR은 항목
여덟 줄을 다 읽었다) 러너는 100% 배율이라 872x608이다. Windows OCR은 그 크기의 GRUB 항목 글자(약 11픽셀)를
읽지 못한다. 고침은 드라이버 쪽이다. `ci/dod/ocr.ps1`이 판독 전에 이미지를 정수 배로 키운다(Fant 보간,
너비가 약 1600픽셀 이상이 되는 가장 작은 배수, `OcrEngine.MaxImageDimension` 안). 872x608은 2배, 개발
PC의 1744x1216은 그대로다. 저장하는 스크린샷은 화면 복사 원본이고 판독 결과에 `scale`을 함께 적는다.
실패 캡처는 전경 검사에 막히면 바탕 화면 복사로 대신한다.

부수 관찰: 마지막 캡처(`run25/installer-00-grub-menu-last-capture.png`)는 Live 부팅이 해상도를 바꾼 뒤의
창 자리를 찍은 것으로 위 315픽셀은 제품 무대의 어두운 배경과 스크롤 막대, 아래는 검정이다. 게스트가
해상도를 바꾸는 순간 창이 어떻게 놓이는지는 설치가 진행되면 다시 본다. 정리 단계의 트레이 `종료`는 이번에도
메뉴가 열리지 않아 창 닫기로 끝냈고, 제품 출력에 "Error removing system tray icon"이 남았다(러너의
사용자 분리, 23회 참고).

### 26회 (2026-09-30 08:50~09:01 UTC): 2배 판독으로도 GRUB 항목은 안 읽히고 바닥줄만 읽힌다

OCR 확대는 동작했다(`run26/grub-ocr-sequence.txt`, 매 판독에 `scale 2`). 그런데 판독 결과는 25회와 같은
모양이다. 메뉴가 뜬 08:59:01부터 27초 동안 바닥줄 "Enter: Boot Selected E: Edit Selected C: Grub Terminal"의
깨진 판독("EnLer: BOOL SelecLed E: Edit Selected C: Grub Tet")과 "Advanced options ->"의 잔해만 있고 항목
여덟 줄은 없다. 항목 글자는 872x608로 줄어들 때 획이 뭉개져 2배로 다시 키워도 살아나지 않고, 바닥줄의
"Edit Selected"만 두 회차 모두 안정적으로 읽힌다. 08:59:30에 카운트다운이 Live를 부팅했고 드라이버는 60초
뒤 끝났다. 이번에는 실패 캡처가 남았다(`run26/failure-live-boot-in-stage.png`). 무대 안의 게스트 창은
정상이고 Live 부팅 배너가 보이며, 그 옆의 `설치 완료` 단추가 켜져 있다(아래 제품 고침 참고). 내려받기는
110초(약 22 MB/s)였다.

고침 둘. 드라이버는 GRUB 메뉴의 신호를 항목 글자 대신 바닥줄(`edit selected`, `boot selected`, `grub te`,
또는 `installation`)로 삼는다. 바닥줄은 ISO와 설치된 디스크의 GRUB에 같은 테마로 찍히므로 첫 부팅의
소프트웨어 렌더링 항목 선택(`29-disk-vm-options`)도 이 신호를 기다린 뒤 키를 보낸다(그전에는 창이 뜨고
0.5초 뒤에 보내 펌웨어 단계에 닿을 수 있었다). 기다리는 동안의 프레임은 `…-00`, `…-01`처럼 번호를 붙여
전부 남긴다. 고른 항목이 맞는지는 다음 화면(파티션 대화 상자, 첫 부팅의 부팅 완료)이 증명한다.

제품 고침. 마법사 S1.5의 `설치 완료`는 게스트 기록이 있으면 켜졌다(`guest_installed`가 기록 유무만
봤다). 설치기 QEMU가 시작에 실패하거나(21회의 DirectSound) 끝나기 전에 죽어도 사용자가 빈 디스크로
다음 단계에 갈 수 있었다. 이제 설치기가 실패 상태면 그 단계는 이어갈 수 없고, 무대에 실패 문구와 로그
경로, `다시 설치`(디스크를 지우고 처음부터)가 뜬다(`installer_failure_blocks_install_completion_until_the_installer_ends_normally`,
`install-failed` 화면 픽스처와 UI 테스트). 위 실패 캡처처럼 설치기가 살아 있는데 Live로 들어간 경우는
제품이 알 수 없으므로 그대로 사용자의 판단에 둔다(안내 6번).

### 27회 (2026-09-30 09:06~09:16 UTC): 메뉴는 알아봤고, DOWN 한 번이 여섯 칸을 갔다

바닥줄 신호는 두 번째 프레임(09:14:36)에서 잡혔고 드라이버는 HOME, DOWN 넷, ENTER를 보냈다
(`run27/grub-key-sequence.txt`). 프레임을 보면 HOME 뒤 선택은 첫 항목, 첫 DOWN 뒤 둘째 항목이었는데
(`run27/grub-down-1.png`), 둘째 DOWN 뒤에는 마지막 여덟째 항목 `Advanced options ->`에 가 있었다
(`run27/grub-down-2.png`). 한 번 누른 DOWN이 여섯 칸을 간 것이다. 셋째와 넷째 DOWN은 바닥에서 멈췄고
ENTER는 그 항목을 열어 부팅 배너로 이어졌다(`run27/after-enter.png`). 드라이버는 다음 화면에서
`partition`을 못 찾아 "refusing blind partition keystrokes"로 멈췄다(맹목 키 입력 거부가 의도대로 동작).

원인은 키 반복이다. 호스트 쪽 SDL 경로(`native.ps1`의 `Tap`)는 키를 약 180 ms 누르고 떼는데, EDK2의
USB 키보드 드라이버는 500 ms 넘게 눌린 키를 33 ms마다 반복한다. 러너는 Hyper-V 안의 중첩 가상화라
QEMU 주 루프나 게스트가 잠깐 멈추면 뗌이 늦게 닿고, 그 한 번이 여섯 칸이 됐다. 개발 PC에서는 같은
코드가 한 번도 반복을 내지 않았다.

고침. 키를 사람이 치듯 제품 창으로 보낸다. Playwright가 웹뷰에 `keydown`·`keyup`을 넣으면 제품의
`useGuestKeyboard`가 `input_host_key`로 넘기고, 런타임의 입력 게이트는 게스트가 부팅 완료 신호를 내기
전에는 프로필에 묶인 키까지 전부 원시 키로 QMP `input-send-event`에 보낸다(`ome-input::Gate::admit`,
설치기와 설치된 GRUB 모두 그 조건이다). 누름과 뗌이 각각 QMP 명령이라 게스트 펌웨어가 반복을 낼 만큼
눌린 상태가 생기지 않고, QEMU의 HID 큐는 두 보고를 차례로 내주므로 짧은 눌림도 잃지 않는다. 게이트가
제품 주창을 전경으로 요구하므로 키 앞에 주창을 전경으로 올린다. `OME_DOD_SDL_KEYS=1`이면 예전 경로다.

### 28회와 29회 (2026-09-30 09:23~09:34 UTC): 제품 키 경로는 정확했고, 드라이버의 전경 검사가 멈춰 세웠다

28회(eccc08d)와 29회(6cf1cb9, 대문자 Shift까지)를 나란히 돌렸다. 제품 창으로 넣은 키는 GRUB에서 한 칸씩
정확히 움직였다. 29회는 HOME, DOWN 넷, ENTER 뒤 선택이 `Installation`에 있었고
(`run29/grub-down-4-installation-selected.png`), 30초 뒤 설치기의 첫 대화 상자 "Choose Partition …
UEFI System detected! Please select a (ESP) partition as EFI System Partition … Create/Modify partitions …"가
2배 판독으로 그대로 읽혔다(`run29/installer-choose-partition.png`, `key-sequence.txt`). 설치기 대화 상자의
글자는 이 배율에서 읽힌다는 뜻이다. 키 반복은 두 회차 모두 없었다.

둘 다 드라이버 자신의 검사에서 멈췄다. 키 앞에 주창을 전경으로 올리는 도우미가 600 ms 뒤 전경이 주창이
아니면 "Foreground changed; refusing capture"를 던지는데, 28회는 둘째 키 때(전경 918110, 한 번),
29회는 파티션 대화 상자에서 `c`를 치기 직전에 그 검사에 걸렸다. 러너에서는 다른 창이 잠깐 전경을
가져갔다가 돌려준다(28회의 키는 그 보고에도 불구하고 게이트를 지나 GRUB에 닿았다). 고침: 키 앞의
전경 올리기는 `-Foreground` 동작으로 바꿔 최대 3초 동안 주창이 전경이 될 때까지 되풀이하고, 안 되면
거부하는 대신 전경 창의 클래스·PID·제목을 적고 계속한다. 키가 닿았는지는 그다음 캡처가 말해 준다.
설치기 화면 순환은 행동 뒤 20초 동안 화면이 그대로면 한 번 더 행동한다(게이트가 떨어뜨린 키의 보정,
같은 화면에서 두 번까지, 진행 중·카운트다운 화면은 제외).

### 30회와 31회 (2026-09-30 09:37~09:49 UTC): cfdisk 순서는 끝까지 갔고, 세 군데가 어긋났다

30회(0a7a0a8)는 GRUB, 파티션 대화 상자, `c`와 Enter, cfdisk 진입, 레이블 gpt까지 지나 cfdisk의 키 순서를
끝까지 보냈다(`run30/key-sequence.txt`, 모든 키가 `settled`). 결과 표(`run30/cfdisk-table-before-quit.png`)는
세 가지 어긋남을 보인다.

- 크기 입력 `512M`에서 첫 글자 `5`가 빠져 12M 파티션이 됐다(`run30/cfdisk-size-prompt-12M.png`, 섹터
  24576). 쓰기 확인의 `yes`도 받아들여지지 않아 "Did not write partition table to disk."가 남았다.
  둘 다 전경 도우미가 동작한 직후의 첫 키였으므로, 키 앞에 300 ms를 쉬고 화면으로 확인한다.
- 형식 목록에서 HOME, Enter는 `MBR partition scheme`을 골랐다. libfdisk의 GPT 형식 순서는 MBR partition
  scheme, EFI System, BIOS boot …이라 EFI System은 둘째 항목이다(M0 기록의 "첫 항목"은 틀렸다). HOME,
  DOWN, Enter로 바꾸고 표에 EFI System이 보이는지 확인한다.
- 둘째 파티션은 만들어지지 않았다(위 어긋남의 연쇄).

31회(9faedb7)는 Installation을 고른 뒤 30초 시점의 판독이 비어 있어 "Expected installer partition dialog"로
멈췄다. 29·30회는 30초에 대화 상자가 읽혔으니 러너마다 부팅 속도가 다르다. 한 번 찍고 판정하는 대신
대화 상자가 읽힐 때까지 최대 180초 기다린다.

고침(드라이버). cfdisk 단계는 키를 보낸 뒤 기대하는 글이 화면에 읽힐 때까지 기다리고, 안 읽히면 같은 키를
다시 보내는 `keysUntil`로 바꿨다(같은 화면에서 되풀이해도 안전한 순서만 고른다). 크기 입력은 프롬프트
줄의 `512M`, 파티션 생성은 표의 `512M`, 형식은 표의 `EFI System`(아니면 목록을 다시 열어 두 번 더),
둘째 파티션은 표의 `Linux filesystem`, 쓰기는 `W` 뒤 "Are you sure"와 `yes` Enter 뒤 "altered"
(거부되면 세 번까지), 종료는 설치기의 "Choose Partition"으로 확인한다. 판독은 2를 Z, v를 u, W를 U로
읽으므로 패턴이 그 변형을 허용한다.

### 32회 (2026-09-30 09:54~10:06 UTC): DOWN 하나가 사라져 PC-Mode Live로 들어갔다

제품 키 경로로 HOME과 DOWN 넷을 보냈고 모두 `settled`였는데, 넷째 DOWN 뒤 선택은 넷째 항목 `Live PC-Mode
w/ FFMPEG`에 있었다(`run32/grub-down-4-pc-mode-selected.png`). 키 하나가 게스트에 닿지 않은 것이다.
Enter는 그 Live 항목을 부팅했고 콘솔에 "PC MODE … will not work once … modules are loaded" 배너가 남은 채
표준 VGA에서 초기 사용자 공간이 멈췄다(M0 시도 4와 같은 증상). 드라이버는 파티션 대화 상자를 180초 기다리다
끝났다. 30회의 첫 글자 유실과 같은 종류다. 전경 도우미는 주창이 전경이라고 보고했으므로 키가 떨어지는
순간은 그 보고 사이에 있다. 도우미가 이제 정착 뒤 600 ms 동안 전경을 12번 표본해 주창이 아닌 창의
클래스·PID·제목을 적는다(다음 회차의 진단).

키를 믿는 대신 화면으로 확인한다. `native.ps1 -Highlight`는 게스트 캡처에서 GRUB 테마의 선택 막대(가로로
넓은 파란 픽셀 띠)와 항목 행(글 왼쪽 열의 파란 연꽃·꺾쇠 아이콘, 선택 행에서는 흰색)을 찾아 선택된 행
번호와 항목 수를 돌려준다. 규칙은 모두 창 크기의 비율이라 러너의 872x608과 개발 PC의 1744x1216에서 같은
답을 낸다(`run32/grubrow-prototype.py`, 27~32회와 개발 PC 프레임에서 0·1·7·1·4·3행을 맞게 읽었다).
드라이버의 `grubSelect`는 프레임을 읽고 목표 행과 다르면 DOWN 또는 UP을 하나 보낸 뒤 다시 읽기를 목표에
닿을 때까지(최대 14프레임) 되풀이하고, 항목 수가 기대(주 메뉴 8, VM Options 4)와 다르면 멈춘다. 설치기
GRUB(Installation, 5행)과 첫 부팅의 설치된 GRUB(VM Options 5행, 하위 메뉴 No HW Acceleration 2행) 모두
이 방식이다.

### 33회 (2026-09-30 10:11~10:20 UTC): 판독기가 컴파일되지 않았다

GRUB 메뉴까지 갔고 첫 `-Highlight` 캡처에서 멈췄다. `Add-Type -ReferencedAssemblies`에 System.Drawing만
넘기자 러너의 pwsh가 기본 참조 집합을 빼 `List<>`를 못 찾았고, 결과 변수 `$highlight`는 스위치 `$Highlight`와
같은 변수라(PowerShell은 대소문자를 가리지 않는다) 일반 캡처의 JSON에 스위치 객체가 실렸다. 판독기를 컴파일
없는 PowerShell 함수(`Get-GrubHighlight`, GetPixel 표본 약 3만 개)로 바꾸고 변수 이름을 `$grubHighlight`로
나눴다. 이 컨테이너에 pwsh 7.5.2를 받아 캡처 프레임의 RGB 덤프를 가짜 비트맵으로 넣어 돌린 결과
(`run32/grub-highlight-mock-test.ps1`), 27·29·32회와 개발 PC 프레임에서 0·7·4·3·(메뉴 없음)·0행을 맞게
읽었고 프레임당 0.4~1.7초였다.

### 34회 (2026-09-30 10:23~10:33 UTC): 전경 표본의 JSON 직렬화

GRUB 메뉴에서 HOME까지 보내고 둘째 키 앞의 전경 도우미가 `ConvertTo-Json`에서 멈췄다. 표본한 다른 창을
Int64 핸들을 키로 하는 해시테이블에 담았는데 ConvertTo-Json은 문자열 키만 받는다. 즉 그 순간 주창이 아닌
창이 전경에 잡혔다는 뜻이기도 하다(다음 회차부터 클래스·PID·제목이 기록된다). 키를 문자열로 바꿨고,
이 컨테이너의 pwsh 7.5.2로 `ci/dod/*.ps1`을 파서에 넣어 구문 오류가 없는 것과 같은 모양의 해시테이블이
직렬화되는 것을 확인했다. 또 GRUB의 Enter도 프레임으로 확인한다(4a05fa8): 메뉴가 사라졌는지 또는 기대한
항목 수의 하위 메뉴가 떴는지 보고, 사라진 Enter는 다시 누르며, 선택 행이 움직여 있으면 멈춘다.
