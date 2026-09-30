# M2 완료 기준 2: R9, R10 위반이 없음을 코드 검색으로 확인한 기록

확인일 2026-09-30, 커밋 bdd7fa6 (브랜치 `ccr-4ddf4024-unn46m`, PR #4). 검색 범위는 제품과 런처 코드다:
`host/crates`, `host/app/src`, `host/ui/src`, `launcher`, `ci/dod`, `translator`. `host/target`과
`node_modules`는 뺐다. 결론은 둘 다 위반 없음이며, 문서 한 곳의 표현을 고쳤다(아래 R10 3번).

## R9. 시스템 설정 변경은 명시적 동의 후에만

검색

```
grep -rniE "bcdedit|hypervisorlaunchtype|Enable-WindowsOptionalFeature|dism(\.exe)?\b|HypervisorPlatform" \
  --include=*.rs --include=*.ts --include=*.tsx --include=*.ps1 --include=*.psm1 --include=*.mjs --include=*.json \
  host launcher ci translator installer
grep -rniE "Restart-Computer|shutdown(\.exe)? /r|InitiateSystemShutdown|ExitWindowsEx" (같은 범위)
```

결과

1. `bcdedit`와 `hypervisorlaunchtype`는 어느 파일에도 없다. 재부팅 명령(`Restart-Computer`, `shutdown /r`,
   `InitiateSystemShutdown`, `ExitWindowsEx`)도 없다.
2. 시스템을 바꾸는 명령은 `dism.exe /Online /Enable-Feature /FeatureName:HypervisorPlatform /NoRestart`
   하나뿐이고 두 곳에 있다.
   - 제품: `host/crates/ome-setup/src/lib.rs`. 별도 실행 파일 `ome-setup.exe`가 인수 `enable-whpx` 하나만
     받아(`parse_args`, 그 밖의 인수는 사용법 오류) `%SystemRoot%\System32\dism.exe`를 고정 인수로 실행한다.
     이 실행 파일을 띄우는 곳은 `host/app/src/setup.rs`의 `enable_whpx`이며 `ShellExecuteExW`의 `runas`
     동사(`host/crates/ome-platform-win/src/ffi/elevation.rs`)로 UAC 창을 띄운다. 런타임에서 그 경로에
     닿는 명령은 `Command::WhpxEnable`(`host/crates/ome-runtime/src/runtime.rs` 742, 1091행) 하나이고,
     웹뷰에서 그 명령을 부르는 곳은 마법사 S1.2의 `활성화` 단추(`host/ui/src/screens/WizardScreen.tsx`
     143행)와 차단 화면의 동의 단추(`host/ui/src/screens/BlockedScreen.tsx`, `consent` 상태가 참일 때만)뿐이다.
     자동으로 부르는 코드 경로는 없다. 재부팅이 필요하면 `Outcome::WhpxEnabledNeedsReboot`로 안내 화면이
     나오고 재부팅은 사용자가 한다.
   - 런처: `launcher/Enable-Whpx.ps1`. `-Consent` 스위치가 없으면 무엇을 할지 보여 주고 끝난다(70행).
     `Start-Guest.ps1`과 `Check-Host.ps1`은 상태를 읽기만 한다(`Get-CimInstance Win32_OptionalFeature`).
3. 웹뷰 능력(`host/app/capabilities/main.json`)은 앱의 타입이 있는 명령만 허용하고 셸, 프로세스, 파일
   플러그인 권한이 없다. 그러므로 웹뷰가 위 경로 밖에서 시스템을 바꿀 방법이 없다.

## R10. 네트워크와 데이터

검색

```
grep -rniE "reqwest|ureq|hyper::|Invoke-WebRequest|Invoke-RestMethod|Start-BitsTransfer|WebClient|HttpClient|fetch\(|XMLHttpRequest|WebSocket\(" \
  --include=*.rs --include=*.ts --include=*.tsx --include=*.ps1 --include=*.psm1 --include=Cargo.toml \
  host/crates host/app host/ui/src launcher
grep -rnoE "https?://[A-Za-z0-9.-]+" (같은 범위와 manifests)
grep -rniE "telemetry|analytics|sentry|crashpad|appcenter|posthog|mixpanel" (host/crates host/app host/ui/src)
```

결과

1. HTTP 클라이언트는 `ureq`(rustls) 하나이며 `host/crates/ome-artifacts/src/lib.rs`의 `UreqFetch`에만
   있다. 웹뷰에는 `fetch`, `XMLHttpRequest`, `WebSocket` 호출이 없고 CSP의 `connect-src`는
   `ipc: http://ipc.localhost https://ipc.localhost`뿐이다(`host/app/tauri.conf.json`). 웹뷰 탐색은
   `local_navigation`이 패키지 원본과 디버그 서버만 허용한다(`host/app/src/window.rs`).
2. `UreqFetch`를 쓰는 경로와 호스트는 셋이고 모두 `docs/NETWORK.md`에 있다.
   - 게스트 ISO: `manifests/artifacts.json`의 URL만 받고(`ArtifactStore`, 480행) 리디렉션 뒤의 최종 URL도
     매니페스트의 `allowed_hosts`(`sourceforge.net` 접미사, HTTPS만, 585행)로 다시 검사한다. 런처의
     `Get-Artifacts.ps1`도 같은 매니페스트와 `Test-OmeAllowedUrl`을 쓴다.
   - 업데이트 확인: `https://api.github.com/repos/115dkk/Open-Mobile-Emulator/releases/latest` 고정
     (`fetch_release_document`, 3048행). 최종 URL이 `api.github.com`이 아니면 실패로 다룬다.
   - 업데이트 내려받기: 릴리스 자산 URL을 `github.com`, `objects.githubusercontent.com` 허용 목록으로
     검사하고(3138, 3181행) 체크섬 파일이 없는 릴리스는 거부한다. 설치는 설정 화면의 `다운로드 후 설치`,
     `설치` 단추(`SettingsScreen.tsx` 507, 523행)로만 시작한다.
3. 업데이트 확인은 두 경로다. 설정 화면의 `업데이트 확인` 단추(541행)와, 설정 `auto_update_check`(기본 켬,
   `settings.rs` 60행)가 켜져 있을 때 앱 시작 시 한 번(`start_auto_update_check`, `host/app/src/lib.rs`
   197행). CLAUDE.md M2 7번은 "GitHub Releases 조회, 서명 검증, 사용자가 누를 때만 설치"이므로 조회의
   자동 실행은 규칙 안이다. 다만 `docs/NETWORK.md`의 자체 업데이트 행이 "user-initiated"라고만 적어
   시작 시 조회를 빠뜨렸으므로 그 행을 고쳤다(이 커밋).
4. 브라우저로 여는 주소는 `open_help`의 `github.com` 도움말·릴리스 페이지(`runtime.rs` 1827~1845행)와
   `open_registration_page`의 `https://www.google.com/android/uncertified/`(1890행)뿐이며 둘 다
   `docs/NETWORK.md`의 "Opened in the user's default browser" 표에 있다. 제품 자체는 이 주소로 요청을
   보내지 않는다.
5. 소스에 있는 그 밖의 호스트는 테스트 안의 가짜 주소(`example.com`, `evil.example`, `notsourceforge.net`,
   `sourceforge.net.evil.com` 등, 허용 목록이 거부하는지 확인하는 용도)와 주석의 출처 링크
   (`gitlab.com/qemu-project`, `gitlab.com/keycodemap`, `github.com/aosp-mirror`)뿐이다.
6. 원격 측정 코드는 없다(위 세 번째 검색 결과 0건). 크래시와 진단은 `diagnostics_export`가 로컬 파일로만
   묶는다.
