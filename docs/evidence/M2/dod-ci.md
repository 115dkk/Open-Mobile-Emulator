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
